// Yolo runs (#603): which runs the filters let through (yoShown), the filters' choices (yoChoices), and the list's
// words — when, how long, what it changed and sent (yoWhen, yoTook, yoChanges).
// Run by scripts/check-mod.sh (`node crates/echoctl/ui/yolo-runs.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const from = '// ── yolo runs:', to = '// end of the yolo runs block'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'yolo runs block not found in app.js')
const { yoShown, yoChoices, yoFiltered, yoTook, yoWhen, yoChanges, yoThrough, yoKey } = new Function(`${src.slice(start, end)}\nreturn { yoShown, yoChoices, yoFiltered, yoTook, yoWhen, yoChanges, yoThrough, yoKey }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

const at = (y, m, d, h = 12) => new Date(y, m - 1, d, h, 5).toISOString()
const RUNS = [
  { id: 'a', kind: 'task', project: 'acme', node: 'T1', title: 'Book the room', thread: 'acme', thread_name: 'Acme rollout', state: 'done', changed: true, say: 'Booked the room and closed the task.', started: at(2026, 11, 14) },
  { id: 'b', kind: 'task', project: 'globex', node: 'T2', title: 'Collect the user list', thread: 'globex', thread_name: 'Globex pilot', state: 'done', changed: false, say: 'Nobody has sent it yet.', started: at(2026, 11, 13) },
  { id: 'c', kind: 'needs', item: 'mail:x', title: 'Mail to you', thread: 'globex', thread_name: 'Globex pilot', state: 'done', changed: false, say: 'Drafted a reply to Kim.', started: at(2026, 11, 10) },
  { id: 'd', kind: 'task', project: 'acme', node: 'T1', title: 'Book the room', thread: 'acme', thread_name: 'Acme rollout', state: 'failed', changed: false, say: 'Claude Code ended without a result', started: at(2026, 11, 1) },
]
const NONE = { state: '', task: '', thread: '', from: '', to: '', changed: false, q: '' }
const ids = (f) => yoShown(RUNS, { ...NONE, ...f }).map((r) => r.id)

test('no filter: every run', () => {
  assert.deepEqual(ids({}), ['a', 'b', 'c', 'd'])
  assert.equal(yoFiltered(NONE), false)
  assert.equal(yoFiltered({ ...NONE, q: '  ' }), false)
})

test('each filter narrows the list, and they add up', () => {
  assert.deepEqual(ids({ state: 'failed' }), ['d'])
  assert.deepEqual(ids({ state: 'done' }), ['a', 'b', 'c'])
  assert.deepEqual(ids({ task: 'task:acme/T1' }), ['a', 'd'], 'every run of that task')
  assert.deepEqual(ids({ task: 'item:mail:x' }), ['c'], 'an item of Needs you now')
  assert.deepEqual(ids({ thread: 'globex' }), ['b', 'c'])
  assert.deepEqual(ids({ changed: true }), ['a'], 'changed or sent through Echo (a draft is neither)')
  assert.deepEqual(ids({ from: '2026-11-10', to: '2026-11-13' }), ['b', 'c'], 'days inclusive')
  assert.deepEqual(ids({ from: '2026-11-14' }), ['a'])
  assert.deepEqual(ids({ q: 'KIM' }), ['c'], "what Claude said, any case")
  assert.deepEqual(ids({ q: 'user list' }), ['b'], 'the title too')
  assert.deepEqual(ids({ thread: 'acme', state: 'failed' }), ['d'])
  assert.deepEqual(ids({ thread: 'acme', q: 'kim' }), [])
  assert.equal(yoFiltered({ ...NONE, changed: true }), true)
})

test("the Task and Thread filters offer what the runs were on, each once, by name", () => {
  const ch = yoChoices(RUNS)
  assert.deepEqual(ch.tasks, [['task:acme/T1', 'Book the room'], ['task:globex/T2', 'Collect the user list'], ['item:mail:x', 'Mail to you']])
  assert.deepEqual(ch.threads, [['acme', 'Acme rollout'], ['globex', 'Globex pilot']])
  assert.equal(yoKey({ kind: 'task', title: 'Older run' }), 'title:Older run', "an older run without its task: by its title")
})

test('two tasks or items of one title are told apart, by thread, then by id', () => {
  const ch = yoChoices([
    { kind: 'task', project: 'acme', node: 'T1', title: 'Send the deck', thread: 'acme', thread_name: 'Acme rollout' },
    { kind: 'task', project: 'globex', node: 'T4', title: 'Send the deck', thread: 'globex', thread_name: 'Globex pilot' },
    { kind: 'task', project: 'globex', node: 'T5', title: 'Send the deck', thread: 'globex', thread_name: 'Globex pilot' },
    { kind: 'needs', item: 'mail:a', title: 'Mail to you · Hi' },
  ])
  assert.deepEqual(ch.tasks.map(([, l]) => l), ['Mail to you · Hi', 'Send the deck · Acme rollout', 'Send the deck · Globex pilot (T4)', 'Send the deck · Globex pilot (T5)'])
  assert.equal(new Set(ch.tasks.map(([, l]) => l)).size, ch.tasks.length)
})

test('what it did through Echo, counted in a few words', () => {
  assert.equal(yoThrough({ changes: 1, sends: 0, tried: 0 }), '1 change')
  assert.equal(yoThrough({ changes: 2, sends: 1, tried: 1 }), '2 changes, 1 send, 1 refused')
  assert.equal(yoThrough({ changes: 0, sends: 0, tried: 0 }), '')
  assert.equal(yoThrough(undefined), '')
})

test('when it started and how long it took, in plain words', () => {
  const now = new Date(2026, 10, 14, 18, 0)
  assert.equal(yoWhen(new Date(2026, 10, 14, 9, 7).toISOString(), now), 'Today 09:07')
  assert.equal(yoWhen(new Date(2026, 10, 13, 23, 59).toISOString(), now), 'Yesterday 23:59')
  assert.equal(yoWhen(new Date(2026, 10, 12, 14, 2).toISOString(), now), 'Thu 12 Nov 14:02')
  assert.equal(yoWhen(new Date(2025, 11, 30, 8, 0).toISOString(), now), 'Tue 30 Dec 2025 08:00')
  assert.equal(yoTook(null), '')
  assert.equal(yoTook(0), '0 s')
  assert.equal(yoTook(42), '42 s')
  assert.equal(yoTook(64), '1 min 4 s')
  assert.equal(yoTook(3720), '1 h 2 min')
})

test("its details' commands: by command; a refused one says it only tried; a draft is left to its drafts", () => {
  const r = { changes: [
    { kind: 'change', command: "project work acme '{}'" },
    { kind: 'send', command: 'm365 teams send x --yes' },
    { kind: 'draft', command: 'compose --to kim@acme.example' },
    { kind: 'change', command: 'project create X', outcome: 'failed: refused' },
  ] }
  assert.deepEqual(yoChanges(r), [['Changed', "project work acme '{}'", ''], ['Sent', 'm365 teams send x --yes', ''], ['Tried to change', 'project create X', 'failed: refused']])
  assert.deepEqual(yoChanges({}), [])
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} of ${tests.length} failed`); process.exit(1) }
console.log(`all ${tests.length} passed`)
