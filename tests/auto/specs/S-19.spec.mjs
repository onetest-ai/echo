// S-19 Claude Code and the minutes — stages 1–5 on the virtual model; 6–8 on this checkout's debug build of the Echo app
// run on the data (`echoctl app run --no-tray --background`, started in stage 6, quit in stage 8 and started again
// there): automatic minutes off (6), `echoctl doctor` and `up` answered by the running app, as Health shows it (7), a
// call missed while Echo was quit caught up at its start (8). Stages 9–13 are a person's (/echo:doctor in a session,
// the live pane, a signed-out Claude Code, recording from a chat, nothing sent without a yes).
import { test, expect } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { useData, textOf, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { useRecordings } from '../support/recordings.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { SettingsPage } from '../pages/settings.page.mjs'

const data = useData(test, { set: 'small', ai: 'virtual', args: ['--clock', 'fast'] })
// The calls are copies of the recordings prepared once a build (the regression's one speech-to-text check), prepared
// when this build has none yet — only with the test's own models (ECHOCTL_TEST_MODELS); without them every stage skips.
const rec = useRecordings(test, data)
test.beforeEach(() => test.skip(!!rec.none, rec.none))
const state = {} // the two calls' stamps, shared by the stages (as the manual scenario's glue keeps them)
const a = () => data.text.app
const sp = () => data.text.speech
const cs = () => data.text.callStates
const run = () => data.text.calls.run
/** The test's own speech models (ECHOCTL_TEST_MODELS), if set. */
const models = () => process.env[sp().testModels]
/** A copy of a prepared recording in the data's calls, under a title: its folder. */
const copy = (recording, title) => data.run('python3', sp().recordings, 'copy', recording, '--title', title).split('\n').at(-1)
/** A wait that checks nothing happens (past the time automatic minutes would come), not a wait for a condition. */
const until = (t) => new Promise((resolve) => setTimeout(resolve, Math.max(0, t - Date.now())))
let app // stages 6–7's app (started in 6), then stage 8's

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

  test('6 · With automatic minutes off nothing is written until asked, and switching them on doesn\'t backfill', covers('claude/TC-015', 'F-MIN-14'), async ({ page }) => {
    test.setTimeout(5 * 60_000)
    const s = data.set.s19.autoOff
    const calls = new CallsPage(page, data)
    const off = data.cli('config', '--auto-minutes', 'false')
    expect(off.code, off.err).toBe(0)
    expect(off.json().auto_minutes).toBe(false)
    const dir = copy(s.recording, s.title)
    const copied = Date.now()
    const folder = path.basename(dir)
    // The app of stages 6–7: nothing asked, no live notes; the speech model in use for stage 7's `up`.
    for (const args of [['config', '--unscheduled', 'manual', '--scheduled', 'manual'], ['ai', 'live', 'off']]) expect(data.cli(...args).code).toBe(0)
    if (models()) {
      fs.symlinkSync(models(), path.join(data.env.ECHOCTL_CONFIG_DIR, sp().models))
      expect(data.cli('model', 'use', data.set.s19.model).code).toBe(0)
    }
    app = await data.app()
    await calls.open()
    await calls.openCall(s.title, cs().tabs.minutes)
    await expect(calls.paneHeading(cs().noMinutes)).toBeVisible()
    await expect(calls.says(cs().ready)).toBeVisible()
    await expect(calls.headerButton(cs().writeMinutes)).toBeVisible()
    await expect(calls.paneHeadingLike(new RegExp(cs().minutesInPattern))).toHaveCount(0)
    expect((await calls.callApi(folder)).auto).toEqual({ due: a().due.off })
    await until(copied + s.quiet * 1000)
    expect(data.exists(dir, run().notes)).toBe(false)
    expect(data.exists(dir, run().runFile)).toBe(false)
    const on = data.cli('config', '--auto-minutes', 'true')
    expect(on.code, on.err).toBe(0)
    expect(on.json().auto_minutes).toBe(true)
    expect(on.json().auto_minutes_since).toBeTruthy()
    expect((await calls.callApi(folder)).auto).toEqual({ due: a().due.before })
    await calls.headerButton(cs().writeMinutes).click()
    await expect.poll(() => data.exists(dir, run().runFile) && data.json(dir, run().runFile).state, { timeout: 90_000 }).toBe(a().runDone)
    expect(data.exists(dir, run().notes)).toBe(true)
    expect(data.exists(dir, a().actionsFile)).toBe(true)
    expect(data.json(dir, run().runFile).auto).toBeFalsy()
  })

  test('7 · `echoctl doctor` and `up` are answered by the running app, as Health shows it', covers('cli/TC-005', 'F-CLI-11', 'F-SET-17'), async ({ page }) => {
    test.skip(!models(), `${sp().testModels} isn't set: \`up\` names the speech model in use, from the test's own folder only`)
    const doctor = data.cli('doctor', '--json')
    expect(doctor.code, doctor.err).toBe(0)
    const d = doctor.json()
    expect(d.checked_by).toBe(a().checkedBy)
    expect(d.checks.map((c) => c.id)).toEqual(data.set.s19.checks)
    for (const c of d.checks) {
      expect(a().statuses).toContain(c.status)
      expect(c.how, c.id).toBeTruthy()
      expect(c.settings_url, c.id).toBeTruthy()
    }
    expect(d.ready).toBe(d.checks.filter((c) => c.required).every((c) => c.status === a().statuses[0]))
    const settings = new SettingsPage(page, data)
    await settings.open(a().health)
    for (const c of d.checks) await expect(settings.rowSays(settings.settingRow(c.name), a().marks[c.status])).toBeVisible()
    const up = data.cli('up')
    expect(up.code, up.err).toBe(0)
    expect(up.json()).toMatchObject({ model: data.set.s19.model, app_pid: app.pid, ready: d.ready })
  })

  test('8 · A call missed while Echo was quit gets its minutes at the next start, written by Echo from the call alone', covers('claude/TC-007', 'F-MIN-14', 'F-MIN-18'), async ({ page }) => {
    test.setTimeout(8 * 60_000)
    const s = data.set.s19.missed
    const t = data.text.minutes
    const calls = new CallsPage(page, data)
    const quit = await app.quit()
    expect(quit.code, quit.err).toBe(0)
    expect(data.status().watcher).toBeNull()
    const dir = copy(s.recording, s.title)
    const copied = Date.now()
    const stamp = path.basename(dir).slice(0, 17)
    const ended = Date.parse(data.json(dir, sp().meta).ended_at)
    await calls.open()
    const auto = (await calls.callApi(path.basename(dir))).auto
    expect(auto.due).toBe(a().due.at)
    expect(Date.parse(auto.at)).toBe(ended + Number(data.env.ECHOCTL_TEST_AUTO_MINUTES_DELAY_S) * 1000)
    await until(copied + s.quiet * 1000)
    expect(data.exists(dir, run().runFile)).toBe(false)
    const from = data.model.requests().length
    let started, pending, done, last, q
    try {
      app = await data.app()
      started = Date.now()
      await expect.poll(() => data.exists(data.callByStamp(stamp), run().runFile), { timeout: 60_000 }).toBe(true)
      pending = data.cli('pending', '--for', data.home)
      await expect.poll(() => data.json(data.callByStamp(stamp), run().runFile).state, { timeout: 300_000 }).toBe(a().runDone)
      done = data.callByStamp(stamp)
      last = data.json(done, run().runFile)
    } finally {
      q = await app.quit()
    }
    expect(q.code, q.err).toBe(0)
    expect(last.auto).toBe(true)
    expect(Date.parse(last.started_at) - started).toBeLessThanOrEqual(s.startsWithin * 1000)
    expect(pending.code, pending.err).toBe(0)
    expect(pending.json()).toEqual([])
    const files = fs.readdirSync(done)
    for (const f of [run().notes, a().actionsFile]) expect(files).toContain(f)
    expect(data.read(done, run().notes)).toContain(data.set.virtualModel.minutes)
    expect(data.json(done, a().actionsFile).map((i) => [i.owner, i.task])).toEqual([[data.set.virtualModel.action.owner, data.set.virtualModel.action.task]])
    expect(JSON.stringify(data.model.requests().slice(from)).toLowerCase()).toContain(s.said)
    const log = data.read(done, run().log).trim().split('\n')
    expect(log[0]).toMatch(new RegExp(fill(a().logLine, { text: literal(a().autoRun) })))
    expect(log.find((l) => l.trim().startsWith(t.modelLine))).toBeTruthy()
    expect(log.find((l) => l.trim().startsWith(t.folderLine))).toContain(data.home)
    expect(log.join('\n')).toContain(t.logGuard)
    const ran = log.map((l) => new RegExp(a().runCommand).exec(l)).filter(Boolean).map((m) => m[1])
    expect(ran.length).toBeGreaterThan(0)
    for (const c of ran) expect(a().runCommands).toContain(c)
    for (const never of a().neverRun) expect(log.join('\n')).not.toContain(never)
    expect(log.at(-1)).toMatch(new RegExp(a().writtenLine))
  })
})
