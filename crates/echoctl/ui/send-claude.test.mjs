// What Escape and a focus on the skill field do to the Send to Claude dialog (app.js, ScKeys). Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/send-claude.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── send to claude keys:')
const end = src.indexOf('// end of the send to claude keys block', start)
assert.ok(start >= 0 && end > start, 'the send to claude keys block not found in app.js')
const { ScKeys } = new Function(`${src.slice(start, end)}\nreturn { ScKeys }`)()

// The window as it runs: Escape changes the state, the redraw then gives the replaced skill field its focus back
// (a focus Echo moved: `restored`), and the field's focus handler sees it (#108).
let c = { title: 'Write the follow-up mail', skill: '', open: false }
const focusField = (restored) => { if (ScKeys.opens(c, restored)) c.open = true }
const escape = () => { c = ScKeys.escape(c); if (c) focusField(true) }

// The user clicks the skill field: its list opens.
focusField(false)
assert.equal(c.open, true)
// First Escape: the list folds, and the redraw's focus does not open it again.
escape()
assert.ok(c, 'the first Escape closed the dialog: it folds the list first')
assert.equal(c.open, false, 'the list opened again after Escape')
// Second Escape: the dialog closes.
escape()
assert.equal(c, null)

// A focus the user moves to the folded field opens it; an open list stays as it is.
assert.equal(ScKeys.opens({ open: false }, false), true)
assert.equal(ScKeys.opens({ open: true }, false), false)
console.log('send-claude: ok')
