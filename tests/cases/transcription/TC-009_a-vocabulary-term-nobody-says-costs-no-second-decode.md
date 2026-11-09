---
id: TC-009
title: A vocabulary term nobody says costs no second decode and never appears in the transcript
priority: medium
type: functional
requirements: [F-VOC-04]
module: transcription
run: agent
platform: macos
tags: [transcription, vocabulary, recordings]
---

# TC-009: A vocabulary term nobody says costs no second decode

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; every command after `. <dir>/env`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `echoctl model use large-v3-turbo-q5_0`

## Test Data

| Call | Built from |
|---|---|
| `<day>_150000-acme-sync-vocabulary` | `acme-sync` (36 s, 6 turns: "SSO checklist", "Berlin first", Madrid, Dana) |
| `<day>_150500-acme-sync-plain` | `acme-sync` again, transcribed with no vocabulary (the decode count to compare with) |

Vocabulary: four terms nobody says (ServiceNow, Kubernetes, Terraform, Snowflake) and three the call says (SSO, Berlin, Madrid).

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | `echoctl config --vocabulary "ServiceNow,Kubernetes,Terraform,Snowflake,SSO,Berlin,Madrid"` | Exit 0 |
| 2 | `python3 tests/fixtures/audio/make_audio.py build acme-sync $ECHOCTL_HOME/calls/<day>_150000-acme-sync-vocabulary` | It prints the folder, "6 turns" |
| 3 | `ECHOCTL_DEBUG=1 echoctl transcribe $ECHOCTL_HOME/calls/<day>_150000-acme-sync-vocabulary 2> tc009.err` | Exit 0; `tc009.err` has "vocabulary hint: ServiceNow, Kubernetes, Terraform, Snowflake, SSO, Berlin, Madrid" |
| 4 | `echoctl config --vocabulary ""`; `python3 tests/fixtures/audio/make_audio.py build acme-sync $ECHOCTL_HOME/calls/<day>_150500-acme-sync-plain`; `ECHOCTL_DEBUG=1 echoctl transcribe $ECHOCTL_HOME/calls/<day>_150500-acme-sync-plain 2> tc009-plain.err` | Exit 0; no "vocabulary hint" line in `tc009-plain.err` |
| 5 | `grep -c 'debug: no_speech' tc009.err tc009-plain.err` | The same count in both: with the vocabulary each utterance is decoded once, as with none — the plain decodes already spell SSO, Berlin and Madrid, and nothing sounds like the other terms, so no hinted decode ran |
| 6 | `grep -c -E 'ServiceNow\|Kubernetes\|Terraform\|Snowflake' $ECHOCTL_HOME/calls/<day>_150000-acme-sync-vocabulary/transcript.md; grep -c -E 'SSO checklist\|Berlin first' $ECHOCTL_HOME/calls/<day>_150000-acme-sync-vocabulary/transcript.md` | 0 (no term nobody said); then 1 or more ("SSO checklist" and "Berlin first" as said) |

## Expected Final State
The call is transcribed with one decode per utterance and with none of the terms nobody said.

## Teardown
- Nothing: step 4 already cleared the run's own vocabulary

## Notes
- Short names sound like common words and ask for the hinted decode (in doubt, it runs): "Dana" ~ "And", "Grafana" ~ "Great. And". The vocabulary leaves them out.
