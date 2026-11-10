// S-32 Model runs on the virtual model — the real Claude Code, its tools and Echo's guard; only the model is scripted.
import { test, expect } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
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

  test("3 · A diagram Mermaid can't parse is repaired on the call's profile with only it and its error; one the model refuses keeps its source", covers('claude/TC-021', 'F-MIN-22'), async ({ page }) => {
    const { calls: c, diagrams: dg } = data.set
    const t = data.text.diagrams
    const calls = new CallsPage(page, data)
    data.model.use(dg.script)
    data.model.clear()
    const dir = data.callDir(c.withDiagram.folder)
    const notes = () => data.read(dir, data.text.minutesRun.files.notes)
    const before = notes()
    fs.appendFileSync(path.join(dir, data.text.minutesRun.files.notes), dg.broken)
    await calls.open()
    await calls.openCall(c.withDiagram.title, data.text.calls.minutesTab)
    await expect.poll(notes, { timeout: 15_000 }).toContain(dg.repaired)
    expect(notes().startsWith(before)).toBe(true) // the rest of the minutes as it was
    expect(data.read(data.dir, t.serveLog)).toContain(t.repaired)
    // Only the diagram and Mermaid's error went to the model.
    const [ask] = data.model.requests()
    expect(textOf(ask.system)).toContain(t.system)
    const said = textOf(ask.messages[0].content)
    for (const words of t.userHas) expect(said).toContain(words)
    expect(said).not.toContain(before.trim().split('\n')[0])
    await expect(calls.minutes.locator('svg').first()).toBeVisible()
    await expect(calls.minutes.getByText(t.bomb)).toHaveCount(0)

    // A second one the model answers with an error: left as it is, shown as its source with the reason.
    fs.appendFileSync(path.join(dir, data.text.minutesRun.files.notes), dg.refused)
    await calls.openCall(c.withDiagram.title, data.text.calls.transcriptTab)
    await calls.tab(data.text.calls.minutesTab).click()
    await expect.poll(() => data.read(data.dir, t.serveLog), { timeout: 15_000 }).toContain(t.refused)
    expect(notes()).toContain(dg.refusedBlock)
    await calls.openMinutesPage(data.callName(c.withDiagram.folder))
    await expect(page.getByText(t.cantDraw)).toBeVisible()
    await expect(page.getByText(t.bomb)).toHaveCount(0)
  })
})
