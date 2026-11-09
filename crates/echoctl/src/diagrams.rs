//! A minutes diagram mermaid can't parse (#199): the minutes page says which block and mermaid's error, and the call's
//! own AI profile repairs that block alone — it gets the diagram and the error, nothing else of the call — once per
//! diagram, then the page draws it again. A repair that still fails stays as its source with the error.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Result};
use sha2::{Digest, Sha256};

const SYSTEM: &str = "You fix one Mermaid diagram that Mermaid can't parse. You get its source and Mermaid's parse \
error. Answer with the corrected diagram's source only: no code fence, no comment, no explanation. Keep what it shows \
(its nodes, edges, labels and order); change only what makes it fail, typically by quoting a label that holds \
brackets, parentheses, quotes or other special characters (A[\"Budget (Q3)\"]). Never add an %%{init}%% directive, \
a front-matter block, a click or a link.";

/// The diagrams tried already, by call and source: a diagram is repaired once, and a repair that fails again isn't.
static TRIED: Mutex<Option<HashSet<(PathBuf, String)>>> = Mutex::new(None);

fn hash(src: &str) -> String {
    format!("{:x}", Sha256::digest(src.trim().as_bytes()))
}

/// Seen for the first time (and now marked tried)?
fn first_try(dir: &Path, src: &str) -> bool {
    let mut g = TRIED.lock().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(HashSet::new).insert((dir.to_path_buf(), hash(src)))
}

/// One write at a time: two repairs of one call's minutes never drop each other's.
static WRITING: Mutex<()> = Mutex::new(());

/// The `index`-th diagram of `md`, numbered as the page draws them (`minutes::mermaid_blocks`): its source's byte
/// range and text. None: no such diagram, or one that isn't one piece of the file (inside a quote).
fn block(md: &str, index: usize) -> Option<(std::ops::Range<usize>, String)> {
    let b = crate::minutes::mermaid_blocks(md).into_iter().nth(index).filter(|b| b.whole)?;
    let src = md[b.range.clone()].to_string();
    Some((b.range, src))
}

/// The model's answer as a diagram's source: a fence it added anyway taken off; no directive or front matter.
fn answer_source(answer: &str) -> Result<String> {
    let t = answer.trim();
    let t = t.strip_prefix("```mermaid").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    let t = t.strip_suffix("```").unwrap_or(t).trim();
    if t.is_empty() || t.contains("```") || t.contains("%%{") || t.starts_with("---") {
        bail!("the answer isn't one diagram");
    }
    Ok(t.to_string())
}

/// Repair the `index`-th diagram of a call's minutes, which failed with `error`, on the call's own profile. Ok(false):
/// nothing done (tried already, a minutes run is writing them, the block is gone or changed).
pub fn repair(dir: &Path, index: usize, error: &str) -> Result<bool> {
    let path = dir.join("notes.md");
    let run_on = || crate::notes::status(dir).is_some_and(|r| crate::notes::pending(&r));
    let Some((_, src)) = block(&fs::read_to_string(&path)?, index) else { return Ok(false) };
    if run_on() || !first_try(dir, &src) {
        return Ok(false);
    }
    let error: String = error.chars().take(600).collect();
    let ready = crate::ai::for_call(dir)?;
    let user = format!("Mermaid's error:\n{error}\n\nThe diagram:\n{}", src.trim_end());
    let fixed = answer_source(&crate::livenotes::complete_text(ready.profile(), SYSTEM, &user, 2000)?)?;
    first_try(dir, &fixed); // a repair that fails again isn't repaired again
    // The minutes as they are now: the block replaced only if it is still the one that failed, and no run writes them.
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let md = fs::read_to_string(&path)?;
    match block(&md, index) {
        Some((range, now)) if now == src && !run_on() => {
            let out = format!("{}{fixed}\n{}", &md[..range.start], &md[range.end..]);
            crate::paths::write_atomic(&path, out.as_bytes())?;
            eprintln!("{}: diagram {} repaired", dir.display(), index + 1);
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD: &str = "# Minutes\n\nText.\n\n```mermaid\nflowchart TD\n  A[Budget (Q3)] --> B\n```\n\nMore.\n\n```mermaid\nsequenceDiagram\n  A->>B: Hi\n```\n";

    #[test]
    fn blocks_are_found_in_the_order_the_page_draws_them() {
        assert_eq!(block(MD, 0).unwrap().1, "flowchart TD\n  A[Budget (Q3)] --> B\n");
        assert_eq!(block(MD, 1).unwrap().1, "sequenceDiagram\n  A->>B: Hi\n");
        assert!(block(MD, 2).is_none());
        assert!(block("```python\nx\n```\n", 0).is_none());
        // As the page counts them: any case, attributes, ~~~ and longer fences; none inside another block.
        let odd = "```Mermaid\nA\n```\n\n~~~mermaid {t}\nB\n~~~\n\n````markdown\n```mermaid\nnot one\n```\n````\n\n````mermaid\nC\n````\n";
        let srcs: Vec<String> = (0..4).filter_map(|i| block(odd, i).map(|b| b.1)).collect();
        assert_eq!(srcs, ["A\n", "B\n", "C\n"]);
        // In a quote its lines carry `>`: counted (the page draws it) but never rewritten.
        let quoted = "> ```mermaid\n> flowchart TD\n>   A --> B\n> ```\n\n```mermaid\nD\n```\n";
        assert!(block(quoted, 0).is_none());
        assert_eq!(block(quoted, 1).unwrap().1, "D\n");
    }

    #[test]
    fn an_answer_is_one_diagram_without_a_directive() {
        assert_eq!(answer_source("```mermaid\nflowchart TD\n  A[\"x (y)\"] --> B\n```").unwrap(), "flowchart TD\n  A[\"x (y)\"] --> B");
        assert_eq!(answer_source("  flowchart TD\n  A --> B  ").unwrap(), "flowchart TD\n  A --> B");
        assert!(answer_source("%%{init: {'securityLevel': 'loose'}}%%\nflowchart TD\n A-->B").is_err());
        assert!(answer_source("---\ntitle: x\n---\nflowchart TD").is_err());
        assert!(answer_source("```mermaid\nA\n```\n```mermaid\nB\n```").is_err());
        assert!(answer_source("  ").is_err());
    }

    #[test]
    fn a_broken_diagram_is_repaired_once_on_the_calls_profile_and_only_that_block_changes() {
        let (_env, root) = crate::paths::testenv::folders("diagram-repair");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("w", None, &crate::ai::fake::signed_in("pat@example.com"));
        let dir = crate::paths::home().join("calls/2026-11-09_120000-x");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("meta.json"), r#"{"title":"x","ai_profile":"w"}"#).unwrap();
        fs::write(dir.join("notes.md"), MD).unwrap();
        let reply = r#"{"type":"result","is_error":false,"result":"```mermaid\nflowchart TD\n  A[\"Budget (Q3)\"] --> B\n```"}"#;
        fs::write(crate::ai::fake::file(&p, "p.reply"), reply).unwrap();
        assert!(repair(&dir, 0, "Parse error on line 2: … got 'PS'").unwrap());
        let md = fs::read_to_string(dir.join("notes.md")).unwrap();
        assert_eq!(md, MD.replace("A[Budget (Q3)]", "A[\"Budget (Q3)\"]"), "only that block, the rest as it was");
        assert!(!repair(&dir, 0, "again").unwrap(), "its repair failing too: not tried again");
        let prompt = fs::read_to_string(crate::ai::fake::file(&p, "p.stdin")).unwrap();
        assert!(prompt.contains("A[Budget (Q3)]") && prompt.contains("got 'PS'"), "the diagram and its error");
        assert!(!prompt.contains("More.") && !prompt.contains("sequenceDiagram"), "nothing else of the call goes to the model");
    }
}
