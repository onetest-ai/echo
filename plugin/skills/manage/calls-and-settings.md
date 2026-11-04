# Echo: during a call, the calendar, vocabulary, AI profiles, plugins, settings

`echoctl` below is `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl`.

Contents: [During a call](#during-a-call) · [Calendar and Teams transcripts](#calendar-and-teams-transcripts) ·
[Transcripts, speakers, vocabulary](#transcripts-speakers-vocabulary) · [AI profiles](#ai-profiles) ·
[Plugins](#plugins) · [Settings](#settings)

## During a call

Recording itself is the `record` skill. While a call records:

```sh
echoctl live show [call]                       # its live notes, answers and state (default: the latest call)
echoctl live ask <call> --kind missed          # what was said in the last 5 minutes
echoctl live ask <call> --kind question        # a question worth asking now
echoctl live ask <call> "did they agree on the date?"   # anything, about the call so far
echoctl live pause <call> | live resume <call> # the live notes (they send the call's text to its AI profile)
echoctl note "text"                            # the user's own note, at this moment of the call
```

Answers come back into `live show` a few seconds later (the call's AI profile writes them).

## Calendar and Teams transcripts

```sh
echoctl calendar events [--days 3]             # the user's meetings from today
echoctl agenda sync                            # today's meetings Echo offers to record
echoctl join <event>                           # open a meeting in Teams and record it
echoctl calendar transcripts [--days 3]        # ended Teams meetings with no call whose transcript Teams has
echoctl calendar import "<subject words>" [--date 2026-11-03]   # make a call of one, from its Teams transcript
echoctl calendar chat <call>                   # fetch a recorded call's meeting chat again
echoctl calendar login | logout                # the Microsoft 365 sign-in (a browser)
```

## Transcripts, speakers, vocabulary

```sh
echoctl speakers <call>                        # the voices Echo told apart, with their first lines
echoctl speakers <call> 2="Jane Doe" 3="Raj Patel"   # name them (transcript re-written)
echoctl speakers <call> --relabel              # name them again from Teams' speaking ring
echoctl transcribe <call>                      # transcribe again (the call's audio must still be there)
echoctl vocab list                             # always-on and learned terms
echoctl vocab add "ServiceNow" "RACI" [--call <call>] [--always]   # spellings Whisper should get right
echoctl vocab remove "<term>"
echoctl delete <call> | restore <call>         # to the trash for 30 days, and back
```

Thread terms (`project term`) and the Brain's terms (the `brain` skill) also hint transcription.

## AI profiles

Which model and account writes a call's minutes and live notes.

```sh
echoctl ai status                              # profiles, the default, sign-ins
echoctl ai default <profile>                   # what new calls use
echoctl ai set <profile> --notes-model sonnet  # a profile's minutes model
echoctl ai live on | off                       # live notes during calls
echoctl ai add-local <name> --url http://127.0.0.1:11434 --model <model>   # Ollama on this computer
echoctl ai add-claude <name> [--email …]       # another claude.ai account; sign it in from the Echo window
echoctl ai login <profile>                     # sign it in here, in a terminal
```

A call's profile is set when it starts (`echoctl arm --ai <profile>`); minutes always use the call's own profile.

## Plugins

```sh
echoctl plugin list                            # installed, offered, marketplaces
echoctl plugin install <id>                    # shows what it may touch; --yes agrees — ask the user first
echoctl plugin update [<id>] | enable <id> | disable <id> | remove <id>
echoctl plugin set <id> <key> [<value>]        # a setting (no value: show it)
echoctl plugin call <id> <command> ['{json}']  # run one of its commands
echoctl plugin marketplace list | add <owner/repo> | remove <name> | update
```

## Settings

`echoctl config` with no flags prints them. One flag a setting:

| Flag | What |
| --- | --- |
| `--languages en,uk,ru` | languages spoken in calls (`any` clears) |
| `--scheduled ask\|manual`, `--unscheduled ask\|manual` | ask to record calendar meetings / other calls, or only on request |
| `--auto-minutes true\|false` | minutes by themselves, 15 minutes after a call |
| `--notes-model sonnet\|opus\|haiku\|default` | the built-in profile's minutes model |
| `--me-name`, `--me-email` | who "Me" is (colleagues vs outside people) |
| `--capture-frames true\|false` | snapshots of shared screens (macOS) |
| `--keep-audio true\|false` | keep audio after transcription (default: text only) |
| `--minutes-page true\|false` | also write each call's minutes as an HTML page |

Elsewhere: `echoctl me` (the path of `me.md`, the profile minutes read); `echoctl server status | use <url> --key … |
off | share on|off` (transcription on another computer); `echoctl brain set --enabled true|false` (the Brain).
