// The day's choices (app.js, dayChoices): meetings you accepted at once — which Echo suggests keeping, and why — and
// invites still to answer — accept when the time is free, decline when you go elsewhere then. Built on Your day's lines
// (agendaRows), so the two agree. Run by scripts/check-mod.sh (`node crates/echoctl/ui/today-choices.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const block = (from, to) => {
  const a = src.indexOf(from), b = src.indexOf(to, a)
  assert.ok(a >= 0 && b > a, `${from} not found in app.js`)
  return src.slice(a, b)
}
const dayChoices = new Function(`${block('// ── day choices:', '// end of the day choices block')}\nreturn dayChoices`)()

const day = (h, m = 0) => new Date(2026, 10, 12, h, m)
const at = (h, m = 0) => day(h, m).toISOString()
const mt = (subject, h1, m1, h2, m2, response = 'accepted', extra = {}) => ({ kind: 'meeting', subject, event: subject, start: at(h1, m1), end: at(h2, m2), response, ...extra })
const show = (cs) => cs.map((c) => (c.kind === 'overlap' ? `${c.members.map((x) => x.subject).join(' or ')} → ${c.pick.subject} (${c.why})` : `${c.item.subject}? ${c.suggest} (${c.why})`)).join(' | ')
const tests = []
const test = (name, f) => tests.push([name, f])

test('two you accepted at once: a choice, Echo suggesting the one you organise', () => {
  const cs = dayChoices([mt('Acme sync', 14, 30, 15, 30), mt('Hiring panel', 14, 30, 15, 0, 'organizer')], day(9, 0))
  assert.equal(show(cs), 'Acme sync or Hiring panel → Hiring panel (you organise it)')
})

test('none yours: the one in a thread, then the longer', () => {
  assert.equal(show(dayChoices([mt('Vendor demo', 11, 0, 12, 0), mt('Globex review', 11, 0, 11, 30, 'accepted', { threads: [{ id: 'g', name: 'Globex pilot' }] })], day(9, 0))), 'Vendor demo or Globex review → Globex review (in Globex pilot)')
  assert.equal(show(dayChoices([mt('Short one', 11, 0, 11, 30), mt('Long one', 11, 0, 12, 0)], day(9, 0))), 'Long one or Short one → Long one (the longer)')
})

test('an invite still to answer: accept when the time is free, decline when you go elsewhere then', () => {
  const cs = dayChoices([mt('Review', 12, 0, 13, 0), mt('Initech intro', 12, 0, 12, 30, 'notResponded'), mt('Ops weekly', 15, 0, 15, 30, 'notResponded')], day(9, 0))
  assert.equal(show(cs), 'Initech intro? decline (you go to Review then) | Ops weekly? accept (your day is free then)')
})

test('what is over is no longer a choice; maybes and lone meetings are none', () => {
  assert.equal(dayChoices([mt('Acme', 9, 0, 10, 0), mt('Hiring', 9, 0, 9, 30), mt('Old invite', 8, 0, 8, 30, 'notResponded')], day(11, 0)).length, 0)
  assert.equal(dayChoices([mt('Standup', 9, 0, 9, 15), mt('Office hours', 16, 0, 17, 0, 'tentativelyAccepted')], day(8, 0)).length, 0)
})

test('in time order, an overlap and an invite each once', () => {
  const cs = dayChoices([mt('Late invite', 16, 0, 16, 30, 'notResponded'), mt('A', 10, 0, 11, 0), mt('B', 10, 30, 11, 0)], day(9, 0))
  assert.equal(show(cs), 'A or B → A (the longer) | Late invite? accept (your day is free then)')
})

test('an invite at the time of an overlap names the one Echo suggests keeping; equal ones are "the first of them"', () => {
  const cs = dayChoices([mt('Office hours', 18, 0, 18, 30), mt('Board', 18, 0, 18, 30, 'organizer'), mt('Dry run', 18, 0, 18, 30, 'notResponded'), mt('X', 20, 0, 20, 30), mt('Y', 20, 0, 20, 30)], day(9, 0))
  assert.equal(show(cs), 'Office hours or Board → Board (you organise it) | Dry run? decline (you go to Board then) | X or Y → X (the first of them)')
})

test('a meeting already over is no member: Keep never declines the past', () => {
  assert.equal(dayChoices([mt('A', 9, 0, 10, 0), mt('B', 9, 30, 11, 0)], day(10, 15)).length, 0)
})

test('a chain of overlaps is one choice; a call is where you are, not a choice', () => {
  assert.equal(show(dayChoices([mt('A', 10, 0, 11, 0), mt('C', 10, 45, 11, 30), mt('B', 11, 15, 12, 0)], day(9, 0))), 'A or C or B → A (the longer)')
  const call = { kind: 'call', subject: 'A call', start: at(10, 0), end: at(10, 20) }
  assert.equal(show(dayChoices([call, mt('A', 10, 10, 11, 0), mt('B', 10, 30, 11, 0)], day(9, 0))), 'A or B → A (the longer)')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
