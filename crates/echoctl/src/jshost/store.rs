//! The plugin's store kept tidy — moved out of `jshost.rs` as it was.

use super::*;

/// When a host half starts: what lay in its store's `cache/` over 7 days, and downloads left half-written (`.part`,
/// over an hour: a fetch takes at most 60 s), go. The rest of the store is the plugin's to clean.
pub(super) fn sweep_store(id: &str) {
    fn walk(dir: &Path, f: &mut dyn FnMut(&Path, std::time::Duration)) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                walk(&e.path(), f);
            } else if t.is_file() {
                let age = e.metadata().and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).unwrap_or_default();
                f(&e.path(), age);
            }
        }
    }
    const DAY: u64 = 86_400;
    let base = plugins::data_dir(id);
    let cache = base.join("cache");
    walk(&base, &mut |p, age| {
        // Only Echo's own downloads: `<name>.<pid>-<n>.part`.
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let ours = name.strip_suffix(".part").and_then(|r| r.rsplit_once('.')).and_then(|(_, t)| t.split_once('-')).is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && (a.to_owned() + b).bytes().all(|c| c.is_ascii_digit()));
        let stale_part = ours && age.as_secs() > 3600;
        if stale_part || (p.starts_with(&cache) && age.as_secs() > 7 * DAY) {
            let _ = std::fs::remove_file(p);
        }
    });
    // The folders of `cache/` left empty (a snapshot swept), deepest first; `cache/` itself stays.
    fn prune(dir: &Path) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                prune(&e.path());
                let _ = std::fs::remove_dir(e.path());
            }
        }
    }
    prune(&cache);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-plugin marketplace `m-<id>` at `root/<id>-market`: the plugin's manifest (at `version`) and host half.
    fn publish(root: &Path, id: &str, version: &str, settings: Value, main: &str) -> String {
        let mk = root.join(format!("{id}-market"));
        let d = mk.join("plugins").join(id);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": id, "name": id, "version": version, "main": "main.js", "settings": settings}).to_string()).unwrap();
        std::fs::write(d.join("main.js"), main).unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": format!("m-{id}"), "plugins": [{"id": id, "source": format!("./plugins/{id}")}]}).to_string()).unwrap();
        mk.to_str().unwrap().to_string()
    }

    fn run(id: &str, cmd: &str, a: Value, files: &[Value]) -> std::result::Result<Value, String> {
        for _ in 0..100 {
            match call_with(id, cmd, &a, files, CALL_TIMEOUT) {
                Err(e) if e.to_string().contains("no command") || e.to_string().contains("isn't loaded") => std::thread::sleep(Duration::from_millis(50)),
                r => return r.map_err(|e| format!("{e:#}")),
            }
        }
        Err("never loaded".into())
    }

    #[test]
    fn a_plugin_keeps_a_handed_file_in_its_store_and_counts_what_it_keeps() {
        let (_env, root) = paths::testenv::folders("jshost-store-copy-in");
        let main = r#"export function apply(ctx) {
          ctx.command('keep', async (a, { files }) => await ctx.store.copyIn(files[0], a.to, a.opts))
          ctx.command('usage', (a) => ctx.store.usage(a.dir))
        }"#;
        let src = publish(&root, "keeper", "0.1.0", json!([]), main);
        let name = plugins::market_add(&src, Some("m-keeper")).unwrap();
        plugins::refresh(&name).unwrap();
        plugins::install("keeper", Some(&name), &plugins::review("keeper", Some(&name)).unwrap().uses).unwrap();
        sync();
        let bytes: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
        std::fs::write(root.join("pic.png"), &bytes).unwrap();
        let h = files::make("keeper", &root.join("pic.png")).unwrap();
        let one = std::slice::from_ref(&h);
        // Byte for byte into the store, a new folder too; counted by `usage`, `dir` too.
        assert_eq!(run("keeper", "keep", json!({"to": "images/pic.png"}), one).unwrap(), json!({"path": "images/pic.png", "size": 5000}));
        assert_eq!(std::fs::read(plugins::data_dir("keeper").join("images/pic.png")).unwrap(), bytes);
        assert_eq!(run("keeper", "usage", json!({}), &[]).unwrap(), json!({"bytes": 5000, "files": 1}));
        assert_eq!(run("keeper", "usage", json!({"dir": "images"}), &[]).unwrap(), json!({"bytes": 5000, "files": 1}));
        assert_eq!(run("keeper", "usage", json!({"dir": "nothing-here"}), &[]).unwrap(), json!({"bytes": 0, "files": 0}));
        // Never over a file unless asked; never out of the store; never over Echo's own files there.
        assert!(run("keeper", "keep", json!({"to": "images/pic.png"}), one).unwrap_err().contains("there already"));
        assert_eq!(run("keeper", "keep", json!({"to": "images/pic.png", "opts": {"createNew": false}}), one).unwrap(), json!({"path": "images/pic.png", "size": 5000}));
        for (to, why) in [("../x.png", "isn't a path inside"), ("/etc/x.png", "isn't a path inside"), ("settings.json", "settings.json"), ("widgets.json", "widgets.json"), ("images", "a folder")] {
            assert!(run("keeper", "keep", json!({"to": to}), one).unwrap_err().contains(why), "{to}");
        }
        // Another plugin's handle is no handle.
        let other = files::make("someone-else", &root.join("pic.png")).unwrap();
        assert!(call_with("keeper", "keep", &json!({"to": "o.png"}), &[other], CALL_TIMEOUT).is_err());
        let left: Vec<_> = std::fs::read_dir(plugins::data_dir("keeper").join("images")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, ["pic.png"], "no part file left");
        plugins::remove("keeper", true).unwrap();
        sync();
    }

    #[test]
    fn settings_get_gives_the_manifests_default_for_what_nobody_set_but_never_for_a_secret() {
        let (_env, root) = paths::testenv::folders("jshost-settings-defaults");
        let main = r#"export function apply(ctx) {
          ctx.command('get', (a) => ctx.settings.get(a.key))
          ctx.command('__testSettings', (a) => {
            const key = ctx.settings.get('token')
            if (a.section === 'bad') throw new Error(`the server said no to ${key}`)
            return { ok: key === 'sk-made-up-key', message: `checked ${key} for ${a.section}`, extra: key }
          })
        }"#;
        let settings = json!([
            {"key": "voice", "label": "Voice", "default": "alloy"},
            {"key": "speed", "label": "Speed", "default": 1.5},
            {"key": "loud", "label": "Loud", "type": "bool", "default": false},
            {"key": "plain", "label": "Plain"},
            {"key": "token", "label": "Token", "type": "secret", "default": "planted"}
        ]);
        let src = publish(&root, "voiced", "0.1.0", settings, main);
        let name = plugins::market_add(&src, Some("m-voiced")).unwrap();
        plugins::refresh(&name).unwrap();
        plugins::install("voiced", Some(&name), &plugins::review("voiced", Some(&name)).unwrap().uses).unwrap();
        sync();
        let get = |k: &str| run("voiced", "get", json!({"key": k}), &[]).unwrap();
        assert_eq!((get("voice"), get("speed"), get("loud"), get("plain"), get("nothing")), (json!("alloy"), json!(1.5), json!(false), Value::Null, Value::Null));
        // What the user set wins, a bool false included; an empty string is a value too.
        plugins::set_setting("voiced", "voice", &json!("")).unwrap();
        plugins::set_setting("voiced", "loud", &json!(true)).unwrap();
        assert_eq!((get("voice"), get("loud")), (json!(""), json!(true)));
        // A secret has no default: its host half gets nothing until the user keeps one.
        assert_eq!(get("token"), Value::Null);
        // The connection test: the host half reads the secret, the window half is told only ok and a message.
        let test = |section: &str| plugins::test_answer("voiced", call_with("voiced", plugins::TEST_SETTINGS, &json!({"section": section}), &[], CALL_TIMEOUT));
        assert_eq!(test("tts")["ok"], false);
        plugins::set_setting("voiced", "token", &json!("sk-made-up-key")).unwrap();
        let ok = test("tts");
        assert_eq!(ok, json!({"ok": true, "message": "checked •••• for tts"}), "the secret is taken out of the message, nothing else is passed on");
        let bad = test("bad");
        assert_eq!(bad["ok"], false);
        assert!(bad["message"].as_str().unwrap().contains("the server said no to ••••") && !bad.to_string().contains("sk-made-up-key"), "{bad}");
        // A plugin with no test: told so, not a stack of errors.
        let none = plugins::test_answer("voiced", Err(anyhow!("no command “__testSettings”")));
        assert_eq!(none, json!({"ok": false, "message": "this plugin has no connection test"}));
        assert_eq!(plugins::test_answer("voiced", Ok(json!(true))), json!({"ok": true, "message": ""}));
        assert_eq!(plugins::test_answer("voiced", Ok(json!("x".repeat(500))))["message"].as_str().unwrap().chars().count(), 300);
        plugins::remove("voiced", true).unwrap();
        sync();
    }

    #[test]
    fn an_updated_plugins_host_half_runs_the_new_code_once_the_app_follows_the_change() {
        let (_env, root) = paths::testenv::folders("jshost-stale-host");
        as_app();
        let v1 = r#"export function apply(ctx) { ctx.command('old', () => 'v1') }"#;
        let v2 = r#"export function apply(ctx) { ctx.command('old', () => 'v2'); ctx.command('added', () => 'new') }"#;
        let src = publish(&root, "grower", "0.1.0", json!([]), v1);
        let name = plugins::market_add(&src, Some("m-grower")).unwrap();
        plugins::refresh(&name).unwrap();
        let agreed = plugins::review("grower", Some(&name)).unwrap().uses;
        plugins::install("grower", Some(&name), &agreed).unwrap();
        sync();
        assert_eq!(run("grower", "old", json!({}), &[]).unwrap(), json!("v1"));
        publish(&root, "grower", "0.2.0", json!([]), v2);
        plugins::update("grower", &agreed).unwrap().unwrap();
        // What the window's state does at its next poll.
        follow(&plugins::stamp().unwrap());
        assert_eq!(run("grower", "added", json!({}), &[]).unwrap(), json!("new"));
        assert_eq!(run("grower", "old", json!({}), &[]).unwrap(), json!("v2"));
        plugins::remove("grower", true).unwrap();
        sync();
    }
}
