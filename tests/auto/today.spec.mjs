// Today → Needs you now: I'll do it and Won't do (standard data, the fake Claude Code; nothing reaches a model).
import { test, expect } from '@playwright/test'
import path from 'node:path'
import { useData, covers } from './echo.mjs'

const echo = useData(test, { set: 'standard' })
test.describe.configure({ mode: 'serial' })

test("Won't do needs a comment; Cancel and Escape keep the item and decide nothing", covers('today/TC-007', 'F-TODAY-04'), async ({ page }) => {
  await echo.open(page, '#today')
  const dana = page.locator('[data-needs-wont="teams:19:dana-lee@unq.gbl.spaces"]')
  await dana.click()
  const ok = page.locator('[data-needs-wont-ok]')
  await expect(page.locator('#nd-dlg')).toHaveText("Won't do")
  await expect(ok).toBeDisabled()
  await page.locator('#nd-note').fill('   ')
  await expect(ok).toBeDisabled()
  await page.locator('#nd-note').fill('x')
  await expect(ok).toBeEnabled()
  await page.locator('.td-dialog [data-needs-off]').click()
  await expect(page.locator('.td-modal')).toHaveCount(0)
  await dana.click()
  await page.keyboard.press('Escape')
  await expect(page.locator('.td-modal')).toHaveCount(0)
  await expect(dana).toBeVisible()
  await expect(page.locator('section.wd')).toHaveCount(0)
  expect(JSON.parse(echo.read(echo.home, '.echo/needs.json')).decided || {}).toEqual({})
})

test("I'll do it adds the task in the chosen thread, and the dialog keeps its typed text and button across Today's redraw", covers('today/TC-013', 'F-TODAY-04'), async ({ page }) => {
  await page.clock.install()
  await echo.open(page, '#today')
  await page.locator('[data-needs-todo="mail:globex-redlines"]').click()
  await expect(page.locator('#nd-dlg')).toHaveText('A task of yours')
  await expect(page.locator('#nd-title')).toHaveValue('Contract redlines for the Globex pilot')
  await page.locator('#nd-title').fill('Send Kim the signed redlines')
  await page.locator('#nd-thread').selectOption({ label: 'Initech hiring' })
  // Marks on the dialog's button and on Today's page: a redraw replaces the page (its mark gone), never the dialog.
  await page.evaluate(() => { document.querySelector('[data-needs-todo-ok]').dataset.mark = '1'; document.querySelector('#r-primary main').dataset.mark = '1' })
  await page.clock.fastForward('01:05') // a minute on: Today's line "now" moves and Today is drawn again
  await expect(page.locator('#r-primary main[data-mark]')).toHaveCount(0)
  await expect(page.locator('[data-needs-todo-ok][data-mark]')).toHaveCount(1)
  await expect(page.locator('#nd-title')).toHaveValue('Send Kim the signed redlines')
  await expect(page.locator('#nd-thread')).toHaveValue('initech-hiring')
  await page.locator('[data-needs-todo-ok]').click()
  await expect(page.locator('.td-modal')).toHaveCount(0)
  await page.reload()
  await expect(page.locator('[data-needs-todo="teams:19:dana-lee@unq.gbl.spaces"]')).toBeVisible()
  await expect(page.locator('[data-needs-todo="mail:globex-redlines"]')).toHaveCount(0)
  const last = JSON.parse(echo.read(path.join(echo.home, 'projects/initech-hiring/ledger/events.jsonl')).trim().split('\n').at(-1))
  expect(last).toMatchObject({ type: 'create', kind: 'task', fields: { title: 'Send Kim the signed redlines', description: 'From Kim Park (Mail to you): Contract redlines for the Globex pilot' } })
})
