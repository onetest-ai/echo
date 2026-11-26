---
id: TC-015
title: A computer with no speech or speaker model of its own, using a transcription server, gets the server's model for each call's language and the speakers told apart by the server
priority: high
type: functional
requirements: [F-STT-12, F-STT-05]
module: transcription
run: agent
platform: macos
data: small
tags: [transcription, server, network, speakers, languages]
---

# TC-015: A client with no models gets everything from the server

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- Bash (each step's commands in one `bash` call: `$S` must split into words). Test data: `tests/fixtures/serve.sh up <dir> <port> --set small --ai fake` (the client); the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D; A=tests/fixtures/audio/make_audio.py; DAY=$(date +%F)`
- **The client has no model:** `ls "$ECHOCTL_CONFIG_DIR/models" 2>/dev/null | wc -l` → `0`, and no `nemo-speech` in its config
- **The server, a second Echo on this computer** with its own config and data, the models and the NeMo-Speech runtime of an installed Echo linked in (as `transcription/TC-011`: `parakeet-tdt-0.6b-v3`, `large-v3-turbo-q5_0`, `speakers-nemotron-3`): `S="env ECHOCTL_CONFIG_DIR=$D/srv-cfg ECHOCTL_HOME=$D/srv-home"; mkdir -p "$D/srv-cfg/models" "$D/srv-home"; for f in "$HOME/Library/Application Support/echo/models/"*; do ln -sf "$f" "$D/srv-cfg/models/"; done; ln -sfn /Applications/Echo.app/Contents/Frameworks/nemo-speech "$D/srv-cfg/nemo-speech"; $S target/debug/echoctl model use large-v3-turbo-q5_0; $S target/debug/echoctl config --languages en,uk`

## Test Data

| Call | Built from | Language |
|---|---|---|
| `<day>_120000-two-remotes-srv` | `two-remotes` (34 s; two people on the remote side) | en |
| `<day>_123000-ukrainian-srv` | `ukrainian-call` | uk |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Prelude; `S=…` as above; `$S target/debug/echoctl server share on \| tee "$D/share.txt"`; `nohup $S target/debug/echoctl server run --port 48091 > "$D/srv.out" 2>&1 & echo $! > "$D/srv.pid"`; `sleep 2`; `KEY=$(grep 'API key' "$D/share.txt" \| awk '{print $NF}')` (the API key `share on` printed); `target/debug/echoctl config --languages en,uk`; `target/debug/echoctl server use http://127.0.0.1:48091 --key "$KEY"` | "transcribing on <server name> (parakeet-tdt-0.6b-v3, …)": the server names its model for its default language (en), its English one, though its active one is turbo |
| 2 | Prelude; `C1="$ECHOCTL_HOME/calls/${DAY}_120000-two-remotes-srv"; python3 $A build two-remotes "$C1"; target/debug/echoctl transcribe "$C1" 2>&1 \| grep -E "model:\|speaker"; head -6 "$C1/transcript.md" \| grep Model; python3 -c "import json,sys; s=json.load(open(sys.argv[1])); print(sorted({x.get('name') or x['speaker'] for x in s if x['speaker']=='remote'}))" "$C1/segments.json"` | "model: parakeet-tdt-0.6b-v3 on <server name>"; "speaker model: …s on the transcription server"; "speakers: 2 on the remote track"; the header's Model is "parakeet-tdt-0.6b-v3 on <server name>"; two remote speakers (e.g. `['Speaker 1', 'Speaker 2']`) |
| 3 | Prelude; `C2="$ECHOCTL_HOME/calls/${DAY}_123000-ukrainian-srv"; python3 $A build ukrainian-call "$C2"; target/debug/echoctl language uk "$C2" 2>&1 \| grep "model:"; python3 $A score "$C2"` (setting its language transcribes it) | "model: large-v3-turbo-q5_0 on <server name>": the server's model for Ukrainian; `pass` true, `wer` ≤ 0.15, every turn `lang_ok` (uk) |
| 4 | Prelude; `ls "$ECHOCTL_CONFIG_DIR/models" 2>/dev/null \| wc -l`; `grep -c "transcription server: loading" "$D/srv.out"` | `0`: still no model on the client; the server loaded a model per language (`2`) |

## Expected Final State
Two calls transcribed on a computer with no model of its own: the English one with the server's English model and its remote speakers told apart by the server, the Ukrainian one with the server's model for Ukrainian.

## Teardown
- `kill $(cat "$D/srv.pid")`; `target/debug/echoctl server off`; `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-STT-12), not from the code. Found by the user (#905): with "transcribe on remote", transcripts came without speakers, and the server used one model whatever the call's language.
- The server being down (this computer takes over) is `transcription/TC-007`.
