// S-19 Claude Code and the minutes — stages 1–5 on the virtual model (stages 6–8 need the Echo app; 9–13 a person).
import { test, expect } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { useData, textOf } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { useRecordings } from '../support/recordings.mjs'
import { CallsPage } from '../pages/calls.page.mjs'

const data = useData(test, { set: 'small', ai: 'virtual', args: ['--clock', 'fast'] })
// The calls are copies of the recordings prepared once a build (the regression's one speech-to-text check), prepared
// when this build has none yet — only with the test's own models (ECHOCTL_TEST_MODELS); without them every stage skips.
const rec = useRecordings(test, data)
test.beforeEach(() => test.skip(!!rec.none, rec.none))
const state = {} // the two calls' stamps, shared by the stages (as the manual scenario's glue keeps them)

scenario(test, 'S-19', 'Claude Code and the minutes', () => {
  test('1 · Write now writes the minutes on the call\'s profile, and Write minutes writes them for a call too short for automatic ones', covers('claude/TC-004', 'F-MIN-13', 'F-MIN-08'), async ({ page }) => {
    const { s19: s } = data.set
    const t = data.text.minutes
    const calls = new CallsPage(page, data)
    data.cli('config', '--auto-minutes', 'true', '--unscheduled', 'manual', '--scheduled', 'manual')
    const recordings = path.join('tests/fixtures/recordings.py')
    const long = data.run('python3', recordings, 'copy', s.long.recording, '--title', s.long.title).split('\n').at(-1)
    const short = data.run('python3', recordings, 'copy', s.short.recording, '--title', s.short.title).split('\n').at(-1)
    state.long = path.basename(long).slice(0, 17)
    state.short = path.basename(short).slice(0, 17)
    // The glue of stages 2–4: the user's notes in the long call; Echo's runs in folder A for run A.
    fs.writeFileSync(path.join(long, 'my-notes.md'), s.notes)
    const folderA = path.join(data.dir, s.folderA)
    fs.mkdirSync(path.join(folderA, '.claude'), { recursive: true })
    fs.writeFileSync(path.join(folderA, '.claude/settings.json'), JSON.stringify({ env: { ECHO_TEST_FOLDER: 'A' } }))
    data.cli('config', '--notes-in', folderA)
    data.model.delay(s.delay) // run A can be caught while it writes (stage 3)
    data.model.clear()

    await calls.open()
    await calls.openCall(s.long.title, data.text.calls.minutesTab)
    await expect(page.getByText(t.due)).toBeVisible()
    await calls.writeNow.click()
    await expect(page.getByText(t.writing)).toBeVisible()
    await expect(page.getByText(t.profile, { exact: true })).toBeVisible()
    // Stage 3's step 1, while run A writes: a session (in folder A) can't take the call.
    const claim = data.cli('claim', long, '--for', folderA, '--session', 'test-session-1')
    state.claimA = claim
    await expect.poll(() => data.json(data.callByStamp(state.long), 'notes-run.json').state, { timeout: 60_000 }).toBe('done')
    const logA = data.read(data.callByStamp(state.long), 'claude.log')
    state.logA = logA
    expect(logA).toContain(t.logStarts)
    expect(logA).toContain(t.logGuard)
    expect(logA).toContain(t.logOk)
    expect(logA).toContain(t.logWritten)

    // Run B: the short call, too thin for automatic minutes, written anyway; Echo's runs back in the data folder.
    data.cli('config', '--notes-in', 'any')
    data.model.delay(0)
    await calls.openCall(s.short.title, data.text.calls.minutesTab)
    await expect(page.getByText(t.tooThin)).toBeVisible()
    await expect(page.getByText(t.tooThinWhy)).toBeVisible()
    // Stage 3's step 2: a session claims the short call first; the click below takes it over.
    state.claimB = data.cli('claim', short, '--for', data.home, '--session', 'test-session-2')
    state.claimedB = data.exists(short, '.claimed')
    await calls.writeMinutes.click()
    await expect.poll(() => data.exists(data.callByStamp(state.short), 'notes.md'), { timeout: 60_000 }).toBe(true)
    await expect.poll(() => data.json(data.callByStamp(state.short), 'notes-run.json').state, { timeout: 60_000 }).toBe('done')
    state.logB = data.read(data.callByStamp(state.short), 'claude.log')
  })

  test("2 · The user's notes, from during and after the call, reach the run that writes its minutes", covers('claude/TC-009', 'F-MIN-13'), async ({ page }) => {
    const { s19: s } = data.set
    const calls = new CallsPage(page, data)
    await calls.open()
    await calls.openCall(s.long.title)
    await page.getByRole('button', { name: 'My notes' }).click()
    for (const words of s.notesSay) await expect(page.getByText(words, { exact: false }).first()).toBeVisible()
    // The context Echo gave the model (`echoctl context`, the run's first tool result) names this call's notes file,
    // which holds all three notes: process-call reads it and writes the minutes around them.
    const first = data.model.requests().filter((e) => e.rule === data.set.virtualModel.minutesRule)
      .flatMap((e) => (Array.isArray(e.messages.at(-1).content) ? e.messages.at(-1).content : []).filter((b) => b.type === 'tool_result'))[0]
    const notesFile = JSON.parse(textOf(first.content)).user_notes
    expect(notesFile).toBe(path.join(data.callByStamp(state.long), data.text.minutes.userNotes))
    for (const words of s.notesSay) expect(data.read(notesFile)).toContain(words)
  })

  test('3 · A session can\'t take a call while Echo writes it; Write minutes takes a call back from a session', covers('claude/TC-005', 'F-MIN-13'), async () => {
    const t = data.text.minutes
    expect(state.claimA.code).toBe(1)
    expect(state.claimA.err).toContain(t.claimRefused)
    expect(state.claimB.code).toBe(0)
    expect(state.claimedB).toBe(true)
    expect(data.read(data.dir, data.text.diagrams.serveLog)).toContain(t.claimTaken)
  })

  test('4 · Minutes runs work in the chosen folder, else in the data folder', covers('claude/TC-006', 'F-MIN-13'), async () => {
    const t = data.text.minutes
    const folderLine = (log) => log.split('\n').find((l) => l.trim().startsWith(t.folderLine))
    expect(folderLine(state.logA)).toContain(path.join(data.dir, data.set.s19.folderA))
    expect(folderLine(state.logB)).toContain(data.home)
  })

  test("5 · Each AI profile's minutes model is used, and a wrong one is explained", covers('claude/TC-012', 'F-AI-08'), async ({ page }) => {
    const { s19: s } = data.set
    const t = data.text.minutes
    const calls = new CallsPage(page, data)
    const modelLine = state.logB.split('\n').find((l) => l.trim().startsWith(t.modelLine))
    expect(modelLine).toContain(t.profile)
    await calls.open()
    await calls.openCall(s.short.title, data.text.calls.minutesTab)
    await expect(calls.minutes.getByText(t.notice, { exact: false })).toBeVisible()
    expect(data.cli('ai', 'set', 'ollama', '--notes-model', s.wrongModel).code).toBe(0)
    await calls.callAction(t.writeAgain)
    await expect(page.getByText(t.cantUseModel, { exact: false })).toBeVisible({ timeout: 30_000 })
    await expect(page.getByText(s.wrongModel, { exact: false }).first()).toBeVisible()
    const back = data.cli('ai', 'set', 'ollama', '--notes-model', '')
    expect(back.out + back.err).toContain(t.minutesModelBack)
  })
})
