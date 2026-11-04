# Echo

**Echo records the Teams calls you choose, turns them into text on your own Mac, and gives you
minutes and action items afterwards.** No bot joins the call, nothing is uploaded for
transcription, and the audio is deleted as soon as the transcript is ready.

![How Echo works](docs/images/how-it-works.svg)

Echo is two things that work together:

- **The Echo app.** It lives in the menu bar, records the calls you opt in to and writes the
  transcript. It works on its own.
- **Claude Code (optional).** After a call, Claude reads the transcript and writes the minutes
  and the action items (and follow-up drafts, in a session). Without it you still get the full
  transcript.

macOS 14.2 or later today. Windows is in progress.

---

## 1. Install

1. Download `Echo-<version>.dmg` from [Releases](https://github.com/onetest-ai/echo/releases).
2. Open it and drag **Echo** to **Applications**.
3. Open Echo. On first launch it asks you for three things:
   - **Where to keep your calls.** Pick any folder (for example `~/Echo`). Everything Echo makes
     stays in that folder.
   - **A speech model.** Accept the suggested one. It downloads once (about 0.5 GB), then
     everything runs offline.
   - **Permissions.** macOS asks for *Microphone* (your voice) and *System Audio Recording*
     (the other people in the call, from Teams only). *Accessibility* is optional: with it,
     Echo knows when you mute yourself in Teams and who is speaking.

That's it. The octopus is now in your menu bar and in the Dock. Step by step, with a check after each step, Windows
included: [docs/install.md](docs/install.md).

## 2. Record a call

Echo never records on its own. You decide, per call, in one of two ways:

- **Right now:** click the octopus, then **Record next call**. Echo starts when the Teams call
  starts and stops when it ends. In the Echo window, the small **EN ▾** on the button picks the
  call's language (or **Detect**, to let Echo pick among your languages).
- **Ahead of time:** open the calendar (step 4) and click **Record** on a meeting. Echo arms
  itself a minute before it starts.

<p align="center"><img src="docs/images/menu.png" width="480" alt="The Echo menu in the menu bar"></p>

The icon tells you what is happening: **white** means idle, **green** means armed and waiting
for the call, **orange** means recording.

## 3. Read what was said

Click **Open Echo…** in the menu. Every recorded call is in the list, newest first.

**Minutes**: the summary, decisions, open questions and next steps. The small purple times
(`11:40`) jump to that moment in the transcript.

![Minutes for a call](docs/images/notes.jpg)

**Notes**: your own notes, during the call and after it. They are saved as you type (**Time**
inserts the call's time while it records), and the minutes are written around them.

**Transcript**: who said what and when. *Me* is you. In a call with one other person, they are
named from the invite. In a bigger call, Echo names people from Teams itself: it watches who
Teams shows as speaking (this needs the *Accessibility* permission). Download **Speech models →
Speaker detection** (107 MB) too, and Echo also tells the voices apart, so a voice Teams didn't
name still gets its own label (*Speaker 2*). Claude names those in the minutes when someone
introduces themselves or is addressed by name.

![Transcript](docs/images/transcript.jpg)

**Actions**: the to-dos from the call, with owner and due date. Tick them off as you go.

To share the minutes, click **Open in browser** or **Show in Finder** on the Minutes tab: Echo
writes `<title>-minutes.html` in the call's folder, one file with the minutes, the shared screens
and diagrams, that opens in any browser and prints well. Menu → Claude Code → **Write a minutes
page (HTML file) too** writes it with every call's minutes. Mind what is on those screens before
you send it on. `echoctl html <call> --open` writes it for any call.

![Action items](docs/images/actions.jpg)

**Chat**: what people wrote in the meeting chat while the call was on, including the files they
shared, so a link pasted mid-call isn't lost. There's a dark mode too:

![Meeting chat, dark mode](docs/images/chat-dark.jpg)

There's also a **Screens** tab, if you turn on screen capture in Settings (off by default). It
keeps only the frames where the shared screen actually changed.

## 4. Plan your week

Connect your Microsoft 365 calendar (menu → **Calendar → Connect Microsoft 365**, read-only)
and the **Calendar** view shows yesterday, today and the next five days. Green means recorded,
dashed purple means Echo will record it, and grey means you declined or it was cancelled.
Click a meeting for details, a recap of last time, and the Record button.

![Calendar week](docs/images/calendar.jpg)

> Echo signs in with the public app registration of [CodeMie Code](https://github.com/codemie-ai/codemie-code) (its
> [Microsoft 365 skill](https://github.com/codemie-ai/codemie-code/tree/main/src/agents/plugins/claude/plugin/skills/msgraph) uses the same one) by default. Your organisation's IT may need
> to approve it, or register its own app for you: put its ID under menu → Calendar → Change app
> (client) ID.

## 5. Get minutes from Claude (optional)

Minutes, action items and follow-ups are written by [Claude Code](https://claude.com/claude-code).

1. Install Claude Code and sign in. That is all Echo needs to write minutes. **Settings → Health → Claude Code**
   checks it really works: that it starts, that a terminal finds it (else **Add to PATH**), and that its Echo
   plugin is installed. [docs/install.md](docs/install.md) has the steps.
2. Optional: menu → **Claude Code → Connect Claude Code** installs the Echo plugin for Claude Code
   sessions in Echo's data folder (`echoctl claude connect --scope user` for every session): a status
   line above the prompt, `/echo:process-call`, `/echo:record` and `/echo:doctor` (it checks Echo and
   fixes what's missing if something doesn't work).

From then on, Echo writes the minutes by itself, **15 minutes after the call** (or after your
last note, if you are still typing): it runs Claude Code in the background on your Mac with its
own copy of the plugin, and no Claude Code session needs to be open. When the transcript is ready, a notification says when the
minutes come ("minutes at 17:45"). Calls shorter than 3 minutes or with little said (unless a
screen was shared) get none, nor do calls that ended before automatic minutes were on. **Write
minutes** on the Minutes tab writes them at once, for any call. If it fails (for example, Claude
Code is signed out), the call says why and **Show log** shows what happened. Menu → Claude Code →
**Write minutes automatically** turns this off.

Claude Code sessions don't pick up calls by themselves. Ask one with `/echo:process-call <call>`
and it writes the minutes and offers follow-ups, such as a recap email or tasks for your action
items. **Nothing is sent without your explicit yes.**

Each AI profile has its own minutes model (`sonnet` by default; a local profile, the model it was
added with, or its live model when that is cleared). Change the one new calls use under menu → AI for calls → **Minutes model for …**, or any
profile's with `echoctl ai set <profile> --notes-model <name>`: `opus`, `haiku`, a full model
name, or, for the built-in profile, `default` to use whatever your Claude Code is set up with
(local models, a gateway, Bedrock or Vertex). Minutes runs need Claude Code 2.1.295 or later. During a
call, Echo's window shows the transcript as it happens, and the Notes tab (or menu → **Add a
note…**) takes your own notes.

<details>
<summary>Install the plugin by hand, or for one project</summary>

```sh
claude plugin marketplace add onetest-ai/echo
claude plugin install echo@onetest --scope user      # or: project (shared) / local (just you)
```
</details>

---

## Privacy, in short

- **Opt-in per call.** Echo records only calls you armed. Meetings tagged `no_notetakers` are
  marked so you know.
- **On your Mac.** Speech is turned into text locally with [whisper.cpp](https://github.com/ggml-org/whisper.cpp)
  (or NVIDIA's [Nemotron](https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b), if you pick it), unless you connect Echo to a transcription server yourself (see the FAQ). Then that computer
  hears the audio, and nothing is kept there.
- **Text only.** Audio is deleted once the transcript exists. Calls live in the folder you
  picked, as plain Markdown and JSON.
- **Minutes.** With Claude Code installed, Echo runs it on each recorded call long enough for
  minutes, 15 minutes after the call (or after your last note), with the call's AI profile. To
  write the minutes it reads:
  - the transcript and your own notes;
  - the invite (subject, time, organizer, attendees with their email addresses, the Teams join
    link) and the meeting chat;
  - the shared screens Echo saved (menu → Settings → Save snapshots of shared screens, off by
    default);
  - your name and email from Echo's settings, your profile (`me.md`, if you filled it in), the
    vocabulary and the notes template;
  - the minutes of the last earlier call with the same subject, and on a re-run the current action
    items.

  Echo checks every step the run takes, whatever your own Claude Code settings allow: its own file
  reads are limited to this call's folder and those previous minutes, it writes only the minutes
  and its action list there, it runs only the Echo commands the minutes need and only on this call
  (not your settings, calendar or chats), and it has no web access and no MCP servers. Outside the
  call it may only add vocabulary it learned for this meeting and move screen snapshots it drops to
  Echo's trash. (One limit: if your organisation newly turns on a Claude Code policy that switches
  hooks off, the next run goes unchecked; Echo refuses runs from then on.) With the built-in
  profile, Claude Code's own setup still applies around those steps: hooks from your settings and
  plugins run and see what each step reads (Echo turns hooks on for the run so its check runs, so
  hooks you switched off with `disableAllHooks` run too), and your CLAUDE.md files load. All of it
  goes to the profile's model: for the built-in profile, wherever your Claude Code sends it
  (Anthropic, or your gateway, Bedrock or Vertex); for a Claude account, Anthropic; for a local
  model, the server the profile names (with Ollama on your computer, it stays there). With menu →
  Claude Code → **Write minutes automatically** off, this happens only when you click **Write
  minutes**.
- **You are responsible for consent.** Tell the people on the call you are taking notes.

## FAQ

**Teams didn't get recorded.** Menu → **Health** shows what's missing, usually a permission.
Permissions belong to Echo in System Settings → Privacy & Security.

**Can I use another language?** Yes. Echo detects the language for each sentence. Menu →
Settings → Languages limits it to the ones you speak, which avoids mix-ups between similar
languages.

**Transcription is slow on my laptop.** Without a graphics card, Whisper takes a long time.
Two ways out:

- **A faster model.** Menu → **Speech models → NVIDIA Nemotron → nemotron-3.5-0.6b**: download
  it (742 MB) and choose **Use this model**. It runs several times faster than real time on a
  plain CPU and covers English, Spanish, Russian, Ukrainian and about 20 other languages.
- **A faster computer** on your network, such as a Mac with Apple Silicon or a PC with a
  graphics card, does the work:
  1. On the fast computer, open the Echo menu and choose **Settings → Transcribe for other
     computers**. Echo shows a URL and an API key.
  2. On the laptop, choose **Settings → Connect to a transcription server…** and enter them.

With a server, each sentence of the call's audio is then sent to that computer, and the text comes back.
The fast computer keeps no audio or text. The connection isn't encrypted, so use it only on a
network you trust. Echo also accepts an `https://` URL, for a server behind a TLS proxy. If the
server can't be reached, the laptop transcribes the call itself. From a terminal, use
`echoctl server use <url> --key <key>` or `echoctl server share on`.

**Can a model on my Mac write the minutes?** Yes, through [Ollama](https://ollama.com): then
writing the minutes sends nothing off your Mac.

1. In Ollama, use a model that can call tools (Qwen 3.6, for example) with a context of at least
   64K tokens: Ollama → Settings → Context length, or a model of your own whose Modelfile says
   `PARAMETER num_ctx 131072`.
2. In the folder Echo runs Claude Code in for minutes (your data folder, or the one under
   menu → Claude Code → Echo runs Claude Code for minutes in), create `.claude/settings.json`:
   ```json
   { "env": {
       "ANTHROPIC_BASE_URL": "http://localhost:11434",
       "ANTHROPIC_AUTH_TOKEN": "ollama",
       "ANTHROPIC_API_KEY": "",
       "ANTHROPIC_DEFAULT_HAIKU_MODEL": "<the Ollama model>" } }
   ```
3. `echoctl config --notes-model <the Ollama model>`.

Writing them then takes a few minutes, and a local model follows Echo's instructions less
closely than Claude: read them before you share them. Echo's own runs (automatic, or **Write
minutes**) with the built-in AI profile use the folder from step 2; a Claude Code session you ask
with `/echo:process-call` uses its own model. To keep a call entirely on your Mac, let Echo write
its minutes.

**Where are my files?** In the folder you picked, under `calls/`. Menu → **Open calls folder**.

**How do I update?** When a new version is out, the menu shows **Update to …**. One click
installs it and restarts Echo.

---

## For developers

<details>
<summary>CLI, building, checks</summary>

The app's executable is also a CLI, `echoctl` (menu → Settings → Install command-line tool puts it on
your PATH; on macOS it lives at `/Applications/Echo.app/Contents/MacOS/echoctl`):
`echoctl app [start|quit|autostart on|off|install-cli]`, `echoctl home [set <dir>|choose]`,
`echoctl claude [connect --scope user|project|local]`, `echoctl doctor`,
`echoctl arm|stop|status`, `echoctl model list|download|use|remove`,
`echoctl notes <call>` (write a call's minutes), `echoctl claim|context` (used by the plugin).

```sh
scripts/build-plugin.sh          # dev build into plugin/bin (the plugin's launcher prefers it)
claude --plugin-dir plugin       # or install it from the repo's marketplace
scripts/package-macos.sh         # signed + notarized Echo.app in a .dmg (see the script header)
LOCAL=1 scripts/package-macos.sh # no Apple developer account: an ad-hoc signed dist/Echo.app
                                 # for this Mac (macOS asks for its permissions again per build)
scripts/package-windows.ps1      # .msi installer + .zip for Windows
```
Checks: `cargo clippy`, `cargo test`, `cargo check --target x86_64-pc-windows-msvc
--no-default-features`, `scripts/check-mod.sh`. See `CLAUDE.md` for the design.

Windows 10 2004+: double-click `echo-<version>-windows-x64.msi` (per user, no administrator
rights: Start menu, PATH, start at login; upgrades quit and restart Echo; uninstall from
Settings → Apps keeps your calls). The `.zip` has the same files for a manual install.
On Windows the other people
in the call are recorded from the speakers or headset Teams plays on, so other sounds on that
device (other apps, notifications) are recorded too: Windows can't isolate Teams' audio the
way macOS does.
</details>

## License

Copyright © 2026 Artyom Rozumenko. Echo is **open source** under the
[GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0-only). In plain words:

- **You can** use Echo for free, for any purpose: for yourself, your work calls, inside your
  company. You can read, change and share the code.
- **If you share Echo or a changed version** — as a download, or as a service people use over a
  network — you must offer its complete source code under the same license.
- **Want to ship it in a closed product?** A commercial license is available: open an issue.

This is a summary; [LICENSE](LICENSE) is what counts. Releases up to 0.5.6-beta were published under
the Functional Source License 1.1 (FSL-1.1-ALv2) and keep those terms.
Third-party components keep their own licenses: see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
The onetest name and octopus logo are trademarks of onetest and aren't covered by the license:
a fork must use its own name and logo.

## Credits

Echo was inspired by [Granola](https://www.granola.ai), the AI notepad for meetings. It is an
independent project, not affiliated with Granola. The screenshots above use made-up people and
companies.
