// Files dropped on the window: which plugin they go to (dropTarget) — the one whose view the address shows (its
// activity, its Knowledge tab) when it listens with ctx.onFileDrop; any other view, or a plugin that doesn't listen:
// none, and the drop stays Echo's own (a logo tile's). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/plugin-file-drop.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── plugin file drops:'), b = src.indexOf('// end of the plugin file drops block', a)
assert.ok(a >= 0 && b > a, 'the plugin file drops block not found in app.js')
const dropTarget = new Function(`${src.slice(a, b)}\nreturn dropTarget`)()

const listening = { 'acme-ingest': [() => {}], 'acme-quiet': [] }
// Its activity, at its root or deeper in its own route; its Knowledge tab.
assert.equal(dropTarget('#acme-ingest:main', listening), 'acme-ingest')
assert.equal(dropTarget('#acme-ingest:main/sources/new', listening), 'acme-ingest')
assert.equal(dropTarget('#brain/acme-ingest:ingest', listening), 'acme-ingest')
assert.equal(dropTarget('brain/acme-ingest:ingest/review', listening), 'acme-ingest', 'with or without the #')
// Not listening (none, or every handler gone), or not installed: none.
assert.equal(dropTarget('#acme-quiet:main', listening), null)
assert.equal(dropTarget('#acme-other:main', listening), null)
// Echo's own views — a thread's page with its logo tile, a call, Settings (a plugin's section too), Recall: none.
for (const h of ['', '#', '#today', '#calls', '#call/2026-11-03_1000_acme:kickoff/minutes', '#project/aurora/brief', '#client/acme',
  '#settings/acme-ingest:main', '#settings/plugins/acme-ingest', '#brain', '#brain/sources', '#link=echo://call/acme-ingest:x'])
  assert.equal(dropTarget(h, listening), null, h)
process.stdout.write('plugin-file-drop: ok\n')
