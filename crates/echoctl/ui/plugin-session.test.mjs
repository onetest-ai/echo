// What a plugin's `ctx.claude.session(opts)` asks Echo for (`pluginSessionAsk`): only on the user's click or key
// (`copyAllowed`), in a folder it was given, `read` or `edit`; an open session is shown again, never started twice.
// Run by scripts/check-mod.sh (`node crates/echoctl/ui/plugin-session.test.mjs`): the blocks are taken from app.js as
// it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const block = (from, to) => {
  const start = src.indexOf(from)
  const end = src.indexOf(to, start)
  assert.ok(start >= 0 && end > start, `${from} not found in app.js`)
  return src.slice(start, end)
}
const { pluginSessionAsk, aboutExtra } = new Function(`${block('// ── copy guard:', '// end of the copy guard block')}
${block('// ── plugin session:', '// end of the plugin session block')}
return { pluginSessionAsk, aboutExtra }`)()

const click = { userActivation: { isActive: true } }
const vault = '/Users/sam/Acme vault'

// Without the user's click or key (a timer, a fetch's answer), or an engine that can't tell: refused before anything.
for (const nav of [{ userActivation: { isActive: false } }, {}, null]) {
  assert.throws(() => pluginSessionAsk('acme-vault', { folder: vault }, nav, null), /only on your click/)
}
// On a click: the plugin's own terminal, read by default, the default profile.
assert.deepEqual(pluginSessionAsk('acme-vault', { folder: vault }, click, null), {
  key: 'claude:plugin:acme-vault',
  extra: { plugin: 'acme-vault', folder: vault, path: '', mode: 'read', profile: '' },
  start: true,
  other: false,
})
// A subfolder, edit, a profile it names: passed on for Echo to check.
assert.deepEqual(pluginSessionAsk('acme-vault', { folder: vault, path: 'Notes', mode: 'edit', profile: 'work' }, click, null).extra,
  { plugin: 'acme-vault', folder: vault, path: 'Notes', mode: 'edit', profile: 'work' })
// No folder, or a mode a session hasn't (a copy, a permission mode of its own): refused.
assert.throws(() => pluginSessionAsk('acme-vault', {}, click, null), /in a folder you were given/)
assert.throws(() => pluginSessionAsk('acme-vault', { folder: 42 }, click, null), /in a folder you were given/)
for (const mode of ['copy', 'bypassPermissions', 'skip']) {
  assert.throws(() => pluginSessionAsk('acme-vault', { folder: vault, mode }, click, null), /reads \(read\) or edits \(edit\)/)
}
// Open already: in the same folder it is shown, not started again; in another one, `other`.
assert.deepEqual(pluginSessionAsk('acme-vault', { folder: vault }, click, { folder: vault, path: '' }), {
  key: 'claude:plugin:acme-vault', extra: { plugin: 'acme-vault', folder: vault, path: '', mode: 'read', profile: '' }, start: false, other: false,
})
assert.equal(pluginSessionAsk('acme-vault', { folder: vault, path: 'Notes' }, click, { folder: vault, path: '' }).other, true)
assert.equal(pluginSessionAsk('acme-vault', { folder: '/Users/sam/Other' }, click, { folder: vault, path: '' }).other, true)
// Found running after the window reopened: started again (Resume) as it was, on the profile it runs on — the same
// conversation, not the default's.
assert.deepEqual(aboutExtra({ plugin: 'acme-vault', name: 'Acme vault', folder: vault, path: 'Notes', mode: 'edit', profile: 'work', profile_name: 'Work', permissions: 'auto' }),
  { plugin: 'acme-vault', folder: vault, path: 'Notes', mode: 'edit', profile: 'work' })
assert.equal(aboutExtra({ plugin: 'acme-vault', folder: vault, mode: 'read' }).profile, '')
// The plugin's ctx: the guard first, and no way to type into the session or read it.
assert.ok(/session: \(opts\) => \{(?:(?!termStart|claudePanel)[\s\S]){0,300}?pluginSessionAsk\(id, opts,/.test(src), 'ctx.claude.session asks pluginSessionAsk first')
const ctx = block('function pluginCtx(id)', '/** Load the client halves')
assert.ok(!/pty_input|pty_paste|api\/pty\//.test(ctx), "a plugin's ctx neither types into a terminal nor reads one")
console.log("ok - a plugin opens a Claude session only on the user's click, in a folder it was given, read or edit, once")
