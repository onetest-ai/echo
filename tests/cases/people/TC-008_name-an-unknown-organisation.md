---
id: TC-008
title: A person from an unknown domain offers "Name this organisation"
priority: low
type: functional
requirements: [F-PPL-07]
module: people
run: agent
platform: any
tags: [people, organisations]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-008: A person from an unknown domain offers "Name this organisation"

**Module:** People | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create the scratch thread with the Acme call: `echoctl project create "People lab"` (id `people-lab`), then `echoctl project include people-lab <folder of Acme weekly sync>` (the entry of `$ECHOCTL_HOME/calls` ending `-acme-weekly-sync`). People are placed per thread, so the case never touches Acme rollout
- In S-07: `people-lab` is the scenario's shared thread, made by stage 10's precondition (people/TC-001) and kept (other people may be in it already); its Teardown here deletes it; skip making it. On its own: make it as above

## Test Data

| Field | Value |
|-------|-------|
| Thread | People lab (`people-lab`), with the call Acme weekly sync |
| Person | Lena Fox <lena@contoso.example> |
| Organisation | Contoso |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/people-lab/people` and click "List" in the view switch (`role=group[name="View"]`); click "+ Person", type `Lena Fox <lena@contoso.example>`, press Enter. Wait up to 3 s after an action for the window to redraw before reading. | A row "Lena Fox" with the domain "contoso.example" |
| 2 | Click the row "Lena Fox" | The panel shows "lena@contoso.example · contoso.example" and the button "Name this organisation (contoso.example)" under Organisation |
| 3 | Click that button, type `Contoso` into the input "Organisation name" (placeholder "Who is contoso.example?"), click "Add". Wait up to 3 s after an action for the window to redraw before reading. | The panel's subtitle names "Contoso"; the button is gone |
| 4 | Run `echoctl org list` | An organisation `contoso` with `"account": false` and the domain `contoso.example` |

## Expected Final State
Contoso exists as an organisation that is not a client.

## Teardown
- `echoctl project delete people-lab`
- `echoctl org remove contoso`

## Changes
- Creates organisation `contoso`.
