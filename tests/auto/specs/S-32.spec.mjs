// S-32 Model runs on the virtual model — the real Claude Code, its tools and Echo's guard; only the model is scripted.
import { test, expect } from '@playwright/test'
import { useData, textOf } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CallsPage } from '../pages/calls.page.mjs'

const data = useData(test, { set: 'small', ai: 'virtual' })

scenario(test, 'S-32', 'Model runs on the virtual model', () => {
  test("1 · Write minutes runs Claude Code with the call's prompt and only the run's tools; the minutes show", covers('claude/TC-023', 'F-MIN-13', 'F-CC-01'), async ({ page }) => {
    const { calls: c, virtualModel: vm } = data.set
    const calls = new CallsPage(page, data)
    data.model.clear()
    await calls.open()
    await calls.openCall(c.noMinutes.title, 'Minutes')
    await expect(page.getByText(vm.profile)).toBeVisible()
    await calls.writeMinutes.click()
    await expect(calls.minutes.getByText(vm.minutes)).toBeVisible({ timeout: 30_000 })
    await expect(calls.minutes.getByText(vm.actionLine)).toBeVisible()

    // What Echo handed the model.
    const turns = data.model.turns(vm.minutesRule)
    expect(turns).toHaveLength(vm.turns)
    expect(turns[0].model).toBe('virtual')
    expect([...new Set(turns[0].tools)].filter((t) => t !== 'DeferredToolPlaceholder').sort()).toEqual(vm.runTools)
    const prompt = textOf(turns[0].messages[0].content)
    expect(prompt).toContain('<command-args>' + data.home)
    expect(prompt).toContain('write the new action list to')
    expect(prompt).toContain('nobody is watching it')

    // What Echo did with the answers.
    const dir = data.callDir(c.noMinutes.folder)
    const log = data.read(dir, 'claude.log')
    expect(log).toMatch(/→ Read .*transcript\.md/)
    expect(log).not.toContain('✗')
    expect(log).toContain('Finished: ok')
    expect(data.json(dir, 'actions.json')).toEqual([expect.objectContaining({ ...vm.action, id: expect.any(String) })])
    expect(data.json(dir, 'notes-run.json').state).toBe('done')
  })

  test('2 · A model that tries to leave its call is refused by the guard each time; the previous minutes stay', covers('hostile/TC-019', 'F-CC-01', 'F-MIN-13'), async () => {
    const { calls: c, guardProbe: g } = data.set
    data.model.use(g.script, { OTHER_CALL: data.callName(c.other.folder) })
    data.model.clear()
    const dir = data.callDir(c.withMinutes.folder)
    const before = data.read(dir, 'notes.md')

    const run = data.cli('notes', dir, '--wait')
    expect(run.code).not.toBe(0)
    expect(run.err).toContain("The run didn't finish, so the previous minutes are kept")

    const log = data.read(dir, 'claude.log')
    expect(log.match(/✗/g)).toHaveLength(g.refusals.length)
    expect(log.match(/Not allowed:/g)).toHaveLength(g.refusals.length)
    for (const why of g.refusals) expect(log).toContain(why)
    expect(log).toContain('The previous minutes are back')
    expect(data.exists(data.home, 'calls/stolen.md')).toBe(false)
    expect(data.read(dir, 'notes.md')).toBe(before)
    expect(data.model.toolResults()).toEqual(g.toolResults)
  })
})
