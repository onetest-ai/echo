// A plugin's window half: the stylesheets it may load (pluginStylesheetHref — a plain .css path inside its package,
// versioned), and what unloadPlugin takes out — its cleanups run (one that throws doesn't stop the others), its window
// and document listeners removed, its styles gone. Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/plugin-client-ctx.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── plugin stylesheet:'), b = src.indexOf('// end of the plugin stylesheet block', a)
assert.ok(a >= 0 && b > a, 'the plugin stylesheet block not found in app.js')
const href = new Function(`${src.slice(a, b)}\nreturn pluginStylesheetHref`)()

assert.equal(href('acme-greeter', 'view.css', '0.2.0'), 'plugin/acme-greeter/view.css?v=0.2.0')
assert.equal(href('acme-greeter', 'view.min.css', '1'), 'plugin/acme-greeter/view.min.css?v=1', 'a dot in a root file name')
assert.equal(href('acme-greeter', 'styles/dark.v2.css', '0.2.0:abc'), 'plugin/acme-greeter/styles/dark.v2.css?v=0.2.0%3Aabc', 'versioned: an update loads it afresh')
for (const bad of ['../x.css', 'styles/../../x.css', './x.css', 'a/./x.css', '.hidden.css', 'styles/.x.css', 'a//x.css', '/etc/x.css', 'a\\x.css', 'https://cdn.example/x.css', 'x.js', 'x.css?y', '.css', ''])
  assert.throws(() => href('acme-greeter', bad, '1'), /Not a stylesheet of the plugin/, bad)

// unloadPlugin, with the helper it calls (slotForget) and fakes for the page's document and window.
const listeners = { document: [], window: [] }
const target = (name) => ({ removeEventListener: (type, fn) => { listeners[name] = listeners[name].filter((l) => !(l[0] === type && l[1] === fn)) } })
const u = src.indexOf('function unloadPlugin('), v = src.indexOf('\n}\n', u)
const f = src.indexOf('function slotForget('), g = src.indexOf('\n}\n', f)
const ran = [], errors = []
const onResize = () => {}, onKey = () => {}, other = () => {}
listeners.window.push(['resize', onResize], ['resize', other])
listeners.document.push(['keydown', onKey])
let removed = 0
const P = {
  slots: {}, targets: {}, palette: [], sections: [], settings: {}, widgetDraw: {}, loaded: { kn: '1' }, applied: { kn: true },
  owned: { kn: { listeners: [['keydown', onKey]], windowListeners: [['resize', onResize]], styles: [{ remove: () => removed++ }],
    cleanups: [() => ran.push('timer'), () => { throw new Error('observer gone') }, () => ran.push('graph')] } },
}
const console = { error: (...m) => errors.push(m.map(String).join(' ')) }
const unload = new Function('Plugins', 'ACTIVITIES', 'PANEL_VIEWS', 'S', 'go', 'setTimeout', 'document', 'window', 'console',
  `${src.slice(f, g + 3)}\n${src.slice(u, v + 3)}\nreturn unloadPlugin`)(P, [], [], {}, () => {}, () => {}, target('document'), target('window'), console)
unload('kn')
assert.deepEqual(ran, ['timer', 'graph'], 'every cleanup ran, the one after a throw too')
assert.equal(errors.length, 1); assert.match(errors[0], /plugin kn: its cleanup failed:.*observer gone/)
assert.deepEqual(listeners.window, [['resize', other]], 'its window listener removed, another kept')
assert.deepEqual(listeners.document, [], 'its document listener removed')
assert.equal(removed, 1, 'its stylesheet taken out')
assert.equal(P.owned.kn, undefined)
process.stdout.write('plugin-client-ctx: ok\n')
