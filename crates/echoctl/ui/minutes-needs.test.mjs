// What the Minutes tab says when Claude Code or Echo's plugin is missing (app.js, minutesNeeds).
// Run by scripts/check-mod.sh (`node crates/echoctl/ui/minutes-needs.test.mjs`): the block is taken
// from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── minutes needs:')
const end = src.indexOf('// end of the minutes needs block', start)
assert.ok(start >= 0 && end > start, 'the minutes needs block not found in app.js')
const { minutesNeeds, CONNECT_WHERE } = new Function(`${src.slice(start, end)}\nreturn { minutesNeeds, CONNECT_WHERE }`)()

// Both there: nothing to say.
assert.deepEqual(minutesNeeds({ claude: true, plugin: true }), null)
// No Claude Code: install it (the link opens outside the window).
const [install, none] = minutesNeeds({ claude: false, plugin: true })
assert.match(install, /Install it from <a href="https:\/\/claude\.com\/claude-code" data-ext="1">/)
assert.equal(none, '')
// No plugin (#139): the window's own Settings → General, and a button that goes there — not the tray menu.
const [text, button] = minutesNeeds({ claude: true, plugin: false })
assert.equal(CONNECT_WHERE, 'Settings → General → Claude Code plugin → Connect')
assert.ok(text.includes(CONNECT_WHERE), text)
assert.doesNotMatch(text, /menu →/)
assert.match(button, /<button class="btn primary" data-setsec="general">Open Settings<\/button>/)
console.log('minutes-needs: ok')
