---
id: S-44
title: A plugin reads a web page of your own site as Markdown — once you list its address — saves its pictures and a file as handles, and is told why a page isn't read
run: agent
data: small
cases:
  - tests/cases/plugins/TC-190_a-plugin-reads-a-web-page-as-markdown-only-once-its-address-is-listed.md
  - tests/cases/plugins/TC-191_a-plugin-saves-a-pages-pictures-and-a-file-as-handles-in-its-cache.md
  - tests/cases/plugins/TC-192_a-plugin-is-told-why-a-page-isnt-read-not-html-moved-too-big-or-not-its-to-reach.md
  - tests/cases/plugins/TC-232_a-plugin-with-network-web-reads-a-public-https-page-and-its-redirects-never-this-computer.md
---

# S-44: A plugin reads a web page of your own site

**Story:** Sam keeps an intranet page about the Acme pilot on a small site of their own and installs Web demo, a plugin that reads pages. It reads nothing until Sam lists the site's address; then it gets the page as Markdown — its text, list, link and diagram with its caption, without the navigation, the footer or what the page's script would do — and learns that the dashboard page needs JavaScript. It saves the page's diagram and the pilot plan as files it can hand on, and is told plainly why a PDF, a moved page, a page past its size or an address it wasn't given isn't read. Then Sam installs Acme crawler, which asks to read the public web: its sheet says so on a line of its own, it reads a public https page and follows where it moved, and is refused plain http, another port, this computer and Sam's own network.

**Data:** `small`, served by the lead: `tests/fixtures/serve.sh up <dir> 48220 --set small`. Every Bash call starts with `. <dir>/env; E=target/debug/echoctl`.

**Length:** about 4 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build, served on 48220 | the lead (`serve.sh up`) | 1–4 |
| The made-up site `tests/fixtures/web-site` on 48221, the test marketplace copy and Web demo installed | stage 1 (its preconditions) | 1–3 (removed by stage 3's teardown) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-190_a-plugin-reads-a-web-page-as-markdown-only-once-its-address-is-listed.md` | — | Run its precondition lines as written, except `serve.sh up` (the lead's). Skip its teardown |
| 2 | `plugins/TC-191_a-plugin-saves-a-pages-pictures-and-a-file-as-handles-in-its-cache.md` | 1 | The site runs and its address is listed (stage 1): skip the precondition lines. Skip its teardown |
| 3 | `plugins/TC-192_a-plugin-is-told-why-a-page-isnt-read-not-html-moved-too-big-or-not-its-to-reach.md` | 1 | Skip the precondition lines. Its teardown is TC-190's: stop the site, remove Web demo and `test-plugins`, but not the lead's window server (`serve.sh down` is the lead's at the end) |
| 4 | `plugins/TC-232_a-plugin-with-network-web-reads-a-public-https-page-and-its-redirects-never-this-computer.md` | — | Run its precondition lines as written, except `serve.sh up` (the lead's): its own site on 48231, `ECHOCTL_TEST_HOSTS` added to `<dir>/env`. Its teardown, but not `serve.sh down` (the lead's) |

## Notes
- The site is made up and on this computer (`python3 -m http.server` over `tests/fixtures/web-site`); nothing is fetched from the network. Only the CLI is used; the host half runs in the terminal's own process (`echoctl plugin call`) as it does in the app.
- What the cases can't show — hostile pages, huge or deeply nested pages, pages built by JavaScript (JSON-LD, a framework's state, Open Graph), compressed and endless answers, a public page's pictures from their own hosts and one on this computer refused — is proven by the Rust tests `web::read::tests` and `jshost::web::tests`.
