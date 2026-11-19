//! The host's slower services (`__hostAsync`: a script, an HTTPS fetch, a sign-in, a delivery), run on a worker
//! thread — moved out of `jshost.rs` as they were.

use super::*;

pub(super) fn service_async(id: &str, op: &str, arg: &Value) -> Result<Value> {
    match op {
        // A script it declared (`uses.scripts`).
        "scripts.run" => {
            let args: Vec<String> = arg[2].as_array().into_iter().flatten().map(|a| a.as_str().map(str::to_owned).ok_or_else(|| anyhow!("a script's arguments are strings"))).collect::<Result<_>>()?;
            plugins::scripts::run(id, arg[0].as_str().unwrap_or(""), arg[1].as_str().unwrap_or(""), &args, arg[3]["timeout"].as_u64())
        }
        // A Claude Code run on one of the user's AI profiles: Echo's guarded run (`claudejob.rs`).
        "claude.run" => {
            // Only what its install sheet said: running Claude Code at all, and a run's own `skip` (else the user's
            // setting for routines).
            let uses = plugins::read_manifest(&package_dir(id)?)?.uses;
            let mut arg = arg.clone();
            arg["tools"] = plugins::may_run_claude(&uses, arg["tools"].as_str().unwrap_or(""))?.to_string().into();
            let mut spec = crate::claudejob::Spec::from_value(&arg, &plugins::data_dir(id))?;
            // Under a key of its own (`key`): one run at a time per key, stopped by `ctx.claude.stop(key)`; held until
            // the run has ended and its snapshot, if it failed, is gone.
            let keyed = match &arg["key"] {
                Value::Null => None,
                Value::String(k) => Some(crate::claudejob::Keyed::take(id, k)?),
                _ => bail!("a run's key is a string"),
            };
            spec.key = keyed.as_ref().map(|k| k.base.clone());
            // In a folder it was given (`folder`, `path`, `mode`): there, or in a snapshot of it (`copy`).
            let snapshot = folders::claude_folder(id, &arg, &mut spec)?;
            let ran = crate::claudejob::run(&spec);
            // A failed run's snapshot goes: the plugin never got its name.
            if let (Err(_), Some(s)) = (&ran, &snapshot) {
                if let Ok(d) = store_path(id, s["path"].as_str().unwrap_or("")) {
                    let _ = std::fs::remove_dir_all(d);
                }
            }
            let (answer, model, usage) = ran?;
            // What it changed and sent with Echo's CLI, for the plugin to show (a routine's outcome).
            let actions = crate::notes::run_actions(&spec.run_dir.join("actions.jsonl"));
            let mut out = json!({"answer": answer, "model": model, "usage": usage, "actions": actions});
            if let Some(s) = snapshot {
                out["snapshot"] = s;
            }
            Ok(out)
        }
        // Its own run under that key stopped (`claudejob::stop_keyed`): true, or false when none was running.
        "claude.stop" => {
            uses(id, "claude")?;
            Ok(json!(crate::claudejob::stop_keyed(id, arg.as_str().unwrap_or(""))?))
        }
        // The user's sign-in to a host its sheet named (`plugins::signin`), to the host half only.
        "signin.get" => {
            let host = arg[0].as_str().unwrap_or("");
            plugins::may_use_signin(&plugins::read_manifest(&package_dir(id)?)?.uses, host)?;
            let own = plugins::may_keep_signin(&plugins::read_manifest(&package_dir(id)?)?.uses).then_some(id);
            Ok(plugins::signin::get(host, arg[1].as_bool().unwrap_or(false), own).unwrap_or(Value::Null))
        }
"http.fetch" => fetch(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        // A WAV file it was handed, on Echo's speech model (`speech.rs`).
        "speech.transcribe" => speech::transcribe(id, &arg[0], &arg[1]),
        // A web page as Markdown, a file as a handle, HTML it has read the same way (`web.rs`).
        "web.read" => web::read(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "web.fetch" => web::fetch(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "web.markdown" => web::markdown(arg[0].as_str().unwrap_or(""), &arg[1]),
        // Echo's own box asks the user for an address this plugin may fetch once (`uses.network.ask`).
        "http.ask_address" => typed::ask(id),
        // The user's Microsoft 365 through Echo's sign-in, read-only (`uses.m365.read`).
        "m365.search" => m365::search(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "m365.mail_thread" => m365::mail_thread(id, arg.as_str().unwrap_or("")),
        "m365.chat" => m365::chat(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "system.pick_folder" => {
            uses(id, "folders")?;
            files::no_picker_in_a_run()?;
            let name = plugins::enabled().into_iter().find(|p| p.id == id).map(|p| p.name).unwrap_or_else(|| id.to_string());
            let prompt: String = format!("{} — {}", name, arg.as_str().unwrap_or("Choose a folder")).chars().take(200).collect();
            Ok(match crate::app::choose_folder(&prompt) {
                Some(dir) => json!(crate::grants::grant(id, &dir)?.display().to_string()),
                None => Value::Null,
            })
        }
        "system.pick_files" => files::pick(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "fs.copy_in" => files::copy_in(id, &arg[0], arg[1].as_str().unwrap_or(""), arg[2].as_str().unwrap_or(""), arg[3]["createNew"].as_bool().unwrap_or(true)),
        "fs.read_many" => {
            uses(id, "folders")?;
            let root = arg[0].as_str().unwrap_or("").to_string();
            let max = arg[2].as_u64().filter(|m| *m > 0).unwrap_or(1 << 20).min(4 << 20);
            let strict = arg[3].as_bool() == Some(true);
            let mut out = vec![];
            for rel in arg[1].as_array().into_iter().flatten().take(5000) {
                let rel = rel.as_str().unwrap_or("");
                // A file it can't read — with `strict` one that isn't UTF-8 too — is `null`.
                let text = crate::grants::resolve(id, &root, rel).ok().and_then(|f| folders::read_text(&f, rel, max, strict).ok());
                out.push(json!(text));
            }
            Ok(json!(out))
        }
        "fs.walk" => folders::walk(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "store.walk" => folders::store_walk(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "fs.search" => folders::search(id, arg[0].as_str().unwrap_or(""), arg[1].as_str().unwrap_or(""), &arg[2]),
        "fs.snapshot" => folders::snapshot(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "git.info" => git::info(id, arg[0].as_str().unwrap_or("")),
        "git.status" => git::status(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "git.log" => git::log(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "git.show" => git::show(id, arg[0].as_str().unwrap_or(""), arg[1].as_str().unwrap_or(""), arg[2].as_str().unwrap_or(""), &arg[3]),
        "git.init" => git::init(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "git.commit" => git::commit(id, arg[0].as_str().unwrap_or(""), &arg[1]),
        "claude.has_plugin" | "claude.install_plugin" => {
            let spec = arg[0].as_str().unwrap_or("");
            if !spec.contains('@') || !spec.bytes().all(|b| b.is_ascii_alphanumeric() || b"@._-".contains(&b)) {
                bail!("“{spec}” isn't a Claude Code plugin (name@marketplace)");
            }
            // Only the channel plugin its install sheet named; installing (it changes the user's Claude Code) only into
            // the default profile's.
            may_touch_claude_plugin(id, spec)?;
            let profile = if op == "claude.install_plugin" {
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
