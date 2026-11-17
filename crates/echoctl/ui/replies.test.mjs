// The app's answers to page commands (`Handled.reply`, `{fn, args}`): one dispatcher for the window and the served page,
// nothing evaluated (#725). Run by scripts/check-mod.sh (`node crates/echoctl/ui/replies.test.mjs`).
import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import { appSource } from './source.mjs'

const src = appSource()
const from = '// ── replies:', to = '// end of the replies block'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'replies block not found in app.js')
const { REPLIES, answerReply } = new Function(`${src.slice(start, end)}\nreturn { REPLIES, answerReply }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
const page = () => {
  const calls = []
  const echo = {}
  for (const n of REPLIES) echo[n] = (...a) => { calls.push([n, ...a]); return n }
  echo.refresh = () => calls.push(['refresh'])
  return { echo, calls }
}

test('a reply calls its handler with its arguments, in order', () => {
  const { echo, calls } = page()
  answerReply(echo, { fn: 'jobsSet', args: [{ req: 'r1', value: { jobs: [] } }] })
  answerReply(echo, { fn: 'templateSaved', args: [null, false, 'sk'] })
  answerReply(echo, { fn: 'pluginDrop', args: ['ingest', [{ name: 'Kickoff.md', file: 'h1' }]] })
  assert.deepEqual(calls, [['jobsSet', { req: 'r1', value: { jobs: [] } }], ['templateSaved', null, false, 'sk'], ['pluginDrop', 'ingest', [{ name: 'Kickoff.md', file: 'h1' }]]])
})

test('no arguments, or arguments that are not a list: the handler is called with none', () => {
  const { echo, calls } = page()
  answerReply(echo, { fn: 'notice' })
  answerReply(echo, { fn: 'notice', args: 'text' })
  assert.deepEqual(calls, [['notice'], ['notice']])
})

test('only the page\'s reply handlers: another of window.echo, an unknown name or a bad reply runs nothing', () => {
  const { echo, calls } = page()
  for (const r of [{ fn: 'refresh', args: [] }, { fn: 'constructor', args: [] }, { fn: 'toString' }, { fn: 'nope' }, {}, null, 'notice', { fn: ['notice'] }]) answerReply(echo, r)
  assert.deepEqual(calls, [])
  assert.equal(answerReply(undefined, { fn: 'notice', args: ['x'] }), undefined, 'no page yet: nothing')
  assert.equal(answerReply({}, { fn: 'notice', args: ['x'] }), undefined, 'a handler the page lacks: nothing')
})

test('nothing in the block evaluates a string', () => {
  const block = src.slice(start, end)
  assert.ok(!/\beval\s*[()]|new Function|setTimeout\(\s*['"`]/.test(block), block)
  assert.ok(!/\(0, eval\)/.test(src), 'no eval of a reply left in the page')
})

test('every reply the app sends is one the page dispatches, and the page lists none it never gets', () => {
  const dir = new URL('../src/window/', import.meta.url)
  const sent = new Set()
  for (const f of readdirSync(dir).filter((f) => f.endsWith('.rs'))) {
    for (const m of readFileSync(new URL(f, dir), 'utf8').matchAll(/Handled::answer\(\s*(?:true|false),\s*"(\w+)"/g)) sent.add(m[1])
  }
  assert.deepEqual([...sent].sort(), [...REPLIES].sort())
})

for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { console.error(`not ok - ${name}\n${e.stack}`); process.exitCode = 1 }
}
