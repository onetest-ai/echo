#!/usr/bin/env bash
# Remove everything Echo leaves for the CURRENT macOS user, to get a clean-install test (S-15, S-16) on a Mac.
# Dry run by default: it prints what it would remove. `--yes` removes it; `--app` also removes Echo.app.
#
#   tests/tools/clean-mac.sh            # what would go
#   tests/tools/clean-mac.sh --yes      # remove it (keeps Echo.app)
#   tests/tools/clean-mac.sh --yes --app
#
# Where Echo puts things (keep in step with the code):
#   - Echo.app: /Applications/Echo.app or ~/Applications/Echo.app, bundle id ai.onetest.echo
#     (crates/echoctl/src/app.rs installed_app, BUNDLE_ID; packaging/macos/Info.plist).
#   - config dir: ~/Library/Application Support/echo (crates/echoctl/src/paths.rs config_dir). It holds home.json
#     (the chosen data folder), app-path (app.rs record_app_path), models/, ai.json, graph.json (Microsoft 365
#     tokens, a file, not the Keychain: graph.rs), claude/<profile>/ (AI profiles' Claude Code folders: ai.rs
#     config_folder), plugins/, and the rest of paths::config_dir().join(..).
#   - data folder: the "home" of home.json (paths.rs set_home makes calls/ and .echo/ in it; default ~/Echo,
#     docs/install.md step 2); without home.json the data is in the config dir itself. The app's log is
#     <home>/.echo/app.log (app.rs detach_output).
#   - LaunchAgent: ~/Library/LaunchAgents/ai.onetest.echo.plist, label ai.onetest.echo (app.rs launch_agent,
#     set_autostart).
#   - CLI link: /usr/local/bin/echoctl → Echo.app/Contents/MacOS/echoctl (app.rs CLI_LINK, install_cli). It is
#     outside the home folder, so this script only prints the command for the user.
#   - Claude Code plugin: echo@onetest from the marketplace "onetest", user or project scope, in Claude Code's
#     own records (~/.claude/plugins/installed_plugins.json, known_marketplaces.json; app.rs MARKETPLACE,
#     connect_claude, installed_plugins_file). Removed through `claude plugin uninstall` / `marketplace remove`.
#   - The webview's and macOS's per-app folders, named by the bundle id: ~/Library/{Caches,WebKit,HTTPStorages,
#     Preferences,Saved Application State,Logs}/ai.onetest.echo*.
#   - Permissions (TCC: Microphone, AudioCapture, Accessibility, ScreenCapture; crates/echoctl/src/permissions/
#     macos.rs) are not reset here: the script prints the `tccutil reset` lines.
#
# Safety: refuses root; never touches a path outside $HOME except /Applications/Echo.app; removes a folder only
# when Echo's own markers are in it; ignores ECHOCTL_HOME / ECHOCTL_CONFIG_DIR (test folders are not the install).
set -euo pipefail

BUNDLE_ID="ai.onetest.echo"
MARKETPLACE="onetest"
PLUGIN="echo@$MARKETPLACE"

yes=0
app=0
for a in "$@"; do
  case "$a" in
    --yes) yes=1 ;;
    --app) app=1 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "unknown argument: $a" >&2; exit 2 ;;
  esac
done

[ "$(uname -s)" = "Darwin" ] || { echo "macOS only" >&2; exit 1; }
[ "$(id -u)" -ne 0 ] || { echo "refusing to run as root: run it as the user whose Echo you clean" >&2; exit 1; }
HOME_DIR="$(cd "$HOME" && pwd -P)"
[ -n "$HOME_DIR" ] && [ "$HOME_DIR" != "/" ] || { echo "no usable HOME" >&2; exit 1; }
unset ECHOCTL_HOME ECHOCTL_CONFIG_DIR

if [ "$yes" -eq 1 ]; then echo "Removing Echo for user $(whoami) ($HOME_DIR)"; else echo "Dry run for user $(whoami) ($HOME_DIR): nothing is removed (--yes to do it)"; fi

found=0
# A path is allowed when, resolved, it is inside $HOME (not $HOME itself) or is one of the Echo.app bundles.
allowed() {
  local p
  p="$(cd "$(dirname "$1")" 2>/dev/null && pwd -P)/$(basename "$1")" || return 1
  case "$p" in
    /Applications/Echo.app|"$HOME_DIR"/Applications/Echo.app) return 0 ;;
    "$HOME_DIR"/?*) return 0 ;;
    *) return 1 ;;
  esac
}
remove() { # remove <path> <why>
  local p="$1" why="$2"
  [ -e "$p" ] || [ -L "$p" ] || return 0
  if ! allowed "$p"; then echo "  SKIP (outside the home folder) $p" >&2; return 0; fi
  found=1
  if [ "$yes" -eq 1 ]; then rm -rf -- "$p"; echo "  removed  $p  ($why)"; else echo "  would remove  $p  ($why)"; fi
}
run() { # run <description> <command…>
  found=1
  if [ "$yes" -eq 1 ]; then echo "  $1"; "${@:2}" || echo "    (failed: ${*:2})" >&2; else echo "  would run: ${*:2}"; fi
}
bundle_id_of() { /usr/bin/defaults read "$1/Contents/Info" CFBundleIdentifier 2>/dev/null || true; }

# 1. A running Echo stops first (its own quit, then the processes of the bundle). Only this user's processes
#    whose command is Echo.app's echoctl are signalled (options before the pattern: BSD pgrep stops at it).
echo "Running app:"
echo_pids() {
  local p
  for p in $(pgrep -U "$(id -u)" -f '/Echo\.app/Contents/MacOS/echoctl' || true); do
    case "$(ps -o command= -p "$p" 2>/dev/null)" in
      */Echo.app/Contents/MacOS/echoctl*) printf '%s ' "$p" ;;
    esac
  done
}
pids="$(echo_pids)"
if [ -n "$pids" ]; then
  for b in /Applications/Echo.app "$HOME_DIR/Applications/Echo.app"; do
    [ -x "$b/Contents/MacOS/echoctl" ] && [ "$(bundle_id_of "$b")" = "$BUNDLE_ID" ] && run "quit Echo" "$b/Contents/MacOS/echoctl" app quit && break
  done
  [ "$yes" -eq 1 ] && sleep 2
  pids="$(echo_pids)"
  # shellcheck disable=SC2086
  [ -n "$pids" ] && run "stop Echo's processes: $pids" kill $pids
else
  echo "  none"
fi

# 2. LaunchAgent: bootout, then the file (only when its Label is Echo's).
echo "Start at login:"
plist="$HOME_DIR/Library/LaunchAgents/$BUNDLE_ID.plist"
if [ -f "$plist" ] && [ "$(/usr/bin/defaults read "${plist%.plist}" Label 2>/dev/null)" = "$BUNDLE_ID" ]; then
  launchctl print "gui/$(id -u)/$BUNDLE_ID" > /dev/null 2>&1 && run "bootout $BUNDLE_ID" launchctl bootout "gui/$(id -u)/$BUNDLE_ID"
  remove "$plist" "LaunchAgent"
elif [ -e "$plist" ]; then
  echo "  SKIP $plist: its Label isn't $BUNDLE_ID" >&2
else
  echo "  none"
fi

# 3. The Claude Code plugin and its marketplace, through Claude Code (it keeps its own records).
echo "Claude Code plugin:"
claude_dir="${CLAUDE_CONFIG_DIR:-$HOME_DIR/.claude}"
claude_bin="$(command -v claude || true)"
[ -z "$claude_bin" ] && [ -x "$HOME_DIR/.local/bin/claude" ] && claude_bin="$HOME_DIR/.local/bin/claude"
installs="$(python3 - "$claude_dir/plugins/installed_plugins.json" "$PLUGIN" <<'PY' 2>/dev/null || true
import json, sys
try: d = json.load(open(sys.argv[1]))
except Exception: sys.exit(0)
for i in (d.get("plugins") or {}).get(sys.argv[2], []):
    print(f'{i.get("scope", "user")}\t{i.get("projectPath", "")}')
PY
)"
market="$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print("yes" if sys.argv[2] in d else "")' "$claude_dir/plugins/known_marketplaces.json" "$MARKETPLACE" 2>/dev/null || true)"
if [ -z "$installs" ] && [ -z "$market" ]; then
  echo "  none"
elif [ -z "$claude_bin" ]; then
  echo "  $PLUGIN is recorded but \`claude\` isn't found: run \`claude plugin uninstall $PLUGIN\` and \`claude plugin marketplace remove $MARKETPLACE\` yourself" >&2
else
  while IFS=$'\t' read -r scope project; do
    [ -n "$scope" ] || continue
    if [ "$scope" = "user" ]; then
      run "uninstall $PLUGIN (user)" "$claude_bin" plugin uninstall "$PLUGIN" --scope user
    elif [ -n "$project" ] && [ -d "$project" ]; then
      run "uninstall $PLUGIN ($scope, $project)" sh -c 'cd "$1" && "$2" plugin uninstall "$3" --scope "$4"' _ "$project" "$claude_bin" "$PLUGIN" "$scope"
    else
      echo "  $PLUGIN ($scope) is recorded for $project, which is gone: Claude Code drops the record itself" >&2
    fi
  done <<< "$installs"
  [ -n "$market" ] && run "remove marketplace $MARKETPLACE" "$claude_bin" plugin marketplace remove "$MARKETPLACE"
fi
# Leftovers Claude Code keeps per marketplace and plugin (only Echo's names).
for d in "$claude_dir/plugins/marketplaces/$MARKETPLACE" "$claude_dir/plugins/cache/$MARKETPLACE" "$claude_dir/plugins/data/echo-$MARKETPLACE"; do
  remove "$d" "Claude Code's copy of Echo's plugin"
done

# 4. The data folder recorded in the config dir (only with Echo's calls/ and .echo/ in it).
echo "Data folder:"
cfg="$HOME_DIR/Library/Application Support/echo"
data=""
if [ -f "$cfg/home.json" ]; then
  data="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("home",""))' "$cfg/home.json" 2>/dev/null || true)"
fi
if [ -z "$data" ]; then
  echo "  none chosen (the data, if any, is in the config dir)"
elif [ ! -d "$data" ]; then
  echo "  $data (recorded) doesn't exist"
elif [ "$(cd "$data" && pwd -P)" = "$HOME_DIR" ]; then
  echo "  SKIP $data: it is the home folder itself" >&2
elif ! allowed "$data"; then
  echo "  SKIP $data: outside the home folder" >&2
elif [ -d "$data/calls" ] && [ -d "$data/.echo" ]; then
  remove "$data" "data folder (calls and .echo inside); the user's calls go with it"
else
  echo "  SKIP $data: no calls/ and .echo/ in it, so not identified as Echo's" >&2
fi

# 5. The config dir (only with one of Echo's own files in it).
echo "Config dir:"
if [ -d "$cfg" ]; then
  marked=0
  for m in home.json app-path ai.json app.lock config.json models graph.json plugins.json window-layout.json .echo calls; do
    [ -e "$cfg/$m" ] && marked=1 && break
  done
  if [ "$marked" -eq 1 ]; then remove "$cfg" "config dir (settings, models, sign-ins, AI profiles)"; else echo "  SKIP $cfg: none of Echo's files in it" >&2; fi
else
  echo "  none"
fi

# 6. Per-app folders macOS and the webview keep, named by the bundle id.
echo "Caches, preferences, logs:"
n=0
for p in "$HOME_DIR/Library/Caches/$BUNDLE_ID" "$HOME_DIR/Library/WebKit/$BUNDLE_ID" "$HOME_DIR/Library/HTTPStorages/$BUNDLE_ID" \
         "$HOME_DIR/Library/HTTPStorages/$BUNDLE_ID.binarycookies" "$HOME_DIR/Library/Preferences/$BUNDLE_ID.plist" \
         "$HOME_DIR/Library/Saved Application State/$BUNDLE_ID.savedState" "$HOME_DIR/Library/Logs/$BUNDLE_ID"; do
  { [ -e "$p" ] || [ -L "$p" ]; } && n=1
  remove "$p" "named by $BUNDLE_ID"
done
[ "$n" -eq 0 ] && echo "  none"

# 7. Echo.app, only with --app and only the bundle with Echo's id.
echo "Echo.app:"
for b in /Applications/Echo.app "$HOME_DIR/Applications/Echo.app"; do
  [ -d "$b" ] || continue
  if [ "$(bundle_id_of "$b")" != "$BUNDLE_ID" ]; then echo "  SKIP $b: bundle id isn't $BUNDLE_ID" >&2; continue; fi
  if [ "$app" -eq 1 ]; then remove "$b" "the app"; else echo "  kept $b (--app removes it)"; fi
done

# 8. What stays with the user: the CLI link outside the home folder, and permissions.
echo "For you to do (outside this script):"
if [ -L /usr/local/bin/echoctl ]; then
  case "$(readlink /usr/local/bin/echoctl)" in
    *Echo.app/Contents/MacOS/echoctl) echo "  sudo rm /usr/local/bin/echoctl      # Echo's command-line link" ;;
    *) echo "  /usr/local/bin/echoctl links elsewhere ($(readlink /usr/local/bin/echoctl)): left alone" ;;
  esac
fi
echo "  tccutil reset Microphone $BUNDLE_ID"
echo "  tccutil reset AudioCapture $BUNDLE_ID"
echo "  tccutil reset Accessibility $BUNDLE_ID"
echo "  tccutil reset ScreenCapture $BUNDLE_ID"

[ "$found" -eq 0 ] && echo "Nothing of Echo's found for this user."
exit 0
