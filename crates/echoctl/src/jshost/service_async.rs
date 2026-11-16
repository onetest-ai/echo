//! The host's slower services (`__hostAsync`: a script, an HTTPS fetch, a sign-in, a delivery), run on a worker
//! thread — moved out of `jshost.rs` as they were.

use super::*;

pub(super) fn service_async(id: &str, op: &str, arg: &Value) -> Result<Value> {
    match op {
        // A script it declared (`uses.scripts`): a read-only routine's host runs only one marked readonly.
        "scripts.run" => {
            let args: Vec<String> = arg[2].as_array().into_iter().flatten().map(|a| a.as_str().map(str::to_owned).ok_or_else(|| anyhow!("a script's arguments are strings"))).collect::<Result<_>>()?;
            let read_only = std::env::var(crate::guard::RUN_MODE).is_ok_and(|m| m == "routine");
            plugins::scripts::run(id, arg[0].as_str().unwrap_or(""), arg[1].as_str().unwrap_or(""), &args, arg[3]["timeout"].as_u64(), read_only)
        }
        // A Claude Code run on one of the user's AI profiles: Echo's guarded run (`claudejob.rs`).
        "claude.run" => {
            read_only_here("a Claude run")?;
            // Only what its install sheet said: running Claude Code at all, and with permission checks skipped.
            plugins::may_run_claude(&plugins::read_manifest(&package_dir(id)?)?.uses, arg["tools"].as_str().unwrap_or(""))?;
            let spec = crate::claudejob::Spec::from_value(arg, &plugins::data_dir(id))?;
            let (answer, model, usage) = crate::claudejob::run(&spec)?;
            // What it changed and sent with Echo's CLI, for the plugin to show (a routine's outcome).
            let actions = crate::notes::run_actions(&spec.run_dir.join("actions.jsonl"));
            Ok(json!({"answer": answer, "model": model, "usage": usage, "actions": actions}))
        }
        // The user's sign-in to a host its sheet named (`plugins::signin`), to the host half only.
        "signin.get" => {
            let host = arg[0].as_str().unwrap_or("");
            plugins::may_use_signin(&plugins::read_manifest(&package_dir(id)?)?.uses, host)?;
            read_only_here("reading your sign-in")?;
            let own = plugins::may_keep_signin(&plugins::read_manifest(&package_dir(id)?)?.uses).then_some(id);
            Ok(plugins::signin::get(host, arg[1].as_bool().unwrap_or(false), own).unwrap_or(Value::Null))
        }
"http.fetch" => fetch(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "system.pick_folder" => {
            uses(id, "folders")?;
            let name = plugins::enabled().into_iter().find(|p| p.id == id).map(|p| p.name).unwrap_or_else(|| id.to_string());
            let prompt: String = format!("{} — {}", name, arg.as_str().unwrap_or("Choose a folder")).chars().take(200).collect();
            Ok(match crate::app::choose_folder(&prompt) {
                Some(dir) => json!(crate::grants::grant(id, &dir)?.display().to_string()),
                None => Value::Null,
            })
        }
        "fs.read_many" => {
            uses(id, "folders")?;
            let root = arg[0].as_str().unwrap_or("").to_string();
            let max = arg[2].as_u64().filter(|m| *m > 0).unwrap_or(1 << 20).min(4 << 20);
            let mut out = vec![];
            for rel in arg[1].as_array().into_iter().flatten().take(5000) {
                let text = crate::grants::resolve(id, &root, rel.as_str().unwrap_or("")).ok().and_then(|f| {
                    use std::io::Read;
                    let mut buf = vec![];
                    std::fs::File::open(f).ok()?.take(max).read_to_end(&mut buf).ok()?;
                    Some(String::from_utf8_lossy(&buf).into_owned())
                });
                out.push(json!(text));
            }
            Ok(json!(out))
        }
        "fs.walk" => {
            uses(id, "folders")?;
            let root = crate::grants::resolve(id, arg[0].as_str().unwrap_or(""), "")?;
            let o = &arg[1];
            let base = match o["path"].as_str().filter(|p| !p.is_empty()) {
                Some(rel) => crate::grants::resolve(id, &root.display().to_string(), rel)?,
                None => root.clone(),
            };
            let hidden = o["hidden"].as_bool().unwrap_or(false);
            let max = o["max"].as_u64().filter(|m| *m > 0).unwrap_or(20_000).min(100_000) as usize;
            let exts: Vec<String> = o["ext"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|e| e.trim_start_matches('.').to_lowercase())).collect()).unwrap_or_default();
            let mut out = vec![];
            let mut stack = vec![base];
            while let Some(dir) = stack.pop() {
                // A folder swapped for a link since it was listed: not read.
                if !dunce::canonicalize(&dir).is_ok_and(|real| real.starts_with(&root)) {
                    continue;
                }
                let Ok(rd) = std::fs::read_dir(&dir) else { continue };
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    let Ok(t) = e.file_type() else { continue };
                    // Links aren't followed: one could lead out of the folder.
                    if t.is_symlink() || (!hidden && name.starts_with('.')) {
                        continue;
                    }
                    let p = e.path();
                    if t.is_dir() {
                        stack.push(p);
                    } else if exts.is_empty() || p.extension().is_some_and(|x| exts.contains(&x.to_string_lossy().to_lowercase())) {
                        let m = e.metadata().ok();
                        let rel = p.strip_prefix(&root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                        let mtime = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                        out.push(json!({"path": rel, "size": m.map(|m| m.len()).unwrap_or(0), "mtime": mtime}));
                        if out.len() >= max {
                            return Ok(json!(out));
                        }
                    }
                }
            }
            Ok(json!(out))
        }
        "claude.has_plugin" | "claude.install_plugin" => {
            let spec = arg[0].as_str().unwrap_or("");
            if !spec.contains('@') || !spec.bytes().all(|b| b.is_ascii_alphanumeric() || b"@._-".contains(&b)) {
                bail!("“{spec}” isn't a Claude Code plugin (name@marketplace)");
            }
            // Only the channel plugin its install sheet named; installing (it changes the user's Claude Code) never from
            // a read-only routine, and only into the default profile's.
            may_touch_claude_plugin(id, spec)?;
            let profile = if op == "claude.install_plugin" {
                read_only_here("installing")?;
                crate::ai::default_id()
            } else {
                Some(arg[1].as_str().unwrap_or("")).filter(|p| !p.is_empty()).map(str::to_owned).unwrap_or_else(crate::ai::default_id)
            };
            let ready = crate::ai::ready(&profile)?;
            let args: Vec<String> = if op == "claude.has_plugin" { vec!["plugin".into(), "list".into(), "--json".into()] } else { vec!["plugin".into(), "install".into(), spec.into(), "--scope".into(), "user".into()] };
            let out = crate::ai::claude_command(ready.profile(), &args, &paths::home(), false)?.wait_with_output()?;
            let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            if op == "claude.has_plugin" {
                // On and seen from the data folder (where Echo's sessions run): user scope, or this folder's project.
                let list: Vec<Value> = serde_json::from_slice(&out.stdout).map_err(|_| anyhow!("Claude Code's plugin list can't be read"))?;
                let home = dunce::canonicalize(paths::home()).unwrap_or_else(|_| paths::home());
                let usable = list.iter().any(|x| {
                    x["id"] == spec
                        && x["enabled"] == true
                        && (x["scope"] == "user" || x["projectPath"].as_str().and_then(|p| dunce::canonicalize(p).ok()).is_some_and(|p| p == home))
                });
                return Ok(json!(usable));
            }
            if !out.status.success() {
                bail!("Claude Code couldn't install {spec}: {}", text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no reason given").trim());
            }
            Ok(Value::Null)
        }
        "deliver.send" => {
            read_only_here("sending")?;
            let kind = arg[0].as_str().unwrap_or("");
            // Only the ways its install sheet named, or its own.
            plugins::may_send(&plugins::read_manifest(&package_dir(id)?)?.uses, kind)?;
            let m = &arg[1];
            let s = |k: &str| m[k].as_str().map(str::to_owned);
            let to: Vec<String> = m["mail_to"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default();
            let (title, markdown, about, ai) = (s("title").unwrap_or_default(), s("markdown").unwrap_or_default(), s("about"), s("ai"));
            crate::deliver::ready(kind)?;
            let sent = crate::deliver::send(kind, &crate::deliver::Msg { title: &title, markdown: &markdown, about: about.as_deref(), ai: ai.as_deref(), mail_to: &to });
            // Inside a routine's or Yolo's run (its plugin call): the send is the run's, noted for its outcome.
            crate::notes::note("send", &format!("plugin {id}: deliver by {kind} “{}”", title.chars().take(120).collect::<String>()), sent.as_ref().err().map(|e| format!("failed: {e:#}")).as_deref());
            sent?;
            Ok(Value::Null)
        }
        o => bail!("Echo has no service “{o}”"),
    }
}
