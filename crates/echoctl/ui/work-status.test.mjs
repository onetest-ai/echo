// What the Work tab's tree and board say of an item, and what its filters keep (app.js, the
// work-status block). Run by scripts/check-mod.sh (`node crates/echoctl/ui/work-status.test.mjs`):
// the block is taken from app.js as it ships, with `ymd` from the calendar's helpers.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

process.env.TZ = 'America/Los_Angeles'
assert.equal(new Date(2026, 10, 6, 22, 56).getTimezoneOffset(), 480, 'America/Los_Angeles not in effect')

const src = appSource()
const start = src.indexOf('// ── work-status:')
const end = src.indexOf('// end of the work-status block', start)
assert.ok(start >= 0 && end > start, 'the work-status block not found in app.js')
const ymd = src.match(/^const ymd = .*$/m)
assert.ok(ymd, '`const ymd` not found in app.js')
const { workStatus, cardCategory, workOwners, workFilter, cardLate, cardWaits, relDay } =
  new Function(`${ymd[0]}\n${src.slice(start, end)}\nreturn { workStatus, cardCategory, workOwners, workFilter, cardLate, cardWaits, relDay }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

test('a blocked task that also waits for someone is blocked in the tree and on the board (#125)', () => {
  const n = { kind: 'task', type: 'action', state: 'blocked', waiting_for: { who: 'Dana Lee', what: 'test accounts' } }
  assert.equal(cardCategory(n), 'blocked')
  assert.deepEqual(workStatus(n), ['blocked', 'blocked'])
  // Open or in progress and waiting: "waiting" in both.
  for (const state of ['open', 'in_progress']) {
    const m = { ...n, state }
    assert.equal(cardCategory(m), 'waiting')
    assert.deepEqual(workStatus(m), ['waiting', 'waiting'])
  }
})

test('a done card shows no waiting chip; an open one does (#124)', () => {
  const w = { who: 'Kim Park', what: 'a date' }
  assert.equal(cardWaits({ state: 'done', waiting_for: w }), false)
  assert.equal(cardWaits({ state: 'open', waiting_for: w }), true)
  assert.equal(cardWaits({ state: 'blocked', waiting_for: w }), true)
  assert.equal(cardWaits({ state: 'open' }), false)
})

test('late and today are judged by the local date, not the UTC one (#123)', () => {
  const evening = new Date(2026, 10, 6, 22, 56) // 06:56 UTC on the 7th
  assert.equal(cardLate({ due: '2026-11-06', state: 'open' }, evening), false)
  assert.equal(cardLate({ due: '2026-11-05', state: 'open' }, evening), true)
  assert.equal(cardLate({ due: '2026-11-05', state: 'done' }, evening), false)
  assert.equal(cardLate({ state: 'open' }, evening), false)
  const day = new Function(`${ymd[0]}\nreturn ymd`)()
  const y = new Date(); y.setDate(y.getDate() - 1)
  assert.equal(relDay(day(new Date())), 'today')
  assert.equal(relDay(day(y)), 'yesterday')
})

const all = [
  { id: 'T001', kind: 'task', state: 'done', internal_owner: 'Me' },
  { id: 'T002', kind: 'task', state: 'open', internal_owner: 'Omar Haddad' },
  { id: 'T010', kind: 'task', state: 'open', internal_owner: 'Sam Rivera' },
  { id: 'T009', kind: 'task', state: 'done', internal_owner: 'Alex Chen' },
  { id: 'T011', kind: 'task', state: 'in_progress' },
  { id: 'Q001', kind: 'question', state: 'done' },
  { id: 'T005', kind: 'task', state: 'cancelled', internal_owner: 'Omar Haddad' },
]
const me = { name: 'Sam Rivera', names: ['Me', 'Sam Rivera'] }
const V = (o) => ({ kind: '', owner: '', open: true, ...o })
const ids = (f) => all.filter(f).map((n) => n.id)

test('the user is one Owner option, "Me (Sam Rivera)", first among the names (#114)', () => {
  assert.deepEqual(workOwners(all, V(), me).map(([v, l]) => [v, l]), [['@me', 'Me (Sam Rivera)'], ['Alex Chen', 'Alex Chen'], ['Omar Haddad', 'Omar Haddad']])
  assert.deepEqual(workOwners(all, V({ owner: '@me' }), me).filter(([, , on]) => on).map(([v]) => v), ['@me'])
  // A view saved before, on either of the user's names, is on the user's option.
  assert.deepEqual(workOwners(all, V({ owner: 'Me' }), me).filter(([, , on]) => on).map(([v]) => v), ['@me'])
  // No name in About me: "Me"; nobody here is the user: no option for them.
  assert.deepEqual(workOwners(all, V(), { names: ['Me'] })[0].slice(0, 2), ['@me', 'Me'])
  assert.deepEqual(workOwners(all, V(), {}).map(([v]) => v), ['Alex Chen', 'Me', 'Omar Haddad', 'Sam Rivera'])
})

test('the user\'s option keeps every item they own, whichever name (#114)', () => {
  for (const owner of ['@me', 'Me', 'Sam Rivera']) {
    const f = workFilter(V({ owner, open: false }), me)
    assert.deepEqual(ids(f.pass), ['T001', 'T010'], owner)
  }
  assert.deepEqual(ids(workFilter(V({ owner: 'Omar Haddad', open: false }), me).pass), ['T002', 'T005'])
  assert.deepEqual(ids(workFilter(V({ owner: '-', open: false }), me).pass), ['T011'])
})

test('Done on the board follows the Owner and Kind filters; only Open only leaves it alone (#113)', () => {
  assert.deepEqual(ids(workFilter(V({ owner: 'Omar Haddad' }), me).board), ['T002'])
  assert.deepEqual(ids(workFilter(V({ kind: 'task' }), me).board), ['T001', 'T002', 'T010', 'T009', 'T011'])
  assert.deepEqual(ids(workFilter(V({ owner: '@me' }), me).board), ['T001', 'T010'])
  // Open only: the tree drops done items, the board keeps them in Done; cancelled ones are dropped by both.
  assert.deepEqual(ids(workFilter(V(), me).pass), ['T002', 'T010', 'T011'])
  assert.deepEqual(ids(workFilter(V(), me).board), ['T001', 'T002', 'T010', 'T009', 'T011', 'Q001'])
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`work-status: ${tests.length} passed`)
