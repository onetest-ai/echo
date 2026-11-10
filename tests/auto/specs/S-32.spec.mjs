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
    await calls.openCall(c.noMinutes.title, data.text.calls.minutesTab)
    await expect(page.getByText(vm.profile)).toBeVisible()
    await calls.writeMinutes.click()
    await expect(calls.minutes.getByText(vm.minutes)).toBeVisible({ timeout: 30_000 })
    await expect(calls.minutes.getByText(vm.actionLine)).toBeVisible()

    // What Echo handed the model.
    const turns = data.model.turns(vm.minutesRule)
    expect(turns).toHaveLength(vm.turns)
    expect(turns[0].model).toBe('virtual')
    const run = data.text.minutesRun
    expect([...new Set(turns[0].tools)].filter((t) => !run.ignoredTools.includes(t)).sort()).toEqual(vm.runTools)
    const prompt = textOf(turns[0].messages[0].content)
    expect(prompt).toContain('<command-args>' + data.home)
    for (const words of run.promptHas) expect(prompt).toContain(words)

    // What Echo did with the answers.
    const dir = data.callDir(c.noMinutes.folder)
    const log = data.read(dir, run.files.log)
    expect(log).toMatch(new RegExp(run.logRead))
    expect(log).not.toContain(run.logRefused)
    expect(log).toContain(run.logOk)
    expect(data.json(dir, run.files.actions)).toEqual([expect.objectContaining({ ...vm.action, id: expect.any(String) })])
    expect(data.json(dir, run.files.run).state).toBe('done')
  })

  test('2 · A model that tries to leave its call is refused by the guard each time; the previous minutes stay', covers('hostile/TC-019', 'F-CC-01', 'F-MIN-13'), async () => {
    const { calls: c, guardProbe: g } = data.set
    const run = data.text.minutesRun
    data.model.use(g.script, { OTHER_CALL: data.callName(c.other.folder) })
    data.model.clear()
    const dir = data.callDir(c.withMinutes.folder)
    const before = data.read(dir, run.files.notes)

    const cli = data.cli('notes', dir, '--wait')
    expect(cli.code).not.toBe(0)
    expect(cli.err).toContain(run.keptError)

    const log = data.read(dir, run.files.log)
    expect(log.split(run.logRefused).length - 1).toBe(g.refusals.length)
    expect(log.split(run.logNotAllowed).length - 1).toBe(g.refusals.length)
    for (const why of g.refusals) expect(log).toContain(why)
    expect(log).toContain(run.keptLog)
    expect(data.exists(data.home, run.outside)).toBe(false)
    expect(data.read(dir, run.files.notes)).toBe(before)
    expect(data.model.toolResults()).toEqual(g.toolResults)
  })
})
