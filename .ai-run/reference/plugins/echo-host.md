# Echo Plugin Host

**Reference**, not a guide: how Echo's side of plugins works — what it keeps in a plugin's store, how it runs scripts,
lays out `.plugins`, fills dashboards' settings and reaches sessions — as the code has it. It changes in the same PR
as the code it describes. The rules are in `.ai-run/guides/plugins/echo-plugins.md`; the API plugins see is in
`plugin-api.md` beside this.

## Store

- A plugin's store is the plugin's to clean; Echo removes only what lay in its `cache/` over 7 days (and the folders
  of `cache/` that leaves empty: a snapshot swept whole) and its own half-written downloads (`.part`) over an hour,
  when its host half starts. Evidence: `jshost::sweep_store`
- A plugin's writes of a file — `ctx.store.write` / `update` and `ctx.fs.write` / `append` — take one lock of its own
  across processes, `.locks/<id>.files.lock` beside the stores (removed with its data). A conditional write reads the
  file's sha256 inside the lock; `createNew` writes a hidden `.<name>.<pid>.<n>.part` beside the target (made new) and
  links it into place, which never replaces a file, the part removed either way. The lock holds only Echo's own
  processes: another program changing a granted folder's file meanwhile isn't held off. Evidence:
  `jshost::folders::put`
- `ctx.fs.snapshot` copies into `cache/snapshots/<16 hex>/` of the store, each file opened without following a link
  (`O_NOFOLLOW`; Windows: the reparse point opened as itself and refused) and refused unless regular, folders checked
  inside the root as they are entered; a file that can't be opened or read, or that grows past 50 MB while it is copied, is left out (its part removed), and on a
  cap or a copy that can't be written the folder made is removed. One snapshot of a plugin is made at a time, under
  `.locks/<id>.snapshot.lock` (taken without waiting; removed with its data). Evidence: `jshost::folders::snapshot`
- Echo keeps the plugin's `settings.json`, `inbox.json`, `widgets.json`, `threads.json` and `work.json` there;
  `threads.json` and `work.json` each change under its own lock across processes (`.locks/<id>.threads.lock`,
  `.locks/<id>.work.lock`); a `threads.json` that isn't JSON is moved aside as `threads.json.bad-<time>`. The plugin
  writes none of those five and removes none of `settings.json`, `threads.json`, `work.json` and the store folder
  itself; a name at the store's top level is plain ASCII. Evidence: `jshost::not_echos`
- A marketplace namespace's shared values (`<ns>.<name>`) live beside the stores in `@<ns>/` (`threads.json`,
  `shared.json`), each under its own lock (`.locks/@<ns>.threads.lock`, `.locks/@<ns>.shared.lock`), no plugin's store
  and never removed with one. Evidence: `plugins::shared`
- The window serves a store only as `/plugin/<id>/store/<file>`, media kinds only, kept inside the store.
  Evidence: `window::plugins::store_file`
- The addresses a plugin may reach by `network.hosts_from` are entered only by the user, in the window or the CLI, and
  kept outside the plugin's store. Evidence: `plugins::listed_addresses`

## Files handed to a plugin

- A handle is kept in the memory of the process that made it — the app's (a pick, a drop, the window's `ctx.call`) or a
  terminal's (`echoctl plugin call --file`) — for an hour, 1,000 at most (the oldest go), with its plugin, the file's real
  path, name and size; the plugin and the page see `{file, name, size}`. Every use checks it is the plugin's and not past
  its hour. They go when the plugin is switched off, updated or removed (`files::forget`, beside `scripts::stop_plugin`).
  A picker opens only on a command (refused in a run Echo started, `notes::headless`, and in its `apply`, a timer's
  or event handler's own slice and Echo's asks for its widgets' data and work rows); Windows' pickers read their prompt and filter from the environment, never quoted into the
  PowerShell script. A handed file, and an image served, is opened without blocking and refused unless it is a regular
  file (a FIFO put in its place is never waited on: `grants::open_regular`). Evidence: `jshost::files::own`
- `copyIn` writes the bytes into a hidden `.<name>.<pid>.<n>.part` beside the target (made new), then links it into place
  (never replacing a file) or, with `createNew: false`, renames it over the file; the part is removed either way.
  Evidence: `jshost::files::copy_in`
- Drops (macOS): the page tells the app which plugin listens with its view shown (`plugin_drops`, on every change of
  address and of listeners); the window's drag handler takes a drag of files only then, from its start to its end, and
  makes the handles; the page takes them once (`plugin_dropped` → `window.echo.pluginDrop`) and hands them to the
  plugin's handlers if its view is still shown. Any other drag is the webview's own. Windows has no drop handler: wry's
  would take every external drop from WebView2. Evidence: `window::plugins::dropped`, `dropTarget` (window script)
- A plugin's microphone (`uses.microphone`): the app keeps which plugin holds it (`window::plugins::MIC`: id, name, and
  one webview grant for each opening). The webview's permission requests: the microphone is granted only while a
  plugin holds it, once for each opening; the camera and the screen are refused; anything else keeps the engine's
  default (macOS: wry's `with_permission_handler`; Windows: Echo's own `PermissionRequested` handler with
  `SavesInProfile` off, so WebView2 asks again on every request, `window::gate_media`). WebKit keeps a page's grant and
  doesn't ask again, so on every watcher tick and after every page command the app stops the webview's microphone
  capture while no plugin holds it (`setMicrophoneCaptureState(None)`, which also makes WebKit ask again next time:
  `EchoWindow::follow_mic`). A recording starting takes the microphone from the plugin and tells the page
  (`window.echo.micStopped`); the window closing releases it. Without a handler (before this) wry granted every
  request on macOS. Evidence: `window::plugins::mic_grant`, `EchoWindow::follow_mic`
- The window serves a granted folder's images as `/plugin/<id>/folder/<root>/<path>` (§ Store's media rule, by folder).
  Evidence: `window::plugins::folder_image`

## Script runs

- Every run goes through `plugins::scripts::run`: the original from the installed package, made runnable by Echo, in
  its skill's session copy. The host half reaches it by `ctx.scripts.run`; Echo's sessions through the launcher Echo
  writes at the script's path in the session copy and in the laid-out skill (`echoctl plugin script`, so Claude Code's
  environment never reaches it) — the laid-out one allowed by its full path in the shared session arguments
  (`folder_args::build`): every declared one, in the folder session and in a routine's or Yolo's run alike. Evidence: `plugins::scripts::run`
- A script runs without stdin, within its declared time (a call may ask for less, a routine uses it as declared), in
  its own process group, killed whole when its time is up or the plugin is unloaded, updated or removed, or Echo quits;
  its output capped (what is past the cap cut and flagged); one run of a script at a time across processes (a lock per
  script; whichever process holds it, Echo stops its group). `ctx.scripts.run` is an async service: the host half
  answers its other calls while a script runs. Evidence: `plugins::scripts::run`, `plugins::scripts::stop_plugin`
- Its environment: `plugin-api.md` § Scripts. Evidence: `plugins::scripts::run`, `plugins::scripts::run_markers`

## Git

- `ctx.git.*` runs git only through `jshost::git::spawn`: the verbs of `VERBS` (`version`, `config`, `rev-parse`,
  `symbolic-ref`, `status`, `log`, `cat-file`, `ls-files`, `init`, `add`, `commit`, `rm` — no network verb), never
  through a shell, no stdin, `--no-pager`, in the granted folder, each path after `--` and literal
  (`GIT_LITERAL_PATHSPECS=1`); in a repository with `GIT_DIR=<top>/.git`, `GIT_WORK_TREE=<top>` and
  `GIT_CEILING_DIRECTORIES` the top's parent (`jshost::git::top_of`), so git never looks for one itself. Started with all it starts as one (`plugins::signin::Tree`: Unix its own session, TERM
  then KILL to the whole group; Windows a job object killing them all) and stopped at 20 s; stdout kept to 16 MB,
  stderr to 64 KB, the rest read and dropped; once it ends its output is waited for 2 s, then whatever it started is
  stopped and the call fails if something still holds it. Evidence: `jshost::git::spawn`
- The program (`jshost::git::program`): first where the system installs git (`system_candidates`) — macOS
  `/opt/homebrew/bin/git`, `/usr/local/bin/git`, then `/usr/bin/git` only when `xcode-select -p` (asked once per process) names a developer
  folder holding `usr/bin/git` (else that stub would open Apple's install dialog); Windows `%ProgramW6432%`,
  `%ProgramFiles%`, `%ProgramFiles(x86)%` `\Git\cmd\git.exe` or `\Git\bin\git.exe` (never a `.cmd`, which runs
  through cmd.exe); other Unix `/usr/bin/git`, `/usr/local/bin/git`. Else the first `git` (Windows `git.exe` only) on
  the user's `PATH` (`path_candidates`): on macOS and other Unix the login shell's, asked once per process as a terminal
  starts it (`ai::wrapper::login_path`; the app started from the Finder has a minimal `PATH`), this process's own where
  runs start without the login shell (`ECHOCTL_NOTES_NO_SHELL`) and on Windows. Each by its real path, skipping a
  relative entry, one inside a folder given to any plugin (`grants::all_folders`, on or off) or inside Echo's data or
  own folder (`ai::wrapper::outside_echo`), macOS's `/usr/bin/git`, and a file that can't be started. None: "git isn't
  installed on this computer". Evidence: `jshost::git::path_candidates`
- Its environment: only `PATH`, `HOME`, `USERPROFILE`, `HOMEDRIVE`, `HOMEPATH`, `SystemRoot`, `TEMP`, `TMP`, `TMPDIR`,
  `XDG_CONFIG_HOME` of the user's (no `GIT_*`, `ECHOCTL*`, `CLAUDE*`, `ANTHROPIC*`), then `LC_ALL=C`,
  `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0`, `GIT_LITERAL_PATHSPECS=1`, `GIT_EDITOR=:`, an empty `GIT_PAGER`,
  `GIT_NO_LAZY_FETCH=1` (a missing object is never fetched from a promisor remote) and an empty `GIT_ALLOW_PROTOCOL`
  (no transport at all, whatever `protocol.*.allow` says), and the overrides as `GIT_CONFIG_COUNT` / `GIT_CONFIG_KEY_n` / `GIT_CONFIG_VALUE_n` (git 2.31+, which is why older git
  is refused): the command line's own scope, above the system's, the user's and the repository's, and passed on to any
  git it starts. Evidence: `jshost::git::spawn`
- The overrides (`jshost::git::overrides`), read for each call from `git config --list` (every scope, `include` and
  `includeIf` files too), whole: a listing that fails or is over 16 MB, or a key that isn't UTF-8, refuses the
  repository (a setting not read, or read under another name, couldn't be cleared), as does a partial clone
  (`extensions.partialClone` or any `remote.*.promisor`) and `core.worktree` set anywhere. No settings file a plugin
  could change between that reading and git's own: read with `--show-origin`, each `include.path` /
  `includeIf.*.path` resolved as git does (`~/` from `HOME`, `%(prefix)/` from the folder above git's own, a relative
  path from the including file's folder; `~user/` refused), there or not (git skips a missing file), by its real path
  (the deepest existing folder canonical, `jshost::git::physical`); the repository is refused when one lies inside a
  folder given to any plugin, inside Echo's data or own folder, or in the work tree outside a hidden folder
  (`jshost::git::changeable`), or names bytes that aren't UTF-8 — and so when the user's `$XDG_CONFIG_HOME/git/config`,
  `~/.gitconfig` or `~/.config/git/config` lies in such a place, or the repository's `.git/config` or
  `.git/config.worktree` is a link into one. Evidence:
  `jshost::git::tests::settings_that_include_a_file_a_plugin_can_write_refuse_the_repository`. Every key whose last part names a program (`PROGRAM_KEYS`: `filter.*.clean/smudge/process`,
  `diff.*.command/textconv`, `merge.*.driver`, `credential.*.helper`, `gpg.*.program`, `*.sshCommand`, `*.askPass`,
  `*.editor`, `*.pager`, `*.proxy`, `remote.*.uploadpack/receivepack/vcs`, `uploadpack.packObjectsHook`, …) set empty,
  `filter.*.required` and every `pager.<command>` false, every `protocol.<name>.allow` never, and each hook set in
  the settings off (`hook.<name>.event` empty, `hook.<name>.enabled` false, its `command` empty); then, winning, `core.hooksPath` an empty folder Echo keeps
  (`<state>/plugins/.locks/git-no-hooks`, refused when not empty), `core.fsmonitor` and `core.useBuiltinFSMonitor`
  false, `core.pager` empty, `core.editor` and `sequence.editor` `:`, `credential.helper` and `diff.external` empty,
  `commit.gpgSign`, `tag.gpgSign` and `log.showSignature` false, `maintenance.auto` false and `gc.auto` 0,
  `submodule.recurse` false with submodules ignored (`diff.ignoreSubmodules=all`, `--ignore-submodules=all`),
  `protocol.allow` never, `core.quotePath` false, `color.ui` never. `commit` adds `--no-verify` and `--no-gpg-sign`,
  `init` `--template=`; `show` reads with `cat-file blob` (no filter or textconv) only at a commit (`cat-file -t` of
  `rev` is `commit`); `log` takes no diff.
  Evidence: `jshost::git::tests::a_repositorys_own_settings_run_nothing`
- The repository (`jshost::git::top_of`, then `Git::repo`): looked for upwards from the granted folder, each folder
  first for its `.git`, then as a bare repository. Only a `.git` that is a real folder (not a link, not a `gitdir:`
  file), owned by the user running Echo (Unix), with no plugin given a folder inside it, makes the folder holding it
  the top; a folder holding `HEAD`, `objects` and `refs` itself (a bare repository, or inside a `.git`) is refused. The
  git folder is then nothing a plugin can write — `grants::writable` refuses every hidden name, and above the granted
  folder it is outside the grant — so its settings stay as `overrides` read them while git runs, with no lock: only
  another program of the user's (a sync tool) could change them meanwhile. Any `core.worktree` refuses the
  repository (`overrides`). `rev-parse` must then give that top, `<top>/.git` as the git folder and the same as the
  common one; and, followed through links, the common folder's `objects`, `refs`, `packed-refs`, `info`, `logs`, the
  git folder's `HEAD` and `index`, and each entry of `objects`, `objects/pack` and `objects/info` must be inside the
  top; `objects/info/alternates` absent; else refused. A folder
  inside a larger repository works within the folder: `status` and `log` limited to `.` (or the paths asked), `show`
  as `<rev>:./<path>`, `commit` only paths `grants::writable` passes. Evidence: `jshost::git::Git::repo`
- `commit`: refused while a merge, rebase, cherry-pick, revert or bisect is under way (`info`'s `busy`), and when a
  path is one the repository ignores (`status --ignored=matching` gives `!!`). `ls-files` names the paths git knows;
  the others (each there on disk) are `add`ed, then `commit --only --cleanup=whitespace -- <paths>`; when the add or
  the commit fails the added ones are taken out of the index again (`rm --cached`).
  Identity as `committer.name` / `committer.email` (and `author.*` when `user.name` or `user.email` is unset) on the
  command line. Evidence: `jshost::git::commit`

## Sessions

- A plugin's session skills (its runtime text skills and its package skills) go only to Echo's sessions: the folder
  session, routines and Yolo get a laid-out plugin's skills through `.plugins/<id>/` (§ Layout) and a plugin not laid
  out by its session copy (`--plugin-dir`), once each; a `tools: "none"` run gets none.
  Evidence: `grants::set_session_skills`, `folder_args::build`
- Package skills are copied from the installed package into the plugin's session folder, beside its runtime text
  skills: checked by `plugins::package_skills` as a plugin tree (plain files, no links, inside the plugin), copied again
  on install, update and load, removed with the plugin. Evidence: `grants::copy_package_skills`
- A plugin's commands named in `uses.sessions.commands` are allowed by the arguments the folder session, routines and
  Yolo share (`folder_args::build`), with no `RUN_PLUGIN_*` guard. Evidence: `plugins::session_commands`

## Layout

Echo lays out each enabled plugin's `agent/` folder, package skills and runtime skills and entries at
`<data folder>/.plugins/<id>/` (`CLAUDE.md`, `.claude/skills`, `.claude/rules`, `guides/`, `scripts/echoctl`), built
beside it and swapped in, `${CLAUDE_PLUGIN_ROOT}` filled in; again on install, update, enable or a changed installed
copy; removed on disable and remove, and what isn't an enabled plugin's pruned at start — but `.plugins/.proof/`
(`plugins::agent::PROOF`: the marks of runs through a wrapper, claude-code-runs reference § Wrapper profiles), never. The folder session gets no
plugin skill up front: Echo's core (`echoctl core --hook`, a SessionStart hook) names each plugin with its summary and
its `CLAUDE.md` to Read first, and Claude Code loads the rest of that folder when Claude reads there; plugin text in the
core is one line each, `|` escaped, the core ≤ 9,000 chars (rows left out are counted). Each skill is laid out as
`<prefix>-<name>` (the manifest's `agent.prefix`, bound to its marketplace source like a shared namespace; a laid-out
name is unique across enabled plugins). The layout's record (installed files, folder, Echo's version, a hash of what
was written) lives in the config folder, and a folder whose record differs is laid out again (`sync_all`, the core's
hook). The pre-tool hook of the folder session, routines and Yolo (`echoctl plugins-hook`) refuses Edit and Write
under `.plugins/` and, best effort, a Bash line that writes there.
Evidence: `plugins::agent::lay_out`, `plugins::agent::render`, `guard`

A plugin's client half runs in the window's own page and can send the page's actions as the user would; "only the user
adds" a plugin's offer (a wrapper profile, a dashboard) holds for what a plugin can do through its manifest, `ctx`
services, jobs and the CLI inside runs, not against its own page code or its scripts (programs run with your rights).

## Dashboards

A thread's dashboard (its Brief) offers only widgets with a setting of type `thread`, a client's (its Overview) only
those with one of type `client`, and Echo's Filters on both; the page sets those settings, not editable there: on a
thread's page its `thread` setting is that thread, and a `client` setting beside it that thread's client (none: its
default); on a client's page its `client` setting is that client, and a `thread` setting beside it keeps the user's
value only for a thread of that client (else its default). Each place a widget is on is asked, and its answer kept, on
its own. Evidence: `today_layout::params_on`, `today_layout::Page::holds_thread`

A dashboard added from a plugin's preset keeps where it came from in `boards.json` (`from: {plugin, preset, version,
taken, taken_for, laid}`: the preset's widgets and `for` as taken, and the layout written) and its `for`; its status
compares them with the plugin's preset now (`update`) and the layout as saved, before the catalog's sizes (`edited`),
and with the registry (`off`, `gone`, `dropped`), reading the registry and manifests once for all dashboards. The window reads them with the presets on offer from `/api/boards` (`boards::view`). Evidence:
`boards::status_with`, `boards::take_update`

## Where things are

| When you are | Exemplar | Tests |
|---|---|---|
| adding a host service | `jshost::service` | `jshost::a_host_half_gets_echos_services_and_answers_commands` |
| adding to the client-half `ctx` | `pluginCtx` (window script) | no covering test |
| adding a granted folder's service | `jshost::folders::stat` | `jshost::folders::stat_reports_links_as_links_and_never_an_absolute_path` |
| handing a plugin files | `jshost::files::make` | `jshost::files::a_handle_is_its_plugins_alone_and_never_names_the_path` |
| changing install, update or remove | `plugins::install` | `plugins::install_update_and_remove_from_a_local_marketplace` |
| letting a plugin run Claude | `claudejob::Spec::from_value` | `claudejob::a_spec_stays_in_the_plugins_store` |
| changing package skills or scripts | `plugins::scripts::run` | `jshost::a_plugins_package_skill_reaches_the_sessions_and_its_declared_script_runs` |
