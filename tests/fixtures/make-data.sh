#!/usr/bin/env bash
# Build a made-up Echo data folder for functional tests: `tests/fixtures/make-data.sh <dir>`.
#
# <dir>/home is the data folder (ECHOCTL_HOME), <dir>/config the settings (ECHOCTL_CONFIG_DIR); <dir>/env sets
# both for a shell (`. <dir>/env`). Everything in it is invented (acme.example, globex.example): never real calls,
# people or companies. Times are relative to now, so Today has meetings behind and ahead of the line "now".
# The folder is rebuilt from nothing each time: give a folder of its own (a run's scratch folder), never ~/echo.
set -euo pipefail

dir="${1:?usage: make-data.sh <dir>}"
root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
echoctl="${ECHOCTL_BIN:-$root/target/debug/echoctl}"
[ -x "$echoctl" ] || { echo "no $echoctl: run cargo build first" >&2; exit 1; }
case "$dir" in "$HOME/echo"*|"$HOME") echo "refusing $dir: test data never goes into the real data folder" >&2; exit 1 ;; esac
if [ -e "$dir/home" ] || [ -e "$dir/config" ]; then
  echo "$dir already holds test data: give an empty folder" >&2
  exit 1
fi
mkdir -p "$dir/home" "$dir/config"
dir="$(cd "$dir" && pwd)"
export ECHOCTL_HOME="$dir/home" ECHOCTL_CONFIG_DIR="$dir/config"
# Outside any Claude Code session (a run inside one would be refused or marked as a child).
unset CLAUDECODE CLAUDE_CODE_ENTRYPOINT CLAUDE_CODE_CHILD_SESSION AI_AGENT
printf 'export ECHOCTL_HOME=%q ECHOCTL_CONFIG_DIR=%q\nunset CLAUDECODE CLAUDE_CODE_ENTRYPOINT CLAUDE_CODE_CHILD_SESSION AI_AGENT\n' "$ECHOCTL_HOME" "$ECHOCTL_CONFIG_DIR" > "$dir/env"

q() { "$echoctl" "$@" > /dev/null; }

q config --me-name "Sam Rivera" --me-email sam@initech.example --languages en
q client add Initech --domains initech.example --internal
q client add Acme --domains acme.example
q client add Globex --domains globex.example
q project create "Acme rollout" --client acme
q project create "Globex pilot" --client globex

# Calls and the agenda (Python writes the files: JSON and Markdown in Echo's own shapes).
python3 - "$ECHOCTL_HOME" <<'PY'
import json, os, sys
from datetime import datetime, timedelta

home = sys.argv[1]
now = datetime.now().astimezone()
day0 = now.replace(hour=0, minute=5, second=0, microsecond=0)
def at(hours):  # now + hours, kept inside today
    return max(day0, min(now + timedelta(hours=hours), now.replace(hour=23, minute=50)))
def iso(t): return t.isoformat(timespec="seconds")

me = {"name": "Sam Rivera", "email": "sam@initech.example"}
dana = {"name": "Dana Lee", "email": "dana@acme.example"}
omar = {"name": "Omar Haddad", "email": "omar@acme.example"}
kim = {"name": "Kim Park", "email": "kim@globex.example"}
lee = {"name": "Alex Chen", "email": "alex@initech.example"}

def call(start, minutes, title, event, people, lines, notes=None, actions=None):
    stamp = start.strftime("%Y-%m-%d_%H%M%S")
    slug = "-".join(title.lower().split())
    d = os.path.join(home, "calls", f"{stamp}-{slug}")
    os.makedirs(d)
    end = start + timedelta(minutes=minutes)
    json.dump({"app": "teams", "platform": "macos", "title": title, "languages": ["en"], "started_at": iso(start), "ended_at": iso(end),
               "stop_reason": "user", "audio": "deleted after transcription", "tracks": []}, open(f"{d}/meta.json", "w"), indent=2)
    json.dump({"id": event, "subject": title, "start": iso(start), "end": iso(end), "organizer": me, "attendees": people,
               "join_url": f"https://teams.example/{event}", "vocabulary": [], "source": "test"}, open(f"{d}/meeting.json", "w"), indent=2)
    segs, t = [], 5.0
    for who, text in lines:
        segs.append({"speaker": who, "start": t, "end": t + 6, "lang": "en", "text": text})
        t += 8
    json.dump(segs, open(f"{d}/segments.json", "w"), indent=2)
    with open(f"{d}/transcript.md", "w") as f:
        f.write(f"# {title}\n\n- **Recorded:** {iso(start)}\n- **Folder:** `{os.path.basename(d)}`\n\n")
        for s in segs:
            m, sec = divmod(int(s["start"]), 60)
            f.write(f"**[{m:02d}:{sec:02d}] {'Me' if s['speaker'] == 'me' else 'Remote'}:** {s['text']}\n\n")
    if notes:
        open(f"{d}/notes.md", "w").write(notes)
    if actions is not None:
        json.dump(actions, open(f"{d}/actions.json", "w"), indent=2)
    return os.path.basename(d)

acme = call(at(-3), 30, "Acme weekly sync", "ev-acme-sync", [me, dana, omar], [
    ("me", "Morning Dana, let's go through the SSO rollout."),
    ("remote", "We need the SSO checklist from you by the end of today."),
    ("me", "I'll send the SSO checklist this afternoon."),
    ("remote", "Omar will answer the security questionnaire by Friday."),
    ("me", "Then we agree: pilot starts with the Berlin office."),
], notes="""# Acme weekly sync

## Summary
- SSO rollout for Acme is on track; the pilot starts with the Berlin office [00:37]
- Acme waits for the SSO checklist [00:13]

## Decisions
- The pilot starts with the Berlin office [00:37]

## Action items
- [ ] Me — send Acme the SSO checklist today [00:21]
- [ ] Omar Haddad — answer the security questionnaire by Friday [00:29]

## Open questions
- Who signs off the pilot on Acme's side? [00:29]
""", actions=[
    {"id": "acme0001", "owner": "Me", "task": "Send Acme the SSO checklist", "due": now.strftime("%Y-%m-%d"), "quote": "I'll send the SSO checklist this afternoon.", "confidence": "high", "status": "open"},
    {"id": "acme0002", "owner": "Omar Haddad", "task": "Answer the security questionnaire", "due": None, "quote": "Omar will answer the security questionnaire by Friday.", "confidence": "high", "status": "open"},
])
plan = call(at(-1.5), 20, "Initech planning", "ev-initech-plan", [me, lee], [
    ("me", "Alex, what do we take into next sprint?"),
    ("remote", "The Globex pilot plan and the Acme checklist."),
])
yesterday = (now - timedelta(days=1)).replace(hour=14, minute=0, second=0, microsecond=0)
globex = call(yesterday, 45, "Globex pilot kickoff", "ev-globex-kickoff", [me, kim], [
    ("remote", "We'd like a pilot plan in two weeks."),
    ("me", "I'll draft the pilot plan and share it next week."),
], notes="""# Globex pilot kickoff

## Summary
- Globex wants a pilot plan in two weeks [00:05]

## Action items
- [ ] Me — draft the Globex pilot plan [00:13]
""", actions=[{"id": "glob0001", "owner": "Me", "task": "Draft the Globex pilot plan", "due": None, "quote": "I'll draft the pilot plan and share it next week.", "confidence": "high", "status": "open"}])

def meeting(event, subject, start, minutes, people, response="accepted"):
    return {"id": event, "subject": subject, "start": iso(start), "end": iso(start + timedelta(minutes=minutes)), "organizer": me,
            "attendees": people, "join_url": f"https://teams.example/{event}", "source": "test", "response": response}
agenda = [
    meeting("ev-acme-sync", "Acme weekly sync", at(-3), 30, [dana, omar]),
    meeting("ev-initech-plan", "Initech planning", at(-1.5), 20, [lee]),
    meeting("ev-globex-review", "Globex design review", at(1), 30, [kim]),
    meeting("ev-webinar", "Industry webinar", at(1.5), 60, [kim], response="notResponded"),
    meeting("ev-acme-demo", "Acme demo", at(2.5), 30, [dana]),
]
json.dump(agenda, open(os.path.join(home, "agenda.json"), "w"), indent=2)
open(os.path.join(home, "calls.txt"), "w").write(f"{acme}\n{plan}\n{globex}\n")
PY

"$echoctl" agenda set "$ECHOCTL_HOME/agenda.json" > /dev/null
rm "$ECHOCTL_HOME/agenda.json"
{ read -r acme; read -r plan; read -r globex; } < "$ECHOCTL_HOME/calls.txt"
rm "$ECHOCTL_HOME/calls.txt"
q project include acme-rollout "$acme"
q project include globex-pilot "$globex"

# The threads' work: an epic, a story, tasks on you and on others (each create prints the new item's id).
add() { "$echoctl" project ledger "$1" edit "$2" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])'; }
today="$(date +%F)"
epic="$(add acme-rollout '{"op":"create","kind":"epic","title":"Acme rollout"}')"
story="$(add acme-rollout "{\"op\":\"create\",\"kind\":\"story\",\"parent\":\"$epic\",\"title\":\"SSO for the Berlin pilot\",\"ac\":[\"SAML works for Berlin users\"]}")"
add acme-rollout "{\"op\":\"create\",\"kind\":\"task\",\"parent\":\"$story\",\"title\":\"Send Acme the SSO checklist\",\"internal_owner\":\"Sam Rivera\",\"due\":\"$today\"}" > /dev/null
add acme-rollout "{\"op\":\"create\",\"kind\":\"task\",\"parent\":\"$story\",\"title\":\"Answer the security questionnaire\",\"internal_owner\":\"Omar Haddad\"}" > /dev/null
epic="$(add globex-pilot '{"op":"create","kind":"epic","title":"Globex pilot"}')"
story="$(add globex-pilot "{\"op\":\"create\",\"kind\":\"story\",\"parent\":\"$epic\",\"title\":\"Pilot plan\"}")"
add globex-pilot "{\"op\":\"create\",\"kind\":\"task\",\"parent\":\"$story\",\"title\":\"Draft the Globex pilot plan\",\"internal_owner\":\"Sam Rivera\"}" > /dev/null

echo "test data in $dir (. $dir/env)"
