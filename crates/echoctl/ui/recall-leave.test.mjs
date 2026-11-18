// Recall with no Knowledge tab goes to Calls only after the grace a plugin's update needs (app.js, leaveRecallLater).
// Run by scripts/check-mod.sh (`node crates/echoctl/ui/recall-leave.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── recall leave')
const end = src.indexOf('// end of the recall leave block', start)
assert.ok(start >= 0 && end > start, 'the recall leave block not found in app.js')

/** The block with its own state, a timer the test runs by hand, and the tabs and view the test sets. */
function load() {
  const S = { view: 'brain' }
  const env = { tabs: [], went: [], timers: [] }
  const go = (hash, opts) => env.went.push([hash, opts && opts.replace])
  const knowledgeTabs = () => env.tabs
  const later = (fn, ms) => { env.timers.push([fn, ms]); return env.timers.length }
  const { leaveRecallLater, RECALL_GRACE } = new Function('S', 'knowledgeTabs', 'go', `${src.slice(start, end)}\nreturn { leaveRecallLater, RECALL_GRACE }`)(S, knowledgeTabs, go)
  return { S, env, leave: () => leaveRecallLater(later), RECALL_GRACE }
}

// A plugin updating: its tab is gone, then back within the grace — the user stays on Recall.
{
  const { env, leave, RECALL_GRACE } = load()
  leave()
  leave()
  assert.equal(env.timers.length, 1, 'one wait at a time')
  assert.equal(env.timers[0][1], RECALL_GRACE)
  assert.equal(RECALL_GRACE, 1500, 'the grace unloadPlugin gives an activity')
  env.tabs = [{ tab: 'acme:notes' }]
  env.timers[0][0]()
  assert.deepEqual(env.went, [], 'the tab came back: no move')
}
// Switched off or removed: still no tab after the grace — Calls, replacing the address.
{
  const { env, leave } = load()
  leave()
  env.timers[0][0]()
  assert.deepEqual(env.went, [['calls', true]])
  leave()
  assert.equal(env.timers.length, 2, 'a later draw may wait again')
}
// The user left Recall meanwhile: nothing.
{
  const { S, env, leave } = load()
  leave()
  S.view = 'today'
  env.timers[0][0]()
  assert.deepEqual(env.went, [])
}
console.log('ok')
