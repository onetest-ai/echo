// What a view loads on arrival (app.js, hashChanged). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/hash-changed.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── hash changed: what a view loads on arrival')
const end = src.indexOf('// end of the hash changed block', start)
assert.ok(start >= 0 && end > start, 'the hash changed block not found in app.js')

/** hashChanged on the calendar with `cached` as the data of the last visit; the copy drawn at each render. */
async function arrive(cached) {
  const S = { view: 'calendar', cal: { data: cached } }
  const drawn = []
  const noop = async () => {}
  const loadCalendar = async () => { S.cal.data = { events: [{ id: 'ev-m01', import: { error: 'not signed in to Microsoft 365' } }] } }
  const render = () => drawn.push(S.cal.data)
  const { hashChanged } = new Function('S', 'route', 'render', 'loadCalendar', 'loadSettings', 'loadProjects', 'loadProject',
    'loadTasks', 'loadToday', 'loadCall',
    `${src.slice(start, end)}\nreturn { hashChanged }`)(S, () => {}, render, loadCalendar, noop, noop, noop, noop, noop, noop)
  await hashChanged()
  return { S, drawn }
}

// An import failed from the Inbox after the last visit: the card must say so on coming back (#790).
const old = { events: [{ id: 'ev-m01' }] }
const back = await arrive(old)
assert.equal(back.drawn[0], old, 'the cached copy is drawn at once')
assert.equal(back.S.cal.data.events[0].import.error, 'not signed in to Microsoft 365', 'then the fresh copy replaces it')
assert.equal(back.drawn[back.drawn.length - 1], back.S.cal.data, 'and is drawn')
const first = await arrive(null)
assert.ok(first.S.cal.data, 'a first visit loads')
console.log('ok')
