---
id: TC-232
title: A plugin reads the text of a file handed to it (Word, text, a PDF's pages) with ctx.files.text, capped, with no path in the answer, and a file that isn't text is refused
priority: high
type: functional
requirements: [F-PLUG-139]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, files]
---

# TC-232: A plugin reads the text of a file handed to it

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run: `tests/fixtures/serve.sh up <dir> <port>`; every Bash call starts with `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D` (abort if `$ECHOCTL_HOME` isn't inside `<dir>`); `E=target/debug/echoctl`, built right before
- Made-up files and a made-up plugin **Acme reader** (no `uses`):

```bash
mkdir -p "$D/reader" "$D/in"
echo '{"id":"acme-reader","name":"Acme reader","version":"0.1.0","main":"main.js"}' > "$D/reader/echo-plugin.json"
cat > "$D/reader/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('text', async (a, { files }) => await ctx.files.text(files[0], a))
}
JS
python3 - <<'PY'
import os, zipfile
d = os.environ["D"] + "/in"
open(d + "/Brief.md", "w").write("# Acme kickoff\nScope: Aurora phase 2.")
open(d + "/blob.bin", "wb").write(bytes([1, 0, 2, 0]))
with zipfile.ZipFile(d + "/Budget.docx", "w") as z:
    z.writestr("word/document.xml", '<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Umbrella budget</w:t></w:r></w:p><w:p><w:r><w:t>Q3 is approved</w:t></w:r></w:p></w:body></w:document>')
# A two-page PDF with one line on each page.
def content(t):
    s = f"BT /F1 14 Tf 20 100 Td ({t}) Tj ET"
    return f"<</Length {len(s)}>>\nstream\n{s}\nendstream"
objs = ["<</Type/Catalog/Pages 2 0 R>>", "<</Type/Pages/Kids[3 0 R 5 0 R]/Count 2>>",
        "<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 200]/Contents 4 0 R/Resources<</Font<</F1 7 0 R>>>>>>", content("Globex plan page one"),
        "<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 200]/Contents 6 0 R/Resources<</Font<</F1 7 0 R>>>>>>", content("Globex plan page two"),
        "<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>"]
out, offs = b"%PDF-1.4\n", []
for i, o in enumerate(objs, 1):
    offs.append(len(out)); out += f"{i} 0 obj\n{o}\nendobj\n".encode()
x = len(out)
out += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode() + b"".join(f"{o:010d} 00000 n \n".encode() for o in offs)
out += f"trailer\n<</Size {len(objs)+1}/Root 1 0 R>>\nstartxref\n{x}\n%%EOF\n".encode()
open(d + "/Plan.pdf", "wb").write(out)
PY
$E plugin install "$D/reader" --yes
```

**Changes:** the installed plugin; `$D/reader`, `$D/in`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin call acme-reader text '{}' --file "$D/in/Brief.md"` | `{"pages": null, "text": "# Acme kickoff\nScope: Aurora phase 2.", "truncated": false}`; no path of `$D` in the answer |
| 2 | `$E plugin call acme-reader text '{}' --file "$D/in/Budget.docx"` | `text` holds `Umbrella budget` and `Q3 is approved`; `pages` is `null` |
| 3 | `$E plugin call acme-reader text '{"max": 10}' --file "$D/in/Brief.md"` | `text` is `# Acme kic` and `truncated` is `true` |
| 4 | `$E plugin call acme-reader text '{}' --file "$D/in/blob.bin"` | Fails: `not a file with text in it (.bin)`; exit 1 |
| 5 | `$E plugin call acme-reader text '{"pages": "2-1"}' --file "$D/in/Brief.md"` | Fails: `pages is a number or a range like "2-5"` |
| 6 | Only where PDFium is installed (`scripts/fetch-pdfium.sh <dest>`, run with `ECHOCTL_PDFIUM_DIR=<dest>`; otherwise the call fails with `PDFium isn't installed` and the step is skipped): `$E plugin call acme-reader text '{}' --file "$D/in/Plan.pdf"`, then the same with `'{"pages": 2}'` | First: `pages` is `2`, `text` holds `Globex plan page one` and `Globex plan page two`. Second: `pages` is still `2`, `text` holds only `Globex plan page two` |

## Expected Final State
Acme reader is installed; nothing was written outside `$D`.

## Teardown
- `$E plugin remove acme-reader --delete-data`; `tests/fixtures/serve.sh down <dir>`
