// The window's search over what was said (app.js, the search request block): the query and filters as the request,
// the chips, the date read from the words, the labels, the marked words. Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/search.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── search request: filters, chips and labels')
const end = src.indexOf('// end of the search request block', start)
assert.ok(start >= 0 && end > start, 'the search request block not found in app.js')
const escLine = src.match(/^const esc = .*$/m)
assert.ok(escLine, 'esc not found in app.js')
const X = new Function(`${escLine[0]}\n${src.slice(start, end)}\nreturn { esc, sxNoFilters, sxRequest, sxSaidLabel, sxDays, sxChips, sxDropWords, sxMark, sxOpen, sxSection, sxWhen, sxNothing }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
const day = (iso) => ({ '2026-11-01': 'Nov 1', '2026-11-12': 'Nov 12', '2026-11-18': 'Nov 18', '2026-07-01': 'Jul 1', '2026-07-31': 'Jul 31' })[iso] || iso
const name = (kind, id) => ({ 'client:acme': 'Acme', 'thread:acme-rollout': 'Acme rollout' })[`${kind}:${id}`] || id
const f = (o = {}) => ({ ...X.sxNoFilters(), ...o })

test('the query and only the filters that are set make the request', () => {
  assert.equal(X.sxRequest(f(), 'sso cutover'), 'q=sso%20cutover')
  assert.equal(X.sxRequest(f({ kinds: ['call', 'mail'], sections: ['decisions'], client: 'acme', since: '2026-11-01' }), 'pen & test'),
    'q=pen%20%26%20test&kind=call%2Cmail&section=decisions&client=acme&since=2026-11-01')
  assert.equal(X.sxRequest(f({ thread: 'acme-rollout', person: 'Omar', until: '2026-07-31' }), ''), 'thread=acme-rollout&person=Omar&until=2026-07-31')
})

test('what was said is labelled by the kinds picked, every kind by calls, mail and channels', () => {
  assert.equal(X.sxSaidLabel([]), 'Said in calls, mail, channels')
  assert.equal(X.sxSaidLabel(['call']), 'Said in calls')
  assert.equal(X.sxSaidLabel(['mail']), 'Said in mail')
  assert.equal(X.sxSaidLabel(['channel', 'mail']), 'Said in mail, channels')
  assert.equal(X.sxSaidLabel(['document']), 'Said in documents')
})

test('days read as a chip: since, until, one day, a span', () => {
  assert.equal(X.sxDays('2026-11-01', '2026-11-18', day, '2026-11-18'), 'Since Nov 1')
  assert.equal(X.sxDays(null, '2026-07-31', day, '2026-11-18'), 'Until Jul 31')
  assert.equal(X.sxDays('2026-11-12', '2026-11-12', day, '2026-11-18'), 'Nov 12')
  assert.equal(X.sxDays('2026-07-01', '2026-07-31', day, '2026-11-18'), 'Jul 1 – Jul 31')
  assert.equal(X.sxDays(null, null, day, '2026-11-18'), '')
})

test('the active filters are chips in order; a date read from the words is a dashed guess with its words', () => {
  const res = { filters: { since: '2026-11-01', until: '2026-11-18', dates_from_query: 'since november' } }
  const chips = X.sxChips(f({ sections: ['decisions'], client: 'acme' }), res, name, day, '2026-11-18')
  assert.deepEqual(chips.map((c) => [c.key, c.label, !!c.guess]), [['section:decisions', 'Decisions', false], ['client', 'Acme', false], ['guess', 'Since Nov 1', true]])
  assert.equal(chips[2].title, 'Read from “since november” in your words')
  // Days the user picked: a plain chip, and the query's date isn't a second one.
  const picked = X.sxChips(f({ since: '2026-11-12', until: '2026-11-12', thread: 'acme-rollout', person: 'Omar' }), res, name, day, '2026-11-18')
  assert.deepEqual(picked.map((c) => [c.key, c.label, !!c.guess]), [['thread', 'Acme rollout', false], ['person', 'Said by Omar', false], ['days', 'Nov 12', false]])
  assert.deepEqual(X.sxChips(f(), null, name, day, '2026-11-18'), [])
  assert.deepEqual(X.sxChips(f(), { filters: { dates_from_query: null } }, name, day, '2026-11-18'), [])
})

test('removing the guessed date takes its words out of the query', () => {
  assert.equal(X.sxDropWords('sso cutover since November', 'since november'), 'sso cutover')
  assert.equal(X.sxDropWords('what happened yesterday at Acme', 'yesterday'), 'what happened at Acme')
  assert.equal(X.sxDropWords('sso cutover', null), 'sso cutover')
  assert.equal(X.sxDropWords('sso cutover', 'july'), 'sso cutover')
})

test('the words looked for are marked whole, any case, and every other text is escaped', () => {
  assert.equal(X.sxMark('If the cutover slips, the Cutover waits.', ['cutover'], X.esc), 'If the <mark>cutover</mark> slips, the <mark>Cutover</mark> waits.')
  assert.equal(X.sxMark('cutovers and precutover', ['cutover'], X.esc), 'cutovers and precutover', 'only whole words')
  assert.equal(X.sxMark('the SSO cutover, then SSO.', ['sso', 'cutover'], X.esc), 'the <mark>SSO cutover</mark>, then <mark>SSO</mark>.', 'words side by side are one mark')
  assert.equal(X.sxMark('<img src=x onerror=alert(1)> sso', ['sso', 'img'], X.esc), '&lt;<mark>img</mark> src=x onerror=alert(1)&gt; <mark>sso</mark>')
  assert.equal(X.sxMark('Дата переходу на SSO', ['переходу'], X.esc), 'Дата <mark>переходу</mark> на SSO', 'in any script')
  assert.equal(X.sxMark('a "quote" & more', [], X.esc), 'a &quot;quote&quot; &amp; more')
  assert.equal(X.sxMark(null, ['x'], X.esc), '')
})

test('two sources or fewer open whole, more open the first; the user’s toggles win', () => {
  const s = (...ids) => ids.map((source) => ({ source }))
  assert.deepEqual([...X.sxOpen(s('a', 'b'), {})], ['a', 'b'])
  assert.deepEqual([...X.sxOpen(s('a', 'b', 'c'), {})], ['a'])
  assert.deepEqual([...X.sxOpen(s('a', 'b', 'c'), { a: false, c: true })], ['c'])
})

test('a moment says its section and when: a time in the call, else its day', () => {
  assert.equal(X.sxSection({ section_kind: 'decisions', heading: 'Decisions' }), 'Decisions')
  assert.equal(X.sxSection({ section_kind: 'action_items', heading: 'Action items (3)' }), 'Action items')
  assert.equal(X.sxSection({ section_kind: 'minutes', heading: 'Risks' }), 'Risks')
  assert.equal(X.sxSection({ section_kind: 'transcript', heading: '[07:31] Dana Lee' }), 'Transcript')
  assert.equal(X.sxSection({ section_kind: 'channel' }), 'Post')
  const clock = (t) => `c${t}`
  assert.equal(X.sxWhen({ t: 468, day: '2026-11-18' }, clock, day), 'c468')
  assert.equal(X.sxWhen({ t: null, day: '2026-11-12' }, clock, day), 'Nov 12')
  assert.equal(X.sxWhen({}, clock, day), '')
})

test('no match is one line naming what was searched, with a wider search only when something narrowed it', () => {
  assert.deepEqual(X.sxNothing(f({ kinds: ['mail'], client: 'acme' }), 'pen test budget', name), { text: 'Nothing in Acme’s mail says “pen test budget”.', wider: true })
  assert.deepEqual(X.sxNothing(f(), 'zebra', name), { text: 'Nothing said matches “zebra”.', wider: false })
  assert.deepEqual(X.sxNothing(f({ thread: 'acme-rollout' }), 'zebra', name), { text: 'Nothing in Acme rollout’s sources says “zebra”.', wider: true })
  assert.deepEqual(X.sxNothing(f({ kinds: ['call', 'channel'] }), 'x', name), { text: 'Nothing in calls or channels says “x”.', wider: true })
  assert.equal(X.sxNothing(f({ sections: ['decisions'] }), 'x', name).wider, true)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n${e.stack}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
