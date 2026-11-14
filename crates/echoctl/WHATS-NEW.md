# Writing What's new

`whats-new.json` beside this file is what the window shows after an update (`whats_new.rs`). Every release adds its
entry in its version-bump PR: `cargo test` fails while the running version (`Cargo.toml`) has none.

## Shape

The file is a list of entries, newest first:

```json
{ "version": "0.9.9", "items": [{ "icon": "filter", "title": "A few words", "text": "One sentence.", "open": "today" }] }
```

- `items`: 1 to 6, the release's most useful changes first. Leave out what a user doesn't notice.
- `icon`: one of `whats_new::ICONS` (`sparkle`, `filter`, `export`, `layout`, `link`, `plugin`, `shield`, `thread`,
  `calendar`, `record`, `fix`). A new icon is added there and in the window's `WN_ICONS`.
- `title`: a few words, at most 40 characters, no full stop.
- `text`: one sentence, at most 160 characters, ending with a full stop (no ". " inside, so no "e.g.").
- `open` (optional): where "Show me" takes the user: a window route (`today`, `calls`, `calendar`, `calendar/day`,
  `calendar/week`, `tasks`, `projects`, `found`, `yolo`, `settings`, `settings/<section>`) or an `echo://` link.

## Words

- Write for someone who reads only the headlines: what they can do now, or what works better for them.
- Their words, not ours: "Add Filters to Today", not "a Filters widget kind with shared keys".
- No internals: no PR or issue numbers, ids, file names, commands or code.
- A fix is told by what now works ("Widgets name the right page"), not by the bug.
- Security changes say what is safer for the user, in one plain line.
