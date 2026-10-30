// The window's workbench (app.js): the registry of activities, call tabs and calendar actions, and
// where panel views go (`Layout`). Run by scripts/check-mod.sh (`node crates/echoctl/ui/workbench.test.mjs`):
// the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const src = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const start = src.indexOf('// ── workbench: what the window is made of')
const end = src.indexOf('// end of the workbench block', start)
assert.ok(start >= 0 && end > start, 'the workbench block not found in app.js')
const { ACTIVITIES, callTabs, eventAction, Layout } = new Function(`${src.slice(start, end)}\nreturn { ACTIVITIES, callTabs, eventAction, Layout }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
const ids = (tabs) => tabs.map((t) => t[0])
const VIEWS = [{ id: 'claude', label: 'Claude', area: 'bottom' }, { id: 'tasks', label: 'Tasks', area: 'right' }]

test('Calls (list beside the call) comes first, Calendar (one column) second, Settings at the bottom', () => {
  assert.deepEqual(ACTIVITIES.map((a) => [a.id, a.layout, !!a.bottom]), [['calls', 'list-detail', false], ['calendar', 'single', false], ['projects', 'single', false], ['tasks', 'single', false], ['brain', 'single', false], ['settings', 'single', true]])
})

test('a call that records shows Live first; chat and screens only when there are any', () => {
  assert.deepEqual(ids(callTabs({ live: true, chat: 0, frames: 0, actions: null })), ['live', 'minutes', 'transcript', 'actions'])
  assert.deepEqual(callTabs({ live: false, chat: 3, frames: 2, actions: 4 }), [
    ['minutes', 'Minutes', ''], ['transcript', 'Transcript', ''],
    ['chat', 'Chat', 3], ['screens', 'Screens', 2], ['actions', 'Actions', 4],
  ])
})

test('Actions shows no count before the minutes, and 0 for a call without any', () => {
  assert.equal(callTabs({ live: false, chat: 0, frames: 0, actions: null }).at(-1)[2], '')
  assert.equal(callTabs({ live: false, chat: 0, frames: 0, actions: 0 }).at(-1)[2], 0)
})

test('a calendar event offers what the card offered before', () => {
  const st = (o) => ({ past: false, live: false, recordable: true, ...o })
  assert.equal(eventAction({ recorded: { name: 'x' } }, st({ recordable: false, past: true })), 'open-notes')
  assert.equal(eventAction({ scheduled: true }, st()), 'scheduled')
  assert.equal(eventAction({ join_url: 'https://teams' }, st({ live: true })), 'join')
  assert.equal(eventAction({ join_url: 'https://teams' }, st()), 'record')
  assert.equal(eventAction({}, st({ live: true })), 'record')
  assert.equal(eventAction({}, st({ recordable: false })), null)
})

test('no layout saved: views in their own areas, the right panel open, the bottom one closed', () => {
  Layout.load(undefined)
  const e = Layout.effective(1180, 760, VIEWS)
  assert.deepEqual([e.right.views, e.bottom.views], [['tasks'], ['claude']])
  assert.equal(e.right.shown, true)
  assert.equal(e.bottom.shown, false)
})

test('a saved layout that says nothing about a panel keeps its default (right open)', () => {
  Layout.load({})
  assert.equal(Layout.effective(1180, 760, VIEWS).right.shown, true)
  Layout.load({ bottom: { size: 300 } })
  assert.equal(Layout.effective(1180, 760, VIEWS).right.shown, true)
})

test('an area with no views is never drawn, even when saved open', () => {
  Layout.load({ right: { open: true }, bottom: { open: true } })
  const e = Layout.effective(1180, 760, [])
  assert.equal(e.right.shown, false)
  assert.equal(e.bottom.shown, false)
})

test('moving a view opens it in the other panel and remembers it', () => {
  Layout.load({})
  assert.ok(Layout.move('claude', 'right', VIEWS))
  const e = Layout.effective(1180, 760, VIEWS)
  assert.deepEqual(e.right.views, ['claude', 'tasks'])
  assert.equal(e.right.active, 'claude')
  assert.equal(e.right.shown, true)
  assert.deepEqual(e.bottom.views, [])
  const saved = Layout.toJSON()
  Layout.load(saved)
  assert.equal(Layout.area('claude', VIEWS), 'right')
})

test('a move to an unknown area or of an unknown view changes nothing', () => {
  Layout.load({})
  assert.equal(Layout.move('claude', 'left', VIEWS), false)
  assert.equal(Layout.move('nope', 'right', VIEWS), false)
  assert.deepEqual(Layout.toJSON().views, {})
})

test('a panel dragged to a size keeps it (within bounds), and a reset gives the default back', () => {
  Layout.load({})
  Layout.resize('right', 512)
  assert.equal(Layout.effective(1180, 760, VIEWS).right.size, 512)
  Layout.resize('right', 5000)
  assert.equal(Layout.toJSON().right.size, 900)
  Layout.resize('bottom', 10)
  assert.equal(Layout.toJSON().bottom.size, 160)
  Layout.resize('right', null)
  assert.equal(Layout.toJSON().right.size, 320)
})

test('toggle opens and closes a panel', () => {
  Layout.load({ right: { open: false } })
  assert.equal(Layout.effective(1180, 760, VIEWS).right.shown, false)
  Layout.toggle('bottom')
  assert.equal(Layout.effective(1180, 760, VIEWS).bottom.shown, true)
  Layout.toggle('bottom')
  assert.equal(Layout.effective(1180, 760, VIEWS).bottom.shown, false)
})

test('views of a removed plugin are forgotten', () => {
  Layout.load({ views: { gone: { area: 'right' }, claude: { area: 'right' } } })
  Layout.prune(VIEWS)
  assert.deepEqual(Object.keys(Layout.toJSON().views), ['claude'])
})

test('malformed saved values fall back to the defaults', () => {
  Layout.load({ right: { open: 'yes', size: 99999, active: 4 }, bottom: { size: 'big' }, views: { claude: { area: 'left' } } })
  const j = Layout.toJSON()
  assert.deepEqual(j.right, { open: true, size: 900, active: null }) // 'yes' isn't a choice: the default
  assert.equal(j.bottom.size, 220)
  assert.deepEqual(j.views, {})
})

test('narrow: the right panel opens over the page; short: the bottom one folds', () => {
  Layout.load({ right: { open: true }, bottom: { open: true } })
  const wide = Layout.effective(1180, 760, VIEWS)
  assert.equal(wide.right.overlay, false)
  assert.equal(wide.bottom.folded, false)
  // Narrow: not drawn until asked for in this session, then over the page.
  assert.equal(Layout.effective(800, 760, VIEWS).right.shown, false)
  assert.equal(Layout.effective(800, 760, VIEWS, true).right.overlay, true)
  assert.equal(Layout.effective(1180, 560, VIEWS).bottom.folded, true)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`workbench: ${tests.length} passed`)
