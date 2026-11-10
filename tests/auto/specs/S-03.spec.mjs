// S-03 Tidy the calls — every stage (small data): Initech planning, transcribed without minutes, offers to write them
// and shows its transcript; then it is deleted (asked first, kept once) and restored from the trash with the CLI.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CallsPage } from '../pages/calls.page.mjs'

const data = useData(test, { set: 'small' })

scenario(test, 'S-03', 'Tidy the calls: a call without minutes, delete one, take it back', () => {
  test('1 · A transcribed call without minutes says so, offers Write minutes and the session command; its transcript reads', covers('window/TC-006', 'F-MIN-06'), async ({ page }) => {
    const i = data.set.housekeeping.initech
    const c = data.text.calls
    const calls = new CallsPage(page, data)
    await calls.open()
    await calls.openCall(i.title)
    await expect(calls.tab(c.minutesTab)).toHaveAttribute('aria-selected', 'true')
    await expect(calls.main.getByRole('heading', { name: c.noMinutes, exact: true })).toBeVisible()
    await expect(calls.says(c.ready)).toBeVisible()
    await expect(calls.main.getByText(new RegExp(`^${literal(c.writesWith)}\\S+`))).toBeVisible()
    await expect(calls.writeMinutes).toBeVisible()
    await expect(calls.says(fill(c.command, { folder: data.callName(i.folder) }))).toBeVisible()
    await expect(calls.headerButton(c.copy)).toBeVisible()
    await calls.tab(c.transcriptTab).click()
    await expect(calls.tab(c.transcriptTab)).toHaveAttribute('aria-selected', 'true')
    await expect(calls.turns).toHaveCount(i.turns)
    await expect(calls.turnGroups).toHaveCount(i.turns)
  })

  test('2 · Delete asks first; Keep keeps the call; Delete moves it to the trash, and echoctl restore brings it back', covers('window/TC-008', 'F-CALLS-12', 'F-CALLS-13'), async ({ page }) => {
    const i = data.set.housekeeping.initech
    const c = data.text.calls
    const trash = path.join(data.home, c.run.trash)
    const name = data.callName(i.folder)
    const inTrash = () => (fs.existsSync(trash) ? fs.readdirSync(trash).filter((n) => n.endsWith(i.folder)) : [])
    let restored = null
    try {
      const calls = new CallsPage(page, data)
      await calls.open()
      await calls.openCall(i.title)
      const ask = calls.says(fill(c.deleteAsk, { title: i.title }))
      await calls.callAction(c.deleteItem)
      await expect(ask).toBeVisible()
      await expect(calls.headerButton(c.keep)).toBeVisible()
      await expect(calls.headerButton(c.delete)).toBeVisible()
      await calls.headerButton(c.keep).click()
      await expect(ask).toHaveCount(0)
      await expect(calls.call(i.title)).toBeVisible()
      await calls.callAction(c.deleteItem)
      await calls.headerButton(c.delete).click()
      await expect(calls.call(i.title)).toHaveCount(0)
      await expect.poll(inTrash).toHaveLength(1)
      expect(data.callName(i.folder)).toBeUndefined()

      const r = data.cli('restore', inTrash()[0])
      restored = r.code
      expect(r.code).toBe(0)
      await page.reload()
      await expect(calls.call(i.title)).toBeVisible()
      expect(inTrash()).toEqual([])
    } finally {
      // Teardown: the call back from the trash, even when a check failed after its delete.
      if (restored === null && inTrash().length) data.cli('restore', inTrash()[0])
    }
    expect(data.callName(i.folder)).toBe(name)
  })
})
