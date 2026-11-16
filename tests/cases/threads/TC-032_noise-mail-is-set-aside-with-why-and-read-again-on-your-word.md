---
id: TC-032
title: An automatic reply is set aside before the model, with why in the sync and on the Work tab, and read again on your word
priority: high
type: functional
requirements: [F-THR-28, F-THR-20]
module: threads
run: agent
platform: macos
data: standard
ai: virtual
tags: [threads, sync, mail, noise, virtual-model]
---

# TC-032: An automatic reply is set aside before the model, with why, and read again on your word

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH`; the sync's script and an empty request log: `cp tests/fixtures/model-scripts/thread-sync.json <dir>/model/model-script.json; : > <dir>/model/model-log.jsonl`
- `$L` stands for `target/debug/echoctl project ledger acme-rollout` (write it out in full); `$E` for `$ECHOCTL_HOME/projects/acme-rollout/ledger/evidence`
- **A made-up mail item**, written as the mail sync writes one (no mailbox is used): `mail(){ python3 -c "import json,sys,datetime as d; t=(d.datetime.now().astimezone()-d.timedelta(days=1)).isoformat(); i,s,x,E=sys.argv[1:5]; json.dump({'id':i,'kind':'mail','source':'mail','at':t,'date':t[:10],'title':s,'from':{'name':'Dana Lee','email':'dana@acme.example'},'turns':[{'ts':'10:00:00','speaker':'Dana Lee','text':x}],'text':''},open(E+'/'+i+'.json','w')); open(E+'/'+i+'.md','w').write('# '+s+'\n\n[10:00:00] Dana Lee: '+x+'\n')" "$1" "$2" "$3" "$E"; }` — `mail <id> <subject> <text>`
- `requests` stands for `wc -l < <dir>/model/model-log.jsonl` (requests the model got so far)
- Besides `echoctl`, this case may run `python3 -c`, `cat`, `grep`, `wc` and `echo` in `<dir>`

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `$L pending`; `$L done --evidence <id>` for each item listed; then `$L run` | `$L pending` prints `[]`; the sync ends `state` "done" |
| 2 | Note `requests`. `mail mail-acme-ooo 'Automatic reply: Acme rollout status' 'I am out of the office until Monday.'`; run `$L run` | `state` "done", `processed` 1, no `run` id (no model run); `set_aside` `[{"id": "mail-acme-ooo", "title": "Automatic reply: Acme rollout status", "why": "an automatic reply"}]`; `message` has "mail-acme-ooo set aside, no work in it: an automatic reply." and "To read one again: echoctl project ledger acme-rollout read-again --evidence <item>, then Sync now."; `requests` as noted: the model was asked nothing |
| 3 | Run `$L pending`; `grep mail-acme-ooo $ECHOCTL_HOME/projects/acme-rollout/ledger/events.jsonl` | `[]`; one `processed` event for it with `"by":"filter"` and `"note":"set aside: an automatic reply"` |
| 4 | Open `{{base_url}}/#project/acme-rollout/work` | The sync line beside Sync now has "1 mail set aside" after "Synced HH:MM"; hovering "1 mail set aside" (its `title`) shows "Automatic reply: Acme rollout status: an automatic reply" |
| 5 | Run `$L read-again --evidence mail-acme-ooo`, then `$L pending` | `{"read_again": "mail-acme-ooo", "pending": 1}`; `pending` lists `mail-acme-ooo` |
| 6 | Run `$L run` | `state` "done" with a `run` id; `set_aside` `[]`: not set aside again; the model log's last extract request (system "# Echo: a thread's sync — extract") has "I am out of the office until Monday." (`python3 -c "import json; L=[json.loads(l) for l in open('<dir>/model/model-log.jsonl')]; e=[x for x in L if 'sync — extract' in json.dumps(x.get('system'), ensure_ascii=False)]; print('I am out of the office until Monday.' in json.dumps(e[-1]['messages'], ensure_ascii=False))"` prints `True`) |
| 7 | Reload `{{base_url}}/#project/acme-rollout/work` | The sync line has no "set aside" |
| 8 | Run `$L read-again --evidence mail-acme-nope` | An error: `unknown evidence "mail-acme-nope"` |

## Expected Final State
The automatic reply was set aside without asking the model, its reason kept in the ledger, the sync's message and the Work tab; on the user's word it was read by the extract run.

## Teardown
- `tests/fixtures/serve.sh down <dir>`

## Notes
- Noise is judged by rules only (no model judges it); what the virtual model's extract run makes of the mail doesn't matter here (the fake extraction is a call's).
