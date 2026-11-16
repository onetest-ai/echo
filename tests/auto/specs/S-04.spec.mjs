// S-04 The CLI on the same data — both stages (small data, no app run on it): echoctl project tasks sorts the open
// tasks into the user's and others' piles; echoctl status answers without the app.
import { test, expect } from '@playwright/test'
import { useData } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'

const data = useData(test, { set: 'small' })

/** Today as a `YYYY-MM-DD` date, on this computer's clock (the CLI's). */
const today = () => {
  const d = new Date()
  return [d.getFullYear(), d.getMonth() + 1, d.getDate()].map((n) => String(n).padStart(2, '0')).join('-')
}

scenario(test, 'S-04', 'The CLI on the same data: tasks by who has to move, the app\'s state', () => {
  test("1 · echoctl project tasks puts Sam's two tasks in mine and Omar's in others, each with its thread", covers('cli/TC-001', 'F-CLI-07', 'F-CLI-02'), async () => {
    const s = data.set.s04
    const r = data.cli('project', 'tasks')
    expect(r.code).toBe(0)
    const { piles } = r.json()
    expect(piles).toEqual(expect.any(Object))
    expect(piles.mine.map((t) => t.title).sort()).toEqual([...s.mine].sort())
    expect(piles.others.map((t) => t.title)).toEqual(s.others)
    expect(piles.mine.find((t) => t.title === s.dueToday.title)).toMatchObject({ project: s.dueToday.project, due: today() })
  })

  test("2 · echoctl status answers without the app: no watcher, nothing armed, today's agenda", covers('cli/TC-002', 'F-CLI-03'), async () => {
    const s = data.set.s04
    const r = data.cli('status')
    expect(r.code).toBe(0)
    const status = r.json()
    for (const k of s.keys) expect(status).toHaveProperty(k)
    expect(status.watcher).toBeNull()
    expect(status.armed).toBeNull()
    expect(status.agenda.map((m) => m.subject)).toEqual(s.agenda)
  })
})
