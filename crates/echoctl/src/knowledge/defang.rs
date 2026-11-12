//! A vault's skills and commands as Echo puts them into a plugin of its own — an ingest run's (`ingest::plugin`), a
//! managed profile's conversations' (`terminal::vault_plugin`) — in Echo's copy only, never the vault's own files
//! (final review F1). Claude Code honours more in a skill's own files than its words: hooks in its frontmatter run
//! commands, `allowed-tools` and an agent's `permissionMode` widen what it may do, `` !`cmd` `` runs a command before
//! the model reads the text, and hook files run commands too. Echo leaves those out of its copy and says what it left
//! out (`LeftOut::said`).

use anyhow::{Context as _, Result};
use std::path::Path;

use super::detect::{self, Skill};

/// A file of a plugin: its path in the plugin, its contents.
pub(crate) type File = (String, Vec<u8>);

/// What Echo left out of a skill's files, in its copy.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct LeftOut {
    /// Hooks: a frontmatter's `hooks`, a `hooks/` folder or a `hooks.json`.
    pub hooks: bool,
    /// A frontmatter's `allowed-tools`.
    pub tools: bool,
    /// A frontmatter's `permissionMode`.
    pub mode: bool,
    /// A frontmatter Echo can't read safely: it goes as text, not as frontmatter.
    pub unread: bool,
    /// `` !`cmd` `` commands it would run first.
    pub commands: usize,
    /// Markdown files that aren't UTF-8: left out.
    pub not_text: usize,
}

impl LeftOut {
    fn add(&mut self, o: &LeftOut) {
        self.hooks |= o.hooks;
        self.tools |= o.tools;
        self.mode |= o.mode;
        self.unread |= o.unread;
        self.commands += o.commands;
        self.not_text += o.not_text;
    }

    /// What was left out, in a line ("its hooks; its allowed tools; 2 commands it would run first"); None: nothing.
    pub(crate) fn said(&self) -> Option<String> {
        let s = |n: usize| if n == 1 { "" } else { "s" };
        let mut parts: Vec<String> = vec![];
        for (on, what) in [(self.hooks, "its hooks"), (self.tools, "its allowed tools"), (self.mode, "its permission mode"), (self.unread, "its frontmatter, which Echo can't read (kept as text)")] {
            if on {
                parts.push(what.into());
            }
        }
        if self.commands > 0 {
            parts.push(format!("{} command{} it would run first", self.commands, s(self.commands)));
        }
        if self.not_text > 0 {
            parts.push(format!("{} Markdown file{} that isn't UTF-8", self.not_text, s(self.not_text)));
        }
        (!parts.is_empty()).then(|| parts.join("; "))
    }
}

/// Which key of a frontmatter widens a run: `hooks`, `allowed-tools`, `permissionMode` — in any case, with or without
/// `-` and `_`.
fn widening(key: &str, out: &mut LeftOut) -> bool {
    match key.to_lowercase().replace(['-', '_'], "").as_str() {
        "hooks" => out.hooks = true,
        "allowedtools" => out.tools = true,
        "permissionmode" => out.mode = true,
        _ => return false,
    }
    true
}

/// A top-level key of a frontmatter line and what follows its colon: plain (`key: …`, `key:`), or quoted without
/// escapes (`"key": …`, `'key': …`). None: a line that isn't one Echo reads safely (a flow mapping, a sequence, a
/// complex key, an anchor, a tag, a merge key `<<`, a quoted key with an escape).
fn key_of(line: &str) -> Option<(String, &str)> {
    let first = line.chars().next()?;
    let (key, value) = match first {
        '"' | '\'' => {
            let inner = &line[1..];
            let end = inner.find(first)?;
            let (key, rest) = (&inner[..end], &inner[end + 1..]);
            if key.contains('\\') || rest.starts_with(first) {
                return None;
            }
            (key.to_string(), rest.trim_start().strip_prefix(':')?)
        }
        c if "{}[]?-&*!%|>@`,#:".contains(c) => return None,
        _ => {
            let b = line.as_bytes();
            let at = (0..b.len()).find(|&i| b[i] == b':' && b.get(i + 1).is_none_or(|n| *n == b' ' || *n == b'\t'))?;
            (line[..at].trim_end().to_string(), &line[at + 1..])
        }
    };
    (!key.is_empty() && key != "<<").then_some((key, value))
}

/// `text` without the frontmatter keys that widen a run (`widening`). The frontmatter is taken as widely as any reader
/// could: from a first line that is `---` (spaces aside, a BOM before it) to the next line that is exactly `---` — or
/// to the end. A dropped key takes the lines under it: indented ones, and a list at its own column. One Echo can't read
/// safely (`key_of`, a line indented before any key, a list after a key with a value) loses its opening line instead,
/// so that no reader takes it for frontmatter; its words stay as text.
fn frontmatter(text: &str, out: &mut LeftOut) -> String {
    let (bom, rest) = match text.strip_prefix('\u{feff}') {
        Some(r) => ("\u{feff}", r),
        None => ("", text),
    };
    let mut lines = rest.split_inclusive('\n');
    let Some(open) = lines.next().filter(|l| l.trim() == "---") else { return text.to_string() };
    let mut kept = format!("{bom}{open}");
    let mut found = LeftOut::default();
    // None before the first key; then whether the entry going on is dropped, and whether its key's line holds no value
    // (a list may follow at the key's column).
    let mut dropping: Option<bool> = None;
    let mut open_key = false;
    let mut closed = false;
    for line in lines.by_ref() {
        let bare = line.trim_end_matches(['\r', '\n']);
        if bare == "---" {
            kept.push_str(line);
            closed = true;
            break;
        }
        let keep = if bare.trim().is_empty() || bare.starts_with('#') {
            dropping != Some(true) || bare.starts_with('#')
        } else if bare.starts_with([' ', '\t']) {
            match dropping {
                Some(d) => !d,
                None => return unread(bom, open, rest, out),
            }
        } else if bare == "-" || bare.starts_with("- ") {
            // A list at its key's column (PyYAML's layout): it goes with its key (final review X3).
            match dropping {
                Some(true) => false,
                Some(false) if open_key => true,
                _ => return unread(bom, open, rest, out),
            }
        } else {
            match key_of(bare) {
                Some((k, value)) => {
                    let d = widening(&k, &mut found);
                    dropping = Some(d);
                    open_key = value.trim().is_empty() || value.trim_start().starts_with('#');
                    !d
                }
                None => return unread(bom, open, rest, out),
            }
        };
        if keep {
            kept.push_str(line);
        }
    }
    if closed {
        lines.for_each(|l| kept.push_str(l));
    }
    out.add(&found);
    kept
}

/// A frontmatter Echo can't read: its opening line becomes an empty one (its line ending kept), so no reader takes what
/// follows for frontmatter.
fn unread(bom: &str, open: &str, rest: &str, out: &mut LeftOut) -> String {
    out.unread = true;
    let ending = &open[open.trim_end_matches(['\r', '\n']).len()..];
    format!("{bom}{ending}{}", &rest[open.len()..])
}

/// `text` without Claude Code's preprocessing: every run of `!` before a backtick (`` !`cmd` ``, `` !!`cmd` ``) and
/// after a fence's opening (```` ```! ````, `~~~!` at a line's start) goes, so the words read the same and nothing
/// runs; again until none is left, as what is left joins up (`` ``!`! `` is ```` ```! ```` once its first `!` goes).
/// How many places went: a run counts once, however many `!` (final review X1).
fn no_commands(text: &str, n: &mut usize) -> String {
    let mut text = text.to_string();
    loop {
        let chars: Vec<char> = text.chars().collect();
        let (mut out, mut went, mut i) = (String::with_capacity(text.len()), 0, 0);
        while i < chars.len() {
            if chars[i] != '!' {
                out.push(chars[i]);
                i += 1;
                continue;
            }
            let end = (i..chars.len()).find(|&j| chars[j] != '!').unwrap_or(chars.len());
            if chars.get(end) == Some(&'`') || opens_fence(&out) {
                went += 1;
            } else {
                out.extend(&chars[i..end]);
            }
            i = end;
        }
        *n += went;
        if went == 0 {
            return out;
        }
        text = out;
    }
}

/// Does `out` end in a fence's opening: three or more backticks, or three or more tildes at a line's start (spaces
/// before them aside)?
fn opens_fence(out: &str) -> bool {
    if out.chars().rev().take_while(|c| *c == '`').count() >= 3 {
        return true;
    }
    let tildes = out.chars().rev().take_while(|c| *c == '~').count();
    tildes >= 3 && out[..out.len() - tildes].rsplit('\n').next().is_some_and(|l| l.chars().all(|c| c == ' '))
}

/// A Markdown file of a skill as Echo copies it: its frontmatter without `hooks`, `allowed-tools` and `permissionMode`,
/// and no `` !`cmd` `` preprocessing. What was left out.
pub(crate) fn markdown(text: &str) -> (String, LeftOut) {
    let mut out = LeftOut::default();
    let text = frontmatter(text, &mut out);
    let text = no_commands(&text, &mut out.commands);
    (text, out)
}

fn is_md(rel: &str) -> bool {
    rel.rsplit('.').next().is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// A hook file of a skill's folder: anything under a `hooks/` folder, or a `hooks.json` (in any case).
fn hook_file(rel: &str) -> bool {
    let parts: Vec<String> = rel.split('/').map(str::to_lowercase).collect();
    parts.last().is_some_and(|l| l == "hooks.json") || parts[..parts.len().saturating_sub(1)].iter().any(|p| p == "hooks")
}

/// Skill `s` of the vault at `root` (or of a run's copy of it) as a plugin holds it: `(path in the plugin, contents)` —
/// a command as `commands/<file>`, a skill's folder as `skills/<folder>/…` without its hook files, each Markdown file
/// through `markdown`, a file over `max` bytes left out — and what was left out.
pub(crate) fn skill_files(root: &Path, s: &Skill, max: u64) -> Result<(Vec<File>, LeftOut)> {
    let src = super::join(root, &s.path);
    let mut left = LeftOut::default();
    let mut todo: Vec<(String, std::path::PathBuf)> = vec![];
    if s.kind == "command" {
        todo.push((format!("commands/{}", s.path.strip_prefix(".claude/commands/").unwrap_or_else(|| src.file_name().unwrap_or_default().to_str().unwrap_or(""))), src));
    } else {
        let folder = src.parent().ok_or_else(|| anyhow::anyhow!("{}: not a skill's file", s.path))?;
        let name = folder.file_name().unwrap_or_default().to_string_lossy().into_owned();
        for rel in detect::walk(folder, &[], |_| true) {
            if hook_file(&rel) {
                left.hooks = true;
                continue;
            }
            todo.push((format!("skills/{name}/{rel}"), super::join(folder, &rel)));
        }
    }
    let mut files = vec![];
    for (rel, from) in todo {
        if std::fs::metadata(&from).map(|m| m.len()).unwrap_or(0) > max {
            continue;
        }
        let bytes = std::fs::read(&from).with_context(|| format!("the skill's {rel}"))?;
        if !is_md(&rel) {
            files.push((rel, bytes));
            continue;
        }
        match String::from_utf8(bytes) {
            Ok(text) => {
                let (text, out) = markdown(&text);
                left.add(&out);
                files.push((rel, text.into_bytes()));
            }
            Err(_) => left.not_text += 1,
        }
    }
    Ok((files, left))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(text: &str) -> (String, Option<String>) {
        let (t, l) = markdown(text);
        (t, l.said())
    }

    #[test]
    fn the_keys_that_widen_a_run_go_and_the_rest_stays() {
        let (t, s) = said("---\nname: x\ndescription: |\n  hooks: only words here\nallowed-tools: Bash(*)\n# a comment\nhooks:\n  Stop:\n    - hooks: []\n\nmodel: haiku\n---\nBody: hooks: stay.\n");
        assert_eq!(t, "---\nname: x\ndescription: |\n  hooks: only words here\n# a comment\nmodel: haiku\n---\nBody: hooks: stay.\n");
        assert_eq!(s.as_deref(), Some("its hooks; its allowed tools"));
        // Quoted keys, other spellings, CRLF, a BOM.
        let (t, s) = said("\u{feff}---\r\n\"hooks\": {Stop: []}\r\n'allowed_tools': [Bash]\r\nPermissionMode : bypassPermissions\r\nname: y\r\n---\r\nText\r\n");
        assert_eq!(t, "\u{feff}---\r\nname: y\r\n---\r\nText\r\n");
        assert_eq!(s.as_deref(), Some("its hooks; its allowed tools; its permission mode"));
        // Nothing to leave out: as it was.
        assert_eq!(said("---\nname: z\n---\n# Z\n"), ("---\nname: z\n---\n# Z\n".to_string(), None));
        assert_eq!(said("# No frontmatter\nhooks: words\n"), ("# No frontmatter\nhooks: words\n".to_string(), None));
    }

    #[test]
    fn a_list_at_the_keys_column_goes_with_its_key() {
        // Final review X3: PyYAML writes a key's list at the key's own column.
        let (t, s) = said("---\nname: inbox-steps\ndescription: Process the inbox\nallowed-tools:\n- Read\n- Bash(*)\nhooks:\n- matcher: \"*\"\n  hooks: []\ntags:\n- skill\n---\nBody\n");
        assert_eq!(t, "---\nname: inbox-steps\ndescription: Process the inbox\ntags:\n- skill\n---\nBody\n");
        assert_eq!(s.as_deref(), Some("its hooks; its allowed tools"));
        // A list after a key that has its value already is no YAML Echo reads: as text.
        let odd = "---\nname: x\n- a\n---\nBody\n";
        assert_eq!(said(odd).0, format!("\n{}", &odd[4..]));
    }

    #[test]
    fn a_frontmatter_echo_cant_read_goes_as_text() {
        // A merge key, a flow mapping, an indented mapping, a key with an escape: no reader takes it for frontmatter.
        for text in [
            "---\nbase: &b\n  hooks: {}\n<<: *b\n---\n# X\n",
            "---\n{name: x, hooks: {}}\n---\n# X\n",
            "---\n  name: x\n  hooks: {}\n---\n# X\n",
            "---\n\"ho\\x6fks\": {}\n---\n# X\n",
            "---\nhooks:x\n---\n# X\n",
        ] {
            let (t, s) = said(text);
            assert!(t.starts_with('\n') && t[1..] == text[4..], "{text:?} → {t:?}");
            assert_eq!(s.as_deref(), Some("its frontmatter, which Echo can't read (kept as text)"), "{text:?}");
        }
        // A frontmatter never closed: read to the end, as a reader that ends it at the next `---` line would.
        assert_eq!(said("---\nname: x\nhooks: {}\n").0, "---\nname: x\n");
        // A closing line with a space: some readers end there, others read on — Echo can't tell, so it is text.
        let odd = "---\nname: x\n--- \nhooks: {}\n---\nBody\n";
        assert_eq!(said(odd).0, format!("\n{}", &odd[4..]));
    }

    #[test]
    fn preprocessing_commands_are_words_again() {
        let (t, l) = markdown("Now: !`git status` and !`date`.\n```!\nrm -rf x\n```\n  ~~~!\nls\n~~~\nKeep `code` and a ! mark.\n");
        assert_eq!(t, "Now: `git status` and `date`.\n```\nrm -rf x\n```\n  ~~~\nls\n~~~\nKeep `code` and a ! mark.\n");
        assert_eq!(l.commands, 4);
        assert_eq!(l.said().as_deref(), Some("4 commands it would run first"));
    }

    #[test]
    fn a_run_of_bangs_is_one_command_and_none_is_left() {
        // Final review X1: doubling the `!` must not bring a command back.
        for (text, want, n) in [
            ("Today: !!`echo hi`\n", "Today: `echo hi`\n", 1),
            ("a !!!`x` and !!`y`\n", "a `x` and `y`\n", 2),
            ("```!!\nrm -rf x\n```\n", "```\nrm -rf x\n```\n", 1),
            ("  ~~~!!!\nls\n~~~\n", "  ~~~\nls\n~~~\n", 1),
            ("!```!!\nls\n```\n", "```\nls\n```\n", 2),
        ] {
            let (t, l) = markdown(text);
            assert_eq!((t.as_str(), l.commands), (want, n), "{text:?}");
        }
        // Whatever is left joins up: no `!` before a backtick, no fence opened with `!`, anywhere.
        for text in ["``!`!x\n", "`!!`!`\n", "!!!!!!!!`\n", "~~~!~~~!\n", "```!```!!`\n", "x !`!`!`!` y\n", "   ```!!!!\n"] {
            let (t, _) = markdown(text);
            assert!(!t.contains("!`"), "{text:?} → {t:?}");
            for line in t.lines() {
                let body = line.trim_start_matches(' ');
                let fence = body.chars().take_while(|c| *c == '`').count().max(body.chars().take_while(|c| *c == '~').count());
                assert!(fence < 3 || !body[fence..].starts_with('!'), "{text:?} → {t:?}");
                assert!(!line.contains("```!"), "{text:?} → {t:?}");
            }
        }
    }

    #[test]
    fn hook_files_are_left_out_and_said() {
        let root = std::env::temp_dir().join(format!("echo-defang-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (rel, text) in [("SKILL.md", "---\nname: s\n---\n!`x`\n"), ("hooks/hooks.json", "{}"), ("Hooks.json", "{}"), ("scripts/run.sh", "echo x"), ("notes/hooks.md", "words")] {
            let p = root.join(".claude/skills/s").join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        let s = Skill { name: "s".into(), description: String::new(), path: ".claude/skills/s/SKILL.md".into(), kind: "skill".into() };
        let (files, left) = skill_files(&root, &s, u64::MAX).unwrap();
        let names: Vec<&str> = files.iter().map(|(r, _)| r.as_str()).collect();
        assert_eq!(names, ["skills/s/SKILL.md", "skills/s/notes/hooks.md", "skills/s/scripts/run.sh"]);
        assert_eq!(files[0].1, b"---\nname: s\n---\n`x`\n");
        assert_eq!(left.said().as_deref(), Some("its hooks; 1 command it would run first"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
