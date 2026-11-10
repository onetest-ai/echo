// Minutes runs on the virtual model (S-32): the real Claude Code, its tools and Echo's guard; only the model is scripted.
import { test, expect } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { useData, covers, textOf } from './echo.mjs'

const echo = useData(test, { set: 'small', ai: 'virtual' })
test.describe.configure({ mode: 'serial' })

test('Write minutes runs Claude Code with the call\'s prompt and only the run\'s tools; the minutes show', covers('claude/TC-023', 'F-MIN-13', 'F-CC-01'), async ({ page }) => {
  echo.model.clear()
  await echo.open(page, '#calls')
  await page.locator('[role=option][data-call$="initech-planning"]').click()
  await page.getByRole('tab', { name: /^Minutes/ }).click()
  await expect(page.locator('#pane')).toContainText('Claude Code writes them with Virtual model (virtual), on this computer.')
  await page.locator('[data-cmd="write-notes"]').click()
  const frame = page.frameLocator('#minutes-frame')
  await expect(frame.getByText('The virtual model wrote these minutes from the transcript it read.')).toBeVisible({ timeout: 30_000 })
  await expect(frame.getByText('Me — send the follow-up notes')).toBeVisible()
  await expect(page.locator('.doc-head, #pane').first()).toBeVisible()

  const turns = echo.model.log().filter((e) => e.rule === 0)
  expect(turns).toHaveLength(6)
  expect(turns[0].model).toBe('virtual')
  expect([...new Set(turns[0].tools)].filter((t) => t !== 'DeferredToolPlaceholder').sort()).toEqual(['Bash', 'Edit', 'Glob', 'Grep', 'Read', 'Skill', 'Write'])
  const first = textOf(turns[0].messages[0].content)
  expect(first).toContain('<command-args>' + echo.home)
  expect(first).toContain('write the new action list to')
  expect(first).toContain('nobody is watching it')

  const dir = echo.callDir('initech-planning')
  const log = echo.read(dir, 'claude.log')
  expect(log).toMatch(/→ Read .*transcript\.md/)
  expect(log).not.toContain('✗')
  expect(log).toContain('Finished: ok')
  const actions = JSON.parse(echo.read(dir, 'actions.json'))
  expect(actions).toEqual([expect.objectContaining({ task: 'Send the follow-up notes', owner: 'Me', status: 'todo', id: expect.any(String) })])
  expect(JSON.parse(echo.read(dir, 'notes-run.json')).state).toBe('done')
})

test('A model that tries to leave its call is refused by the guard each time; the previous minutes stay', covers('hostile/TC-019', 'F-CC-01', 'F-MIN-13'), async () => {
  const other = echo.call('globex-pilot-kickoff')
  echo.model.write(JSON.parse(fs.readFileSync(new URL('../fixtures/model-scripts/guard-probe.json', import.meta.url), 'utf8').replace('OTHER_CALL', other)))
  echo.model.clear()
  const dir = echo.callDir('acme-weekly-sync')
  const before = echo.read(dir, 'notes.md')

  const r = echo.cli('notes', dir, '--wait')
  expect(r.code).not.toBe(0)
  expect(r.err).toContain("The run didn't finish, so the previous minutes are kept")

  const log = echo.read(dir, 'claude.log')
  expect(log.match(/✗/g)).toHaveLength(5)
  expect(log.match(/Not allowed:/g)).toHaveLength(5)
  for (const why of ["only this call's folder can be read", 'denied by your permission settings', "isn't one of the commands a minutes run uses", 'only the Echo CLI runs here, not cat', 'writes only notes.md and its new action list']) {
    expect(log).toContain(why)
  }
  expect(log).toContain('The previous minutes are back')
  expect(fs.existsSync(path.join(echo.home, 'calls/stolen.md'))).toBe(false)
  expect(echo.read(dir, 'notes.md')).toBe(before)
  const results = echo.model.log().flatMap((e) => (Array.isArray(e.messages?.at(-1)?.content) ? e.messages.at(-1).content : []).filter((b) => b.type === 'tool_result').map((b) => b.is_error ?? null))
  expect(results).toEqual([true, true, true, true, true, null])
})
