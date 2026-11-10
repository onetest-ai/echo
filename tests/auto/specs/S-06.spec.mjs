// S-06 Shape the work — its stages with fixed checks (small data).
import { test, expect } from '@playwright/test'
import { useData } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { WorkPage } from '../pages/work.page.mjs'

const data = useData(test, { set: 'small' })

scenario(test, 'S-06', 'Shape the work', () => {
  test("4 · Handed over by suggests Me and the thread's people, and takes any name", covers('work/TC-012', 'F-WORK-11'), async ({ page }) => {
    const w = data.set.work
    for (const person of w.people) expect(data.cli('project', 'person', w.thread, person).code).toBe(0)
    const work = new WorkPage(page, data)
    await work.open(w.thread)
    await work.openItem(w.item)
    const field = data.text.work.handedOverBy
    const box = await work.edit(field)
    expect(await work.suggestions(box)).toEqual([data.text.work.me, ...w.people])
    await box.fill(w.otherName)
    await box.press('Enter')
    await expect(work.value(field)).toHaveText(w.otherName)
  })
})
