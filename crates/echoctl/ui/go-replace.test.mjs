// A plugin's `ctx.go(hash, {replace: true})` (app.js, `go`): the address changes without a history step and the page is
// routed as on a hashchange; without it, a new history step as before; the same hash only redraws; ctx.go passes only a
// real `replace: true`. Run by scripts/check-mod.sh (`node crates/echoctl/ui/go-replace.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── go:'), b = src.indexOf('// end of the go block', a)
assert.ok(a >= 0 && b > a, 'the go block not found in app.js')
const log = []
const location = { hash: '#today' }
const make = () => new Function('location', 'render', 'replaceUrl', 'hashChanged', `${src.slice(a, b)}\nreturn go`)(
  location, () => log.push('render'), (h) => { log.push(`replace ${h}`); location.hash = h }, () => log.push('routed'))
const go = make()

go('tasks')
assert.deepEqual(log.splice(0), [], 'without replace: the address is set (a history step) and the browser fires hashchange')
assert.equal(location.hash, 'tasks')
location.hash = '#today'
go('call/2026-11-06_100000/minutes', { replace: true })
assert.deepEqual(log.splice(0), ['replace #call/2026-11-06_100000/minutes', 'routed'], 'replaced in place, then routed as on a hashchange')
go('call/2026-11-06_100000/minutes', { replace: true })
assert.deepEqual(log.splice(0), ['render'], 'the same hash only redraws')
location.hash = '#today'
go('tasks', { replace: false })
assert.deepEqual(log.splice(0), [], 'replace: false is a plain go')
assert.ok(/go: \(hash, opts\) => go\(hash, \{ replace: !!opts && opts\.replace === true \}\)/.test(src), 'ctx.go passes only a real replace: true')
assert.ok(/window\.addEventListener\('hashchange', hashChanged\)/.test(src), 'the hashchange listener is the same routing')
console.log('ok - go(hash, {replace: true}) changes the address without a history step and routes; plain go unchanged')
