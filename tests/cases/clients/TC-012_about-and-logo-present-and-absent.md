---
id: TC-012
title: A client's About is Markdown or empty, and its logo shows in the head, the list and on threads that wear it
priority: medium
type: functional
requirements: [F-CLIENT-02, F-CLIENT-05]
module: clients
run: agent
platform: any
tags: [clients, about, logo, pairwise]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-012: A client's About is Markdown or empty, and its logo shows in the head, the list and on threads that wear it

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No client has been edited and no logo changed since the data was built
- In S-26 the fold "Closed · 3" of the Threads list is open from `threads/TC-018` (stage 1) when step 6 reads it; on its own, click "Closed · 3" open in step 6

## Test Data

| Client | About (Overview) | Logo in the head and the list | Logo row in the Profile |
|---|---|---|---|
| Acme | Markdown: heading "About Acme", "A manufacturer of everything, 4,000 people across Europe." (bold word), three bullets (Buyer: Rita Gomez (budget), Delivery: Dana Lee, "See their site" as a link) | an image, `src` `api/client/acme/logo?v=<n>` (relative to the window's address) | an image, the button "Change logo…" and a link "Remove" |
| Globex | empty: "Nothing yet — who they are, what you do for them, who matters." | a grey square with the letter "G" | the box "logo", the button "Add a logo…", no "Remove" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#client/acme/overview` | The block "About Acme" has the rendered Markdown of the table and a button "Edit"; the link "their site" has `href` `https://acme.example` |
| 2 | Read the head of the page | The client's logo is an `img` (`src` `api/client/acme/logo?v=<n>`, relative); the tag reads "client" |
| 3 | Run `curl -s -o /dev/null -w "%{http_code} %{content_type}" {{base_url}}/api/client/acme/logo` and the same for globex | Acme: `200 image/png`; Globex: `404` |
| 4 | Navigate to `{{base_url}}/#client/acme/profile` and read the row "Logo" | The image, "Change logo…" and "Remove"; the line "PNG, JPEG or WebP, up to 512 KB — shown beside its name and on its threads that wear it." |
| 5 | Navigate to `{{base_url}}/#client/globex/overview` and `#client/globex/profile` | The Overview says "Nothing yet — who they are, what you do for them, who matters."; the head shows the letter "G"; the Profile row "Logo" shows "logo" and "Add a logo…" |
| 6 | Navigate to `{{base_url}}/#projects` and look at the group Acme and at the row "Acme hackathon" (in the fold "Closed · 3") | The group Acme shows the logo beside its name; the row of Acme hackathon wears the same image as its icon (the thread uses the client's logo); Globex pilot wears its own logo |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- Does not press "Edit", "Change logo…", "Add a logo…" or "Remove": `TC-002` and `TC-004` edit them. Covers org.about (Markdown, empty) by org.logo (logo, none).
