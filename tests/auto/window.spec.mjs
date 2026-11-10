// The window on standard data: a meeting's header, a work item's person fields, the status bar.
import { test, expect } from '@playwright/test'
import { useData, covers } from './echo.mjs'

const echo = useData(test, { set: 'standard' })
test.describe.configure({ mode: 'serial' })

test("A meeting's header shows each person's answer, the most important first, who declined last; hover and +N more tell the rest", covers('prep/TC-008', 'F-PREP-02'), async ({ page }) => {
  await echo.open(page, '#upcoming/08bd9e1778b4234b')
  await expect(page.locator('.at-tally')).toHaveText('4 yes · 2 maybe · 2 no answer · 1 declined')
  const names = await page.locator('.at > .at-p').evaluateAll((els) => els.map((x) => [
    x.childNodes[1].textContent.trim(), x.querySelector('.at-s').getAttribute('aria-label'), x.classList.contains('key'), !!x.querySelector('.at-new')]))
  expect(names).toEqual([
    ['Kim Park', 'Organiser', true, false], ['Noor Aziz', 'Tentative', false, false], ['Mia Long', 'Accepted', false, true],
    ['Joel Brandt', 'Accepted', false, true], ['Ines Duarte', 'Tentative', false, true], ['Paul Weber', 'No answer yet', false, true],
    ['Sara Lind', 'Accepted', false, true]])
  await page.locator('.at-p.key').hover()
  await expect(page.locator('.at-p.key .at-card')).toBeVisible()
  await expect(page.locator('.at-p.key .at-card')).toContainText('Kim Park · Organiser')
  await expect(page.locator('.at-p.key .at-card')).toContainText('Influence high · interest high · 3 calls together')
  await page.locator('.at-more').hover()
  await expect(page.locator('.at-pop')).toBeVisible()
  await expect(page.locator('.at-pop .at-row')).toHaveText([/Theo Grant/, /Lena Vogel/])
  await expect(page.locator('.at-pop .at-row.declined')).toContainText('Lena Vogel')
})

test("A prep reads in one column with no Who's coming", covers('prep/TC-009', 'F-PREP-04'), async ({ page }) => {
  await echo.open(page, '#upcoming/e11de173ab857a8b')
  await expect(page.locator('.pr-sec h3').first()).toHaveText('In short')
  await expect(page.locator('.pr-side')).toHaveCount(0)
  await expect(page.locator('#pane')).not.toContainText("Who's coming")
  await expect(page.locator('.pr-foot')).toContainText('Written')
})

test("A work item's Handed over by suggests Me and the thread's people", covers('work/TC-012', 'F-WORK-11'), async ({ page }) => {
  await echo.open(page, '#project/acme-rollout/work')
  await page.locator('[data-pj="node"][data-node="T002"]').click()
  await page.locator('[data-pj="edit-field"][data-field="delegated_by"]').click()
  const box = page.locator('input[data-ledger-set="delegated_by"]')
  await expect(box).toHaveAttribute('list', 'wp-names')
  const names = await page.locator('#wp-names option').evaluateAll((os) => os.map((o) => o.value))
  expect(names[0]).toBe('Me')
  expect(names).toEqual(expect.arrayContaining(['Dana Lee', 'Omar Haddad']))
  await box.fill('Sam Rivera')
  await box.press('Enter')
  await expect(page.locator('[data-pj="edit-field"][data-field="delegated_by"]')).toHaveText('Sam Rivera')
})

test('The status bar shows Actions and Claude, and no AI profile', covers('shell/TC-014', 'F-WS-07'), async ({ page }) => {
  await echo.open(page, '#today')
  const bar = page.locator('#r-status')
  await expect(bar).toContainText('Actions')
  await expect(bar).toContainText('Claude')
  await expect(bar).not.toContainText('AI:')
})
