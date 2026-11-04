# Installing Echo

This guide sets up Echo from scratch and checks each step works before going on. It takes about 15 minutes, most of
it downloads.

You need:

- a Mac with macOS 14.2 or later, or a PC with Windows 10 (version 2004) or later;
- Microsoft Teams, the new app;
- for minutes and for Claude in the Echo window: a Claude account (Pro, Max, Team or Enterprise) and
  [Claude Code](https://claude.com/claude-code).

Echo records and transcribes calls without Claude Code. Claude Code is what writes the minutes, the action items and
the threads' work.

---

## 1. Install Echo

### macOS

1. Download `Echo-<version>-macos-arm64.dmg` from [Releases](https://github.com/onetest-ai/echo/releases).
2. Open it and drag **Echo** onto **Applications**.
3. Open Echo from Applications. Its octopus appears in the menu bar and in the Dock.

### Windows

1. Download `echo-<version>-windows-x64.msi` from [Releases](https://github.com/onetest-ai/echo/releases).
2. Run it. Windows may say it protected your PC: Echo isn't signed yet, so click **More info → Run anyway**.
   No administrator rights are needed.
3. Echo starts by itself when the installer ends. Its octopus is in the tray (the ^ next to the clock).

**Check:** the octopus is there. If you can't see it in the macOS menu bar (a full menu bar hides icons), click
Echo's icon in the Dock: it opens the Echo window.

## 2. First start

Echo asks for three things the first time:

1. **Where to keep your calls.** Pick a folder, for example `~/Echo` (`Documents\Echo` on Windows). Everything Echo
   makes stays there.
2. **A speech model.** Accept the suggested one. It downloads once (about 0.5–1.5 GB); after that, speech is turned
   into text on your computer, offline.
3. **Permissions** (macOS). Allow **Microphone** (your voice) and **System Audio Recording** (the others in the call,
   from Teams only). **Accessibility** is optional: with it Echo follows your mute in Teams and knows who is
   speaking.

**Check:** open Echo (menu → **Open Echo…**) → **Settings → Health**. Every permission under *Permissions* has a ✓.
If one doesn't, click **Ask for permissions**, or **Open settings** beside it.

## 3. Install Claude Code

Skip this if `claude --version` already works in a terminal.

1. Install it. macOS: open **Terminal** and paste

   ```sh
   curl -fsSL https://claude.ai/install.sh | bash
   ```

   Windows: open **PowerShell** and paste

   ```powershell
   irm https://claude.ai/install.ps1 | iex
   ```

   Other ways (Homebrew, npm) are on [Claude Code's setup page](https://docs.claude.com/en/docs/claude-code/setup).
2. **Open a new terminal window** (the old one doesn't see the new command) and run `claude`. Sign in with your
   Claude account when it asks, then type `/exit`.

**Check:** in Echo, **Settings → Health → Claude Code**:

| Row | ✓ means | If not |
| --- | --- | --- |
| **Claude Code** | Echo found it, and it starts (its version is shown) | *Not installed*: do step 3 again. *Doesn't start*: reinstall it (an npm install also needs Node.js). |
| **In a terminal** | a new terminal finds `claude` | Click **Add to PATH** (or run `echoctl claude path`), then open a new terminal. Echo itself still finds Claude Code without it. |
| **Echo plugin** | Claude Code lists Echo's plugin where Echo's sessions run | Step 4. |

Click **Check again** after fixing something. The same check from a terminal: `echoctl claude` (its `check` part).

## 4. Connect Claude Code to Echo

**Settings → General → Claude Code plugin → Connect** (or the menu: **Claude Code → Connect Claude Code**).

This installs Echo's plugin into Claude Code for Echo's data folder: the status line, `/echo:process-call`,
`/echo:record`, `/echo:doctor`, `echo:manage` (clients, threads, work through Echo) and `echo:microsoft-365` (mail,
Teams, files and people through Echo's own Microsoft 365 sign-in) — in any Claude Code session. Echo then checks Claude Code really lists it. If it doesn't, the notification says
so and what to run instead. It never reports success for a plugin that isn't there.

For every Claude Code session on this computer, not only Echo's, use `echoctl claude connect --scope user`.

**Check:** *Settings → Health → Echo plugin* has a ✓, and *Settings → General → Claude Code plugin* says
**Installed ✓**.

## 5. Microsoft 365 (calendar, chat, mail)

**Settings → Calendar & chat → Account → Sign in…** Your browser opens your organisation's sign-in. Echo reads
your calendar and meeting chats, and sends mail you ask Claude to send, through its own sign-in.

Echo signs in with the public app registration of [CodeMie Code](https://github.com/codemie-ai/codemie-code), the open-source Claude Code
wrapper — the same one its [Microsoft 365 skill](https://github.com/codemie-ai/codemie-code/tree/main/src/agents/plugins/claude/plugin/skills/msgraph) uses, so your IT may have approved it already. If your IT blocks it, ask them to approve it, or to register an app
for you and put its ID under **App registration → Change…** on the same page.

**Check:** the **Calendar** view shows today's meetings.

> Mail from Claude in the Echo window always goes through Echo first (`echoctl deliver send mail`), from your own
> account, after you confirm. Claude uses its own Microsoft 365 connector only when Echo can't send. If neither
> works, it opens a draft in your mail app.

## 6. Try it

1. Menu → **Record next call**, then join a Teams call (a test call with a colleague is fine). The octopus turns
   green, then orange while it records.
2. Hang up. A notification says the transcript is ready and when the minutes come (15 minutes later, or click
   **Write minutes** on the call's Minutes tab).
3. Open the call: Minutes, Transcript, Actions.

If the minutes fail, the Minutes tab says why and **Show log** shows what Claude Code did.

---

## Optional

- **Command line:** Settings → General → Command line → **Install…** puts `echoctl` in your terminal (macOS asks
  for your password).
- **Start at login:** Settings → General (on by default).
- **Brain:** Settings → Brain → **Use the Brain** (off by default). It indexes your calls, threads' mail, channels
  and folders for search and for Claude.

## Troubleshooting

| What you see | What to do |
| --- | --- |
| No octopus in the menu bar | Click Echo in the Dock, or open it from Applications. A full menu bar hides icons. (Settings → General → *Hide from the Dock when the window is closed* takes it out of the Dock; it's off unless you turned it on.) |
| A recording is silent | Settings → Health: *System Audio Recording* and *Microphone* need a ✓. Run `/echo:doctor` in Claude Code. |
| "Claude Code is not installed" | Step 3, then **Check again** in Settings → Health. |
| `claude: command not found` in a terminal | Settings → Health → **Add to PATH** (or `echoctl claude path`), then a new terminal. |
| Minutes say "Not logged in" | Run `claude` in a terminal and sign in again, or click **Sign in…** where the call shows the error. |
| Settings → Health shows *An older Echo plugin* | Click **Update it**, then restart Claude Code. |
| Claude in the Echo window doesn't know about a call | The call was recorded on another AI profile: pick that profile in the Claude panel. |

## Updating and removing

Echo checks for updates every 6 hours and offers them in the menu (**Update to …**). Removing it keeps your calls:
macOS, drag Echo from Applications to the Bin; Windows, Settings → Apps → Echo → Uninstall. The calls folder stays
where you put it.
