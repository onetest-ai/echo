// When `ctx.ui.copy` puts text on the clipboard (`copyAllowed`): only while the page has the user's activation; an
// engine that can't tell refuses (fails closed). A guard against accidental copies, not a security boundary. Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/copy-guard.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── copy guard:')
const end = src.indexOf('// end of the copy guard block', start)
assert.ok(start >= 0 && end > start, 'the copy guard block not found in app.js')
const { copyAllowed } = new Function(`${src.slice(start, end)}\nreturn { copyAllowed }`)()

assert.equal(copyAllowed({ userActivation: { isActive: true } }), true, 'on the user\'s click or key')
assert.equal(copyAllowed({ userActivation: { isActive: false } }), false, 'a timer, a fetch\'s answer: no activation')
assert.equal(copyAllowed({ userActivation: { isActive: 'yes' } }), false, 'only a real true')
assert.equal(copyAllowed({}), false, 'an engine without userActivation refuses (fails closed)')
assert.equal(copyAllowed(null), false, 'no navigator: refused')
// Echo's own copy (Settings, a note's quote) doesn't go through it: only a plugin's ctx.ui.copy does.
assert.ok(/copy: \(text\) => \{\s*if \(!copyAllowed\(/.test(src), 'ctx.ui.copy asks copyAllowed first')
// A plugin's change to Echo's background jobs, too (echo-plugins R25): asked before anything is sent.
assert.ok(/set: \(job, settings\) => \{\s*if \(!copyAllowed\(/.test(src), "ctx.echo.jobs.set asks copyAllowed first")
// Showing a file of a granted folder in Finder / Explorer, too: asked before the request is sent.
assert.ok(/reveal: async \(root, path\) => \{\s*if \(!copyAllowed\(/.test(src), 'ctx.system.reveal asks copyAllowed first')
// A skill added to or removed from the user's own Claude Code, too: asked before the request is sent.
assert.ok(/installSkill: async \(name, md\) => \{\s*if \(!copyAllowed\(/.test(src), 'ctx.claude.installSkill asks copyAllowed first')
assert.ok(/removeSkill: async \(name\) => \{\s*if \(!copyAllowed\(/.test(src), 'ctx.claude.removeSkill asks copyAllowed first')
// Echo's sheet of a plugin's own routines, too: opened only on the user's click.
assert.ok(/offer: async \(ids\) => \{\s*if \(!copyAllowed\(/.test(src), 'ctx.echo.routines.offer asks copyAllowed first')
console.log('ok - a plugin copies, changes Echo\'s jobs, shows a file in Finder and adds or removes a Claude Code skill only with the user\'s activation; an engine that can\'t tell refuses')
