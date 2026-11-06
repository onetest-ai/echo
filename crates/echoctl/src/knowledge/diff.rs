//! A line diff for pages: what a permission card, an ingest change or a Health fix shows (spec D27). A plain LCS over
//! lines — a page is at most a few thousand lines — after the common start and end are set aside.
//!
//! Lines are compared as text *including* any `\r`, so a CRLF file diffs cleanly against itself; the final line break
//! is not part of the last line. Output lines carry no `\n`.
//!
//! Bounded: if either side is over [`MAX_LINES`] lines, or what is left between the common start and end is too big
//! for the table ([`MAX_CELLS`]), the diff falls back to "everything removed, everything added" — correct, never slow.

/// Over this many lines on either side, the diff is "all removed, all added".
pub const MAX_LINES: usize = 20_000;
/// The most cells the LCS table may have (the changed middle of both sides, multiplied).
const MAX_CELLS: usize = 16_000_000;

fn split(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return vec![];
    }
    s.strip_suffix('\n').unwrap_or(s).split('\n').collect()
}

/// `old` against `new`: `'='` kept, `'-'` removed, `'+'` added, in order.
pub fn lines(old: &str, new: &str) -> Vec<(char, String)> {
    let (a, b) = (split(old), split(new));
    let all = |a: &[&str], b: &[&str]| -> Vec<(char, String)> { a.iter().map(|l| ('-', l.to_string())).chain(b.iter().map(|l| ('+', l.to_string()))).collect() };
    if a.len() > MAX_LINES || b.len() > MAX_LINES {
        return all(&a, &b);
    }
    let pre = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (ma, mb) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let mut out: Vec<(char, String)> = a[..pre].iter().map(|l| ('=', l.to_string())).collect();
    if (ma.len() + 1).saturating_mul(mb.len() + 1) > MAX_CELLS {
        out.extend(all(ma, mb));
    } else {
        // t[i][j]: the LCS length of ma[i..] and mb[j..].
        let w = mb.len() + 1;
        let mut t = vec![0u32; (ma.len() + 1) * w];
        for i in (0..ma.len()).rev() {
            for j in (0..mb.len()).rev() {
                t[i * w + j] = if ma[i] == mb[j] { t[(i + 1) * w + j + 1] + 1 } else { t[(i + 1) * w + j].max(t[i * w + j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < ma.len() || j < mb.len() {
            if i < ma.len() && j < mb.len() && ma[i] == mb[j] {
                out.push(('=', ma[i].to_string()));
                i += 1;
                j += 1;
            } else if j >= mb.len() || (i < ma.len() && t[(i + 1) * w + j] >= t[i * w + j + 1]) {
                out.push(('-', ma[i].to_string()));
                i += 1;
            } else {
                out.push(('+', mb[j].to_string()));
                j += 1;
            }
        }
    }
    out.extend(a[a.len() - suf..].iter().map(|l| ('=', l.to_string())));
    out
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))] // shown by permission cards (`tray`) and ingest changes (a later task)
/// The changes with `context` unchanged lines around each; changes whose contexts touch share a hunk. Nothing changed:
/// no hunks.
pub fn hunks(d: &[(char, String)], context: usize) -> Vec<Vec<(char, String)>> {
    let changed: Vec<usize> = d.iter().enumerate().filter(|(_, (k, _))| *k != '=').map(|(i, _)| i).collect();
    let mut out: Vec<Vec<(char, String)>> = vec![];
    let mut cur: Option<(usize, usize)> = None;
    for i in changed {
        let (from, to) = (i.saturating_sub(context), (i + context + 1).min(d.len()));
        match cur {
            Some((f, t)) if from <= t => cur = Some((f, to.max(t))),
            Some((f, t)) => {
                out.push(d[f..t].to_vec());
                cur = Some((from, to));
            }
            None => cur = Some((from, to)),
        }
    }
    if let Some((f, t)) = cur {
        out.push(d[f..t].to_vec());
    }
    out
}

/// `(added, removed)` line counts.
pub fn counts(d: &[(char, String)]) -> (usize, usize) {
    (d.iter().filter(|(k, _)| *k == '+').count(), d.iter().filter(|(k, _)| *k == '-').count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_marks_removed_and_added_lines() {
        let d = lines("a\nb\nc\n", "a\nB\nc\nd\n");
        let s: Vec<String> = d.iter().map(|(k, l)| format!("{k}{l}")).collect();
        assert_eq!(s, ["=a", "-b", "+B", "=c", "+d"]);
        assert_eq!(counts(&d), (2, 1));
        let h = hunks(&lines("1\n2\n3\n4\n5\n6\n7\n8\n9\n", "1\nX\n3\n4\n5\n6\n7\n8\nY\n"), 1);
        assert_eq!(h.len(), 2, "far apart: two hunks");
        assert_eq!(h[0].iter().map(|(k, _)| *k).collect::<String>(), "=-+=");
        assert_eq!(hunks(&lines("1\n2\n3\n4\n", "1\nX\n3\nY\n"), 1).len(), 1, "touching contexts share a hunk");
        // Edges: empty sides, no final line break, CRLF.
        assert_eq!(lines("", "x\n"), [('+', "x".to_string())]);
        assert_eq!(lines("x", ""), [('-', "x".to_string())]);
        assert_eq!(counts(&lines("a\r\nb\r\n", "a\r\nc\r\n")), (1, 1));
        assert!(lines("a\r\nb\r\n", "a\r\nb\r\n").iter().all(|(k, _)| *k == '='), "CRLF against itself");
        assert_eq!(counts(&lines("a\r\n", "a\n")), (1, 1), "a changed line ending is a change");
    }

    #[test]
    fn diff_of_equal_texts_is_all_context() {
        let t = "one\ntwo\n\nthree\n";
        let d = lines(t, t);
        assert!(d.iter().all(|(k, _)| *k == '='));
        assert_eq!(d.len(), 4);
        assert!(hunks(&d, 3).is_empty());
        assert_eq!(counts(&d), (0, 0));
        assert!(lines("", "").is_empty());
    }

    #[test]
    fn a_huge_side_falls_back_to_all_removed_all_added() {
        let big = "x\n".repeat(MAX_LINES + 1);
        let d = lines(&big, "x\ny\n");
        assert_eq!(counts(&d), (2, MAX_LINES + 1));
    }

    #[test]
    fn diff_counts_match_git_numstat() {
        let old = "# Title\n\nalpha\nbeta\ngamma\ndelta\n\n- one\n- two\n- three\n\nend\n";
        let new = "# Title\n\nalpha\nBETA\ngamma\n\n- one\n- two\n- two and a half\n- three\n- four\n\nend\n";
        let d = lines(old, new);
        let (plus, minus) = counts(&d);
        let Some(git) = super::super::git::program() else { return };
        let dir = std::env::temp_dir().join(format!("echo-diff-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), old).unwrap();
        std::fs::write(dir.join("b.md"), new).unwrap();
        let o = std::process::Command::new(git).args(["diff", "--no-index", "--numstat", "a.md", "b.md"]).current_dir(&dir).output().unwrap();
        let s = String::from_utf8_lossy(&o.stdout).to_string();
        let mut f = s.split_whitespace();
        let (gp, gm): (usize, usize) = (f.next().unwrap().parse().unwrap(), f.next().unwrap().parse().unwrap());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((plus, minus), (gp, gm), "{s}");
    }
}
