// Your day as "where you'll be" (app.js, agendaRows): a line a place with its reason — overlaps folded into the one you
// go to, a free gap carrying your work, no reply / maybe. Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/today-agenda.test.mjs`): the block is taken from app.js as it ships. Design: Workbench
// `WidgetsCalendar` (Your day · agenda).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── agenda rows:')
const end = src.indexOf('// end of the agenda rows block', start)
assert.ok(start >= 0 && end > start, 'the agenda rows block not found in app.js')
const agendaRows = new Function(`${src.slice(start, end)}\nreturn agendaRows`)()

const day = (h, m = 0) => new Date(2026, 10, 12, h, m)
const at = (h, m = 0) => day(h, m).toISOString()
const mt = (subject, h1, m1, h2, m2, response = 'accepted', extra = {}) => ({ kind: 'meeting', subject, event: subject, start: at(h1, m1), end: at(h2, m2), response, ...extra })
const tests = []
const test = (name, f) => tests.push([name, f])
const show = (r) => r.rows.map((x) => [x.kind === 'work' ? `+${x.task.title}` : x.item.subject, x.going, x.next ? 'next' : '', x.choice || '', x.why || ''].filter(Boolean).join(' ')).join(' | ')

test('a line a meeting, in time order; the next is the first you go to that is not over', () => {
  const r = agendaRows([mt('Standup', 9, 0, 9, 15), mt('Review', 11, 0, 12, 0), mt('1:1', 13, 0, 13, 30)], day(10, 40), [])
  assert.equal(show(r), 'Standup yes | Review yes next | 1:1 yes')
  assert.equal(r.rows[0].past, true)
})

test('no reply and maybe are lines of their own, said so; they are never the next', () => {
  const r = agendaRows([mt('Ops weekly', 10, 30, 11, 0, 'notResponded'), mt('Office hours', 16, 45, 17, 0, 'tentativelyAccepted'), mt('Review', 12, 0, 13, 0)], day(10, 0), [])
  assert.equal(show(r), 'Ops weekly noreply no reply | Review yes next | Office hours maybe maybe')
})

test('an overlap with one you said yes to: that one, "instead of" the other, the rest a count', () => {
  const r = agendaRows([mt('Globex review', 12, 0, 13, 0), mt('Initech intro', 12, 0, 12, 30, 'notResponded'), mt('Lunch talk', 12, 15, 12, 45, 'tentativelyAccepted')], day(9, 0), [])
  assert.equal(show(r), 'Globex review yes next made instead of Initech intro · 2 skipped')
  assert.deepEqual(r.skipped.map((x) => x.subject), ['Initech intro', 'Lunch talk'])
})

test('two you said yes to at once: a choice still to make, the first shown, the others at once a count', () => {
  const r = agendaRows([mt('Acme sync', 14, 30, 15, 30), mt('Hiring panel', 14, 30, 15, 0), mt('Vendor demo', 15, 0, 16, 0)], day(9, 0), [])
  assert.equal(show(r), 'Acme sync yes next pending 2 others at once')
  assert.equal(r.skipped.length, 2)
})

test('your work goes into the first free gap of 45 minutes or more after now, the late first, up to 2', () => {
  const tasks = [{ title: 'Budget split', late: true, id: 'T1', project: 'globex' }, { title: 'Room booking', id: 'T2', project: 'acme' }, { title: 'Third', id: 'T3', project: 'acme' }]
  const r = agendaRows([mt('Standup', 9, 0, 9, 15), mt('Review', 12, 30, 13, 0), mt('Sync', 13, 15, 14, 0), mt('Demo', 15, 30, 16, 0)], day(10, 50), tasks)
  // Gaps after 10:50 (rounded to 11:00): 11:00–12:30 (90), 13:00–13:15 (15, too short), 14:00–15:30 (90), 16:00–18:00.
  assert.equal(show(r), 'Standup yes | +Budget split work your work · 1 h | Review yes next | Sync yes | +Room booking work your work · 1 h | Demo yes')
  assert.equal(r.rows[1].at.getHours(), 11)
})

test('a day already over has no gaps; no tasks, no work lines', () => {
  assert.equal(agendaRows([mt('Standup', 9, 0, 9, 15)], day(19, 0), [{ title: 'X', id: 'T', project: 'p' }]).rows.filter((x) => x.kind === 'work').length, 0)
  assert.equal(agendaRows([mt('Standup', 9, 0, 9, 15)], day(8, 0), []).rows.length, 1)
})

test('a recorded call (no meeting) is a line you were at', () => {
  const r = agendaRows([{ kind: 'call', subject: 'A call', start: at(8, 0), end: at(8, 20) }, mt('Review', 12, 0, 13, 0)], day(10, 0), [])
  assert.equal(show(r), 'A call yes | Review yes next')
})

test('a long meeting you are unsure of does not chain the day: the ones you go to keep their lines', () => {
  const r = agendaRows([mt('Conference', 9, 0, 17, 0, 'notResponded'), mt('Standup', 9, 30, 9, 45), mt('Review', 14, 0, 15, 0)], day(8, 0), [])
  assert.equal(show(r), 'Standup yes next made instead of Conference · 1 skipped | Review yes')
})

test('your work never lands on a meeting you go to, even a folded one', () => {
  const r = agendaRows([mt('Acme', 14, 30, 15, 30), mt('Vendor', 15, 0, 17, 30)], day(14, 0), [{ title: 'Budget split', id: 'T1', project: 'p' }])
  assert.equal(r.rows.filter((x) => x.kind === 'work').length, 0, show(r))
})

test('an overlap with no yes is not a choice: it keeps its reason and counts the others', () => {
  const r = agendaRows([mt('Ops', 11, 0, 12, 0, 'notResponded'), mt('Lunch talk', 11, 30, 12, 30, 'tentativelyAccepted')], day(9, 0), [])
  assert.equal(show(r), 'Ops noreply no reply · 1 other at once')
})

test('a choice that is past is no longer one to make', () => {
  const r = agendaRows([mt('Acme', 9, 0, 10, 0), mt('Hiring', 9, 0, 9, 30)], day(11, 0), [])
  assert.equal(show(r), 'Acme yes 1 other at once')
})

test('the next is the line holding the first meeting you go to that is not over, as Next meeting says', () => {
  // Two you said yes to at once, the first over: the second, still on, is folded into its line — that line is the next.
  const r = agendaRows([mt('Acme', 9, 0, 10, 0), mt('Hiring', 9, 30, 11, 0), mt('Review', 14, 0, 15, 0)], day(10, 15), [])
  assert.equal(show(r), 'Acme yes next 1 other at once | Review yes')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
