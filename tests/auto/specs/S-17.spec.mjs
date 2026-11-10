// S-17 A recorded call end to end — its stages with fixed checks (small data, the fake Claude Code, the test clock), on
// this checkout's debug build of the Echo app run on the data (`echoctl app run --no-tray --background`, started in the
// prelude, its calls fed from files): the main call armed and recorded while the window takes notes on it (stage 1),
// the window filling in by itself (2), its events, transcript and files (3); what the recordings prepared once a build
// say about `echoctl capture` and `echoctl transcribe` (4–5, their `prepare` halves), a language pinned for one call
// (6), a call too short for automatic minutes (7), automatic minutes started at their time on the app (8) and moved
// by a note (9), and the call's files handed to macOS from Call actions (10, `open` only logged). The speech model and
// the prepared recordings come only from the test's own models (ECHOCTL_TEST_MODELS): without them, the stages that
// record, transcribe or read one skip.
// Not here:
// - stage 4 steps 1–3 and stage 5 step 3's first half (recording/TC-014, privacy/TC-003): they read the main call's
//   folder while it records, left for the next batch (the stages check the prepared capture);
// - stage 11 (recording/TC-019, the tray's icon): a person's;
// - stages 12–13 (claude/TC-019, window/TC-024): an app instance recording a call of its own, with live notes on;
//   left for the next batch;
// - stage 14 (claude/TC-020): an app instance and a signed-in Claude Code.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { useRecordings } from '../support/recordings.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { SettingsPage } from '../pages/settings.page.mjs'

const data = useData(test, { set: 'small', ai: 'fake', args: ['--clock', 'fast'] })
const rec = useRecordings(test, data)

const r = () => data.text.recording
const sp = () => data.text.speech
const cs = () => data.text.callStates
const set = () => data.set.s17
const a = () => data.text.app
const ev = () => data.text.app.events
/** The test's own speech models (ECHOCTL_TEST_MODELS), if set. */
const models = () => process.env[sp().testModels]
const noModels = () => `${sp().testModels} isn't set: the speech model comes only from the test's own folder, never the installed Echo's`
/** `call.py …`: the call fed from files. */
const call = (...args) => data.run('python3', r().call, ...args)
const inCall = () => JSON.parse(call('status'))
/** The running app's watcher (`echoctl status`'s), or nothing. */
const watcher = () => data.status().watcher || {}
/** The names of the events since the `from`th. */
const events = (from) => data.events(from).map((e) => e.event)
/** The lines of a text that hold any of these words. */
const linesWith = (text, words) => text.split('\n').filter((l) => words.some((w) => l.includes(w))).length
/** A wait that checks nothing comes early (half the automatic minutes' delay), not a wait for a condition. */
const seconds = (n) => new Promise((resolve) => setTimeout(resolve, n * 1000))
/** The test clock's delay of automatic minutes (seconds), as the data's environment sets it for every process. */
const delay = () => Number(data.env.ECHOCTL_TEST_AUTO_MINUTES_DELAY_S)
/** A file of a prepared recording (its capture's and transcription's output, its meta.json). */
const prepared = (recording, ...p) => data.read(data.run('python3', sp().recordings, 'path'), recording, ...p)
/** The prepared recording's own call folder (after `echoctl transcribe`). */
const preparedCall = (recording) => {
  const dir = path.join(data.run('python3', sp().recordings, 'path'), recording, r().callFolder)
  return path.join(dir, fs.readdirSync(dir)[0])
}
/** A copy of a prepared recording in the data's calls, under a title: its folder. */
const copy = (recording, title) => data.run('python3', sp().recordings, 'copy', recording, '--title', title).split('\n').at(-1)
/** The languages of a call's segments, each once. */
const languages = (call) => [...new Set(data.json(call, sp().segments).map((s) => s.lang))]
/** The lines of what macOS's `open` was asked to show (the data's stand-in only logs them). */
const opened = () => (data.exists(data.dir, r().openLog) ? data.read(data.dir, r().openLog).split('\n').filter(Boolean) : [])
/** A transcript's turns: who said them ("Remote", "Me") and the words under their line. */
const turnsOf = (text) => {
  const out = []
  for (const line of text.split('\n')) {
    const m = new RegExp(sp().turnLine).exec(line)
    if (m) out.push({ who: m[1], words: '' })
    else if (out.length) out.at(-1).words += line
  }
  return out
}
/** The `.wav` files under a folder (links not followed), as `find -name '*.wav'` lists them. */
const wavs = (dir) => fs.readdirSync(dir, { recursive: true, withFileTypes: true }).filter((e) => e.isFile() && e.name.endsWith(sp().audio))


// The scenario's app and the main call, shared by stages 1–3 and 8–9 as the manual scenario's prelude keeps them; the
// window's view of stages 1–2 is one page, never reloaded until stage 2 ends.
const main = {}

scenario(test, 'S-17', 'A recorded call end to end: record, take notes, transcript, automatic minutes, the files', () => {
  test.beforeAll(async ({ browser }) => {
    if (!models()) return
    test.setTimeout(2 * 60_000)
    const m = set().main
    // P1: Initech planning on its Transcript tab, in a page kept for stages 1–2.
    main.page = await browser.newPage({ viewport: test.info().project.use.viewport })
    main.view = new CallsPage(main.page, data)
    await main.view.open()
    await main.view.openCall(set().open.title, data.text.calls.transcriptTab)
    // Window/TC-013 step 1's checks, made here once.
    await expect(main.view.tab(data.text.calls.transcriptTab)).toHaveAttribute('aria-selected', 'true')
    await expect(main.view.turnGroups).toHaveCount(set().open.turns)
    // P2–P3: the speech model linked and in use, English; nothing asked, no automatic minutes, no live notes; the app.
    fs.symlinkSync(models(), path.join(data.env.ECHOCTL_CONFIG_DIR, sp().models))
    const model = data.cli('model', 'use', set().model)
    for (const args of [['config', '--languages', set().language, '--unscheduled', 'manual', '--scheduled', 'manual', '--auto-minutes', 'false'], ['ai', 'live', 'off']]) {
      const c = data.cli(...args)
      if (c.code !== 0) throw new Error(`${args.join(' ')}: ${c.err}`)
    }
    main.from = data.events().length
    main.app = await data.app()
    main.idle = data.status()
    main.model = model
    // P4: the arm; P5: the main call, held until stage 1 hangs it up.
    main.arm = data.cli('arm', '--title', m.title)
    for (let t0 = Date.now(); !watcher().armed && Date.now() - t0 < 10_000;) await seconds(0.2)
    main.armed = data.status()
    call('start', m.conversation, '--hold')
    for (let t0 = Date.now(); !(watcher().recording || '').endsWith(`-${m.slug}`) && Date.now() - t0 < 20_000;) await seconds(0.2)
    main.recording = data.status()
    main.dir = main.recording.watcher.recording
  })
  test.afterAll(async () => {
    if (main.page) await main.page.close()
  })

  test('1 · Notes typed while the call records carry the call\'s time; a note from the terminal joins them; one after the call has none', covers('window/TC-014', 'F-NOTES-10'), async () => {
    test.skip(!models(), noModels())
    test.setTimeout(3 * 60_000)
    const m = set().main
    const n = set().notes
    const view = main.view
    // The prelude's results (recording/TC-001 steps 1–4): the model, the idle app, the arm, the call recording.
    expect(main.model.code, main.model.err).toBe(0)
    expect(main.idle.watcher).toMatchObject({ recording: null, armed: false })
    expect(main.idle.armed).toBeNull()
    expect(main.arm.out.trim()).toBe(a().armed)
    expect(main.armed.watcher.armed).toBe(true)
    expect(main.armed.armed.title).toBe(m.title)
    expect(main.recording.watcher.in_call).toBe(true)
    expect(main.dir).toMatch(new RegExp(`-${literal(m.slug)}$`))
    expect(main.recording.armed).toBeNull()
    const file = path.join(main.dir, a().notesFile)
    const timed = (text) => new RegExp(fill(a().timedRow, { text: literal(text) }))
    let typedAt, terminal, hungUp
    try {
      // The row came by itself on the page loaded before the call (window/TC-013 step 3): open it from the list.
      await expect(view.call(m.title)).toHaveAccessibleName(new RegExp(`^${literal(data.text.callStates.icons.recording)} ${literal(m.title)}`))
      await view.openCall(m.title)
      await expect(view.tabNamed(a().recordingTab)).toHaveAttribute('aria-selected', 'true')
      await view.headerButton(data.text.calls.myNotes).click()
      await expect(view.newNote).toHaveAttribute('placeholder', new RegExp(a().notePlaceholder))
      await expect(view.panelButton(a().flag)).toBeVisible()
      await expect(view.panelSays(a().noteHint)).toBeVisible()

      await view.newNote.fill(n.typed)
      typedAt = inCall().for_s
      await view.newNote.press(data.text.shell.keys.enter)
      await expect.poll(() => view.noteTexts()).toContainEqual(expect.stringMatching(timed(n.typed)))

      await view.newNote.fill(n.draft)
      terminal = data.cli('note', n.terminal)
      await view.panelSays(a().noteHint).click()
      await expect.poll(() => view.noteTexts(), { timeout: n.within * 1000 }).toContainEqual(expect.stringMatching(timed(n.terminal)))
      await expect(view.newNote).toHaveValue(n.draft)

      // The hang-up once the conversation is over, then the recording closes.
      await expect.poll(() => inCall().for_s, { timeout: 60_000, intervals: [200] }).toBeGreaterThanOrEqual(m.hangUpAt)
      call('end')
      hungUp = Date.now()
      await expect.poll(() => events(main.from), { timeout: 60_000 }).toContain(ev().recordingStopped)
      main.hungUp = hungUp

      await view.newNote.press(data.text.shell.keys.escape)
      await view.newNote.fill(n.after)
      await view.newNote.press(data.text.shell.keys.enter)
      await expect.poll(() => view.noteTexts()).toContain(fill(a().untimedRow, { text: n.after }))
      await expect(view.panelSays(data.text.calls.notes.saved)).toBeVisible()
    } finally {
      if (!hungUp) call('end')
    }
    // The typed note's time is the call's (seconds into it), taken at its first key.
    const [, mm, ss] = timed(n.typed).exec((await view.noteTexts()).find((t) => timed(n.typed).test(t)))
    expect(Number(mm) * 60 + Number(ss)).toBeLessThanOrEqual(typedAt)
    expect(terminal.out.trim()).toBe(fill(a().noted, { path: file }))
    await expect.poll(() => data.read(file).trim().split('\n')).toEqual([
      expect.stringMatching(new RegExp(fill(a().timedNote, { text: literal(n.typed) }))),
      expect.stringMatching(new RegExp(fill(a().timedNote, { text: literal(n.terminal) }))),
      fill(a().untimedNote, { text: n.after }),
    ])
  })

  test('2 · The recorded call fills in by itself in the open window; what was open before stays as it was', covers('window/TC-013', 'F-CALLS-18'), async () => {
    test.skip(!models(), noModels())
    test.setTimeout(3 * 60_000)
    const m = set().main
    const view = main.view
    // Step 1 was the prelude's P1 (checked there).
    await expect.poll(() => events(main.from), { timeout: 90_000 }).toContain(ev().transcribed)
    await expect(view.call(m.title)).not.toHaveAccessibleName(new RegExp(`^${literal(data.text.callStates.icons.recording)} `), { timeout: 15_000 })
    for (const w of m.words) await expect(view.turnGroups.filter({ hasText: w })).not.toHaveCount(0)
    await view.call(set().open.title).click()
    await expect(view.tab(data.text.calls.transcriptTab)).toHaveAttribute('aria-selected', 'true')
    await expect(view.turnGroups).toHaveCount(set().open.turns)
  })

  test('3 · The armed call was recorded from connect to hang-up, closed as "call_ended" and transcribed, with no audio left', covers('recording/TC-001', 'F-REC-02', 'F-REC-08', 'F-REC-13'), async () => {
    test.skip(!models(), noModels())
    const m = set().main
    const seen = data.events(main.from).filter((e) => [ev().callStarted, ev().recordingStarted, ev().callEnded, ev().recordingStopped, ev().transcribed].includes(e.event))
    expect(seen.map((e) => e.event)).toEqual([ev().callStarted, ev().recordingStarted, ev().callEnded, ev().recordingStopped, ev().transcribed])
    // The stop came 10–15 s after the hang-up: the watcher's tick, the mic's 2 s and the 10 s of grace.
    const stopped = (Date.parse(seen[3].ts) - main.hungUp) / 1000
    expect(stopped).toBeGreaterThanOrEqual(m.stopsIn[0])
    expect(stopped).toBeLessThanOrEqual(m.stopsIn[1])
    const files = fs.readdirSync(main.dir)
    for (const f of [sp().transcript, sp().segments]) expect(files).toContain(f)
    expect(files.filter((f) => f.endsWith(sp().audio))).toEqual([])
    const meta = data.json(main.dir, sp().meta)
    expect(meta).toMatchObject({ stop_reason: r().stop.callEnded, title: m.title })
    expect(meta.ended_at).toBeTruthy()
    expect(meta.tracks.map((t) => t.file).sort()).toEqual([...r().tracks].sort())
    expect(linesWith(data.read(main.dir, sp().transcript), m.words)).toBeGreaterThanOrEqual(m.lines)
  })

  test("4 · `echoctl capture` closes both tracks after the seconds asked, and `echoctl transcribe` names its model (the prepared capture)", covers('recording/TC-014', 'F-REC-07'), async () => {
    test.skip(!!rec.none, rec.none)
    const c = set().capture
    expect(prepared(c.recording, r().captureExit).trim()).toBe(c.exit)
    const folder = prepared(c.recording, r().captureOut).trim().split('\n').at(-1)
    expect(folder).toMatch(new RegExp(fill(r().callPath, { slug: literal(c.recording) })))
    const err = prepared(c.recording, sp().captureErr)
    for (const file of r().tracks) {
      const lines = err.split('\n').map((l) => new RegExp(fill(r().track, { file: literal(file) })).exec(l)).filter(Boolean)
      expect(lines, file).toHaveLength(1)
      const [, seconds, restarts] = lines[0]
      expect(Number(seconds), file).toBeGreaterThanOrEqual(c.seconds[0])
      expect(Number(seconds), file).toBeLessThanOrEqual(c.seconds[1])
      expect(Number(restarts), file).toBe(0)
    }
    expect(JSON.parse(prepared(c.recording, sp().meta)).stop_reason).toBe(r().stop.duration)
    const out = prepared(c.recording, sp().transcribeOut).trim().split('\n')
    expect(out).toContain(fill(r().model, { model: set().model }))
    expect(out.at(-1)).toMatch(new RegExp(r().doneIn))
  })

  test('5 · Audio exists while a capture records; once transcribed only text is left, and no .wav under the data or the config', covers('privacy/TC-003', 'F-PRIV-03'), async () => {
    test.skip(!!rec.none, rec.none)
    const c = set().capture
    const during = prepared(c.recording, r().during).split('\n')
    for (const file of r().tracks) expect(during).toContain(file)
    const after = prepared(c.recording, r().after).split('\n').filter(Boolean)
    for (const file of sp().files) expect(after).toContain(file)
    expect(after.filter((f) => f.endsWith(sp().audio))).toEqual([])
    expect(data.json(preparedCall(c.recording), sp().meta).audio).toBe(r().audioDeleted)
    // Step 3's `find`: this run's data and config hold no audio (the calls copied here were transcribed).
    expect(wavs(data.home)).toEqual([])
    expect(wavs(data.env.ECHOCTL_CONFIG_DIR)).toEqual([])
  })

  test('6 · A language pinned for one call applies to that call only: the workspace and the next call keep theirs', covers('transcription/TC-004', 'F-STT-02'), async () => {
    test.skip(!!rec.none, rec.none)
    const { pinned, next, workspace } = set()
    const meta = JSON.parse(prepared(pinned.recording, sp().meta))
    expect(meta.language).toBe(pinned.language)
    expect(meta.languages).toEqual(pinned.languages)
    expect(languages(copy(pinned.recording, pinned.title))).toEqual([pinned.language])
    expect(JSON.parse(prepared(pinned.recording, r().configAfter)).languages).toEqual(workspace)
    // The next call, with no pin of its own: the default language, said as such (no "(from … utterances)").
    const lines = prepared(next.recording, sp().transcribeOut).split('\n').filter((l) => new RegExp(sp().languageLine).test(l))
    expect(lines).toEqual([fill(sp().language, { lang: next.language })])
    expect(languages(copy(next.recording, next.title))).toEqual([next.language])
  })

  test('7 · A very short call gets a transcript, and its Minutes say none are written automatically, with Write minutes', covers('recording/TC-013', 'F-MIN-08'), async ({ page }) => {
    test.skip(!!rec.none, rec.none)
    const s = set().short
    const on = data.cli('config', '--auto-minutes', 'true')
    let off
    try {
      expect(on.code, on.err).toBe(0)
      expect(on.json().auto_minutes).toBe(true)
      const call = copy(s.recording, s.title)
      const files = fs.readdirSync(call)
      for (const f of s.files) expect(files).toContain(f)
      expect(files.filter((f) => f.endsWith(sp().audio))).toEqual([])
      // The transcript: the remote side's question, then Me's answer.
      const turns = turnsOf(data.read(call, sp().transcript))
      expect(turns.map((t) => t.who)).toEqual([sp().speakers.remote, sp().speakers.me])
      expect(turns[0].words).toContain(s.says)

      const view = new CallsPage(page, data)
      await view.open()
      await view.openCall(s.title)
      await expect(view.fact(fill(cs().duration, { n: s.minutes }))).toBeVisible()
      await view.tab(cs().tabs.minutes).click()
      await expect(view.paneHeadings).toHaveText([cs().notAuto])
      await expect(view.says(fill(cs().thin, { why: s.why }))).toBeVisible()
      await expect(view.headerButton(cs().writeMinutes)).toBeVisible()
    } finally {
      off = data.cli('config', '--auto-minutes', 'false')
    }
    expect(off.code, off.err).toBe(0)
    expect(off.json().auto_minutes).toBe(false)
  })

  test('8 · Automatic minutes start by themselves at the time Echo named (the call\'s end and the delay), never before', covers('claude/TC-003', 'F-MIN-14', 'F-MIN-07'), async ({ page }) => {
    test.skip(!models(), noModels())
    test.skip(!!rec.none, rec.none)
    test.setTimeout(5 * 60_000)
    const s = set().autoMinutes
    const cs = data.text.callStates
    const run = data.text.calls.run
    expect(delay()).toBe(s.delay)
    expect(watcher().pid).toBe(main.app.pid)
    const on = data.cli('config', '--auto-minutes', 'true')
    expect(on.code, on.err).toBe(0)
    expect(on.json().auto_minutes).toBe(true)
    const dir = copy(s.recording, s.title)
    const ended = Date.parse(data.json(dir, sp().meta).ended_at)
    const due = ended + s.delay * 1000

    const view = new CallsPage(page, data)
    await view.open()
    await view.openCall(s.title, cs.tabs.minutes)
    const heading = view.paneHeadingLike(new RegExp(cs.minutesInPattern))
    await expect(heading).toBeVisible()
    const [, n, time] = new RegExp(cs.minutesInPattern).exec(await heading.textContent())
    expect([1, 2]).toContain(Number(n))
    expect(time).toBe(await view.clock(new Date(due).toISOString()))
    await expect(view.says(cs.autoLine)).toBeVisible()
    await expect(view.saysStarting(data.text.calls.writesWith)).toBeVisible()
    await expect(view.headerButton(cs.writeNow)).toBeVisible()
    await expect(view.headerButton(cs.notYet)).toBeVisible()
    // Half the delay on: nothing has started early.
    await seconds((ended + s.early * 1000 - Date.now()) / 1000)
    expect(data.exists(dir, run.runFile)).toBe(false)
    await expect.poll(() => data.exists(dir, run.runFile), { timeout: 120_000 }).toBe(true)
    const started = data.json(dir, run.runFile)
    expect(started.auto).toBe(true)
    expect(Date.parse(started.started_at)).toBeGreaterThanOrEqual(due)
    expect(Date.parse(started.started_at)).toBeLessThanOrEqual(due + s.slack * 1000)
    await expect.poll(() => a().runEnded.includes(data.json(dir, run.runFile).state), { timeout: 60_000 }).toBe(true)
    expect(data.read(dir, run.log).split('\n')[0]).toMatch(new RegExp(fill(a().logLine, { text: literal(a().autoRun) })))
    await expect(view.headerButton(cs.tryAgain)).toBeVisible({ timeout: 15_000 })
  })

  test('9 · A note added after the call moves the automatic minutes to the delay after it', covers('claude/TC-010', 'F-MIN-14', 'F-NOTES-06'), async ({ page }) => {
    test.skip(!models(), noModels())
    test.skip(!!rec.none, rec.none)
    test.setTimeout(6 * 60_000)
    const s = set().notesMove
    const cs = data.text.callStates
    const run = data.text.calls.run
    const view = new CallsPage(page, data)
    let dir, ended, firstDue, noted, moved, started, off, quit
    try {
      dir = copy(s.recording, s.title)
      ended = Date.parse(data.json(dir, sp().meta).ended_at)
      await view.open()
      await view.openCall(s.title, cs.tabs.minutes)
      const heading = view.paneHeadingLike(new RegExp(cs.minutesInPattern))
      await expect(heading).toBeVisible()
      firstDue = new RegExp(cs.minutesInPattern).exec(await heading.textContent())[2]
      // 35 s after the call's end: a note within 30 s of it doesn't count as after the call.
      await seconds((ended + s.noteAfter * 1000 - Date.now()) / 1000)
      await view.headerButton(data.text.calls.myNotes).click()
      await view.newNote.fill(s.note)
      await view.newNote.press(data.text.shell.keys.enter)
      await heading.click()
      await expect.poll(() => view.noteTexts()).toContain(fill(a().untimedRow, { text: s.note }))
      await expect(view.panelSays(data.text.calls.notes.saved)).toBeVisible()
      noted = fs.statSync(path.join(dir, a().notesFile)).mtimeMs
      const at = await view.clock(new Date(noted).toISOString())
      const due = await view.clock(new Date(noted + delay() * 1000).toISOString())
      await expect(async () => {
        await page.reload()
        await expect(view.paneHeadingLike(new RegExp(cs.minutesInPattern))).toHaveText(new RegExp(`\\(${literal(due)}\\)$`), { timeout: 2_000 })
        await expect(view.says(fill(cs.movedNotes, { time: at }))).toBeVisible({ timeout: 2_000 })
      }).toPass({ timeout: 20_000 })
      moved = true
      await expect.poll(() => data.exists(dir, run.runFile), { timeout: 240_000 }).toBe(true)
      started = data.json(dir, run.runFile)
    } finally {
      off = data.cli('config', '--auto-minutes', 'false')
      // The last stage on the scenario's app.
      quit = await main.app.quit()
    }
    expect(off.code, off.err).toBe(0)
    expect(quit.code, quit.err).toBe(0)
    expect(data.status().watcher).toBeNull()
    expect(firstDue).toBe(await view.clock(new Date(ended + delay() * 1000).toISOString()))
    expect(noted).toBeGreaterThanOrEqual(ended + s.noteAfter * 1000)
    expect(noted).toBeLessThan(ended + delay() * 1000)
    expect(moved).toBe(true)
    expect(data.read(dir, a().notesFile)).toContain(s.note)
    expect(started.auto).toBe(true)
    expect(Date.parse(started.started_at)).toBeGreaterThanOrEqual(noted + delay() * 1000)
    expect(Date.parse(started.started_at)).toBeLessThanOrEqual(noted + (delay() + set().autoMinutes.slack) * 1000)
  })

  test("10 · Call actions' Open minutes in browser, Show minutes in Finder and Show folder hand macOS the call's own files", covers('window/TC-012', 'F-CALLS-17'), async ({ page }) => {
    const b = set().browser
    const call = data.callDir(b.folder)
    const minutesPage = path.join(call, fill(r().minutesPage, { title: b.title }))
    expect(fs.existsSync(minutesPage)).toBe(false)
    const view = new CallsPage(page, data)
    await view.open()
    await view.openCall(b.title)
    await view.callActions.click()
    await expect(view.menuItems).toHaveText(data.set.housekeeping.acme.menu)
    const before = opened().length
    await view.menuItem(r().menu.inBrowser).click()
    await expect.poll(() => opened().slice(before)).toEqual([minutesPage])
    expect(fs.existsSync(minutesPage)).toBe(true)
    await view.callAction(r().menu.inFinder)
    await expect.poll(() => opened().slice(before)).toEqual([minutesPage, fill(r().reveal, { path: minutesPage })])
    await view.callAction(r().menu.showFolder)
    await expect.poll(() => opened().slice(before)).toEqual([minutesPage, fill(r().reveal, { path: minutesPage }), call])
  })

  test('12 · While a call records, live notes don\'t run on a profile whose model is on this computer, and Settings says why', covers('claude/TC-019', 'F-LIVE-07'), async ({ page }) => {
    test.skip(!models(), noModels())
    test.setTimeout(4 * 60_000)
    const s = set().localLive
    const before = data.cli('ai', 'status').json()
    const from = data.events().length
    let added, made, live, app, early, late, notesFiles, quit
    try {
      added = data.cli('ai', 'add-local', s.profile, '--url', s.url, '--model', s.model)
      made = data.cli('ai', 'default', s.profile)
      live = data.cli('ai', 'live', 'on')
      // Its own app (the scenario's quit in stage 9).
      app = await data.app()
      data.cli('arm', '--title', s.title)
      await expect.poll(() => watcher().armed, { timeout: 10_000 }).toBe(true)
      call('start', s.conversation, '--hold')
      await expect.poll(() => watcher().recording || '', { timeout: 20_000 }).toMatch(new RegExp(`-${literal(s.slug)}$`))
      const dir = watcher().recording
      await expect.poll(() => inCall().for_s, { timeout: 30_000, intervals: [200] }).toBeGreaterThanOrEqual(s.early)
      early = data.cli('live', 'show', dir).json()
      // Past the first summary's time (90 s): still none.
      await expect.poll(() => inCall().for_s, { timeout: (s.past + 30) * 1000, intervals: [500] }).toBeGreaterThanOrEqual(s.past)
      late = data.cli('live', 'show', dir).json()
      notesFiles = fs.readdirSync(dir).filter((f) => f === a().liveNotesFile)
      const settings = new SettingsPage(page, data)
      await settings.open(a().aiRoute)
      await expect(settings.says(a().liveRow)).toBeVisible()
      await expect(settings.saysPart(fill(a().liveNotWith, { name: s.name }))).toBeVisible()
    } finally {
      call('end')
      if (app) {
        await expect.poll(() => watcher().recording || null, { timeout: 60_000 }).toBeNull()
        quit = await app.quit()
      }
      data.cli('ai', 'live', 'off')
      data.cli('ai', 'default', before.default)
      data.cli('ai', 'remove', s.profile)
    }
    expect(quit.code, quit.err).toBe(0)
    expect(added.out.trim()).toBe(fill(a().added, { name: s.name, id: s.profile }))
    expect(made.out.trim()).toBe(fill(a().defaultSet, { name: s.name }))
    expect(live.out.trim()).toBe(a().liveOn)
    for (const show of [early, late]) {
      expect(show.state.state).toBe(a().liveLocal)
      expect(show.notes).toBeNull()
    }
    expect(notesFiles).toEqual([])
    expect(data.events(from).filter((e) => e.event === ev().recordingStarted)).toHaveLength(1)
  })
})
