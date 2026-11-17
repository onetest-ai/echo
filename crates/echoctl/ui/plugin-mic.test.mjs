// When a plugin's window half may open the microphone (`ctx.microphone.open`, `micRefusal`): only on the user's click or
// key, in its own view (its activity, its Knowledge tab), while Echo records nothing and no other plugin has it; else
// the reason. Echo checks the plugin's install sheet and the recording again (`plugin_mic`). Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/plugin-mic.test.mjs`): the blocks are taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const slice = (from, to) => {
  const a = src.indexOf(from), b = src.indexOf(to, a)
  assert.ok(a >= 0 && b > a, `${from} not found in app.js`)
  return src.slice(a, b)
}
const { micRefusal, micView } = new Function(`${slice('// ── copy guard:', '// end of the copy guard block')}
${slice('// ── plugin microphone:', '// end of the plugin microphone block')}
return { micRefusal, micView }`)()

const click = { userActivation: { isActive: true } }
const timer = { userActivation: { isActive: false } }
const ok = (over) => micRefusal('acme-voice', { nav: click, hash: '#acme-voice:talk', recording: null, holder: null, ...over })

// The user's click in its own view: it may open — its activity at its root or deeper, its Knowledge tab.
assert.equal(ok({}), '')
assert.equal(ok({ hash: '#acme-voice:talk/session/2' }), '')
assert.equal(ok({ hash: '#brain/acme-voice:notes' }), '')
assert.equal(ok({ holder: 'acme-voice' }), '', 'its own microphone already open')
// No activation: a timer, an event, a fetch's answer — or an engine that can't tell (fails closed).
assert.equal(ok({ nav: timer }), 'the microphone opens only on your click')
assert.equal(ok({ nav: {} }), 'the microphone opens only on your click')
assert.equal(ok({ nav: null }), 'the microphone opens only on your click')
// Another view: Echo's own, another plugin's, its Settings section, Settings → Plugins.
for (const h of ['', '#today', '#calls', '#acme-other:talk', '#settings/acme-voice:main', '#settings/plugins/acme-voice', '#brain', '#link=echo://x/acme-voice:talk'])
  assert.equal(ok({ hash: h }), "the microphone opens only in the plugin's own view", h)
// Echo records a call or a memo: refused first, whatever else holds.
assert.equal(ok({ recording: '/calls/2026-11-17_100000_acme' }), 'Echo is recording — a plugin gets the microphone once the recording stops')
assert.equal(ok({ recording: '/calls/x', nav: timer }), 'Echo is recording — a plugin gets the microphone once the recording stops')
// Another plugin has it.
assert.equal(ok({ holder: 'globex-talk' }), 'another plugin has the microphone now')
// Its view: what closes it when the address leaves it.
assert.equal(micView('#acme-voice:talk'), 'acme-voice')
assert.equal(micView('brain/acme-voice:notes/x'), 'acme-voice')
assert.equal(micView('#calls'), null)

// ctx.microphone.open asks micRefusal before Echo or the webview; an unloaded plugin's microphone stops; leaving its
// view stops it; a state read that shows a recording stops it.
assert.ok(/async function micOpen\(id, audio\) \{[\s\S]*?const why = micRefusal\(/.test(src), 'micOpen asks micRefusal first')
assert.ok(/function unloadPlugin\(id\) \{\s*micClose\(id\)/.test(src), 'unloadPlugin closes its microphone')
assert.ok(/addEventListener\('hashchange', \(\) => \{ if \(Mic\.id && micView\(hashText\(\)\) !== Mic\.id\) micClose\(Mic\.id\) \}\)/.test(src), 'leaving its view closes it')
assert.ok(/await loadState\(\)\s*if \(netCheck\(\)\) return\s*micFollow\(\)/.test(src), 'each state read follows the recording')
process.stdout.write('plugin-mic: ok\n')
