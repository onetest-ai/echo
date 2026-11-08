// What the screen depends on (app.js, fingerprint): the poll redraws only when it changes. Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/fingerprint.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── fingerprint: what the screen depends on')
const end = src.indexOf('// end of the fingerprint block', start)
assert.ok(start >= 0 && end > start, 'the fingerprint block not found in app.js')
const S = { cal: {}, pj: {} }, NY = { data: null }
const fingerprint = new Function('S', 'NY', `${src.slice(start, end)}\nreturn fingerprint`)(S, NY)

// #115: the Inbox's count, loaded after the first render, showed only at the next redraw for another reason.
const before = fingerprint()
NY.data = { open: [{ key: 'signin:x', urgent: true }], done: [] }
const loaded = fingerprint()
assert.notEqual(loaded, before, 'the Inbox loaded: the screen changes (its badge)')
// #121: an item's reason changed (a run failed again): the open panel shows the new one.
NY.data = { open: [{ key: 'minutes:1', why: 'new reason' }], done: [] }
assert.notEqual(fingerprint(), loaded, 'an item changed: the screen changes (the open panel)')
console.log('fingerprint: ok')
