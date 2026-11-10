// S-18 Recording off the happy path — the stages with fixed checks (small data, the fake Claude Code), on this
// checkout's debug build of the Echo app run on the data (`echoctl app run --no-tray --background`, started in the
// prelude, its calls fed from files: `tests/fixtures/call.py`, no device, no sound): an arm cancelled (stage 1) and
// one expired (2), a call nobody opted in to (3), a call armed in the middle (6), arm and stop from the terminal (7)
// and what that stop leaves of the call (8), the pill's Stop in the window (9), a drop and a rejoin (11), each on a
// call of its own; a capture stopped by Ctrl-C (4) and one killed (5) beside the app; silence (12) and two remote
// voices (13) in the recordings prepared once a build; a Stop while the app is frozen (25, an app of its own). Speech
// models only from the test's own folder (ECHOCTL_TEST_MODELS): without it, the stages that transcribe or read a
// prepared recording skip.
// Not here:
// - stage 10 (recording/TC-012, ask mode): Echo's own "Record?" dialog opens on the Mac's screen and is answered
//   through System Events;
// - stages 14–24 (privacy/TC-002, recording/TC-023, TC-007, TC-008, permissions/TC-006, recording/TC-009, TC-010,
//   transcription/TC-006, recording/TC-020, TC-021, TC-022): a person's (real devices, Teams, the Mac's sleep, System
//   Settings);
// - stage 26 (transcription/TC-010, the echo on speakers): a 127 s call and its score, left for the next batch;
// - stage 27 (transcription/TC-012, a call's language changed during it): it needs Parakeet v3 and the NeMo-Speech
//   runtime, which the test's models folders don't hold;
// - stage 13 steps 2–3 (recording/TC-018, `echoctl speakers` and two speaker labels): the test's models folder has no
//   speaker model, so the prepared recording has no detected speakers (and F-STT-04, two voices told apart, isn't
//   claimed: step 1 checks only that both stay Remote).
// Each stage runs its case "on its own" on the prelude's app: a call started for it and hung up after it (stage 8 goes
// on with stage 7's), not the scenario's main call — so the captures of stages 4–5 start no call on top of another.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { useRecordings, wavInfo } from '../support/recordings.mjs'
import { AppPage } from '../pages/app.page.mjs'

const data = useData(test, { set: 'small', ai: 'fake' })
const rec = useRecordings(test, data)

const r = () => data.text.recording
const sp = () => data.text.speech
const set = () => data.set.s18
const a = () => data.text.app
const ev = () => data.text.app.events
/** The test's own speech models (ECHOCTL_TEST_MODELS), if set. */
const models = () => process.env[sp().testModels]
/** `call.py …`: the call fed from files. */
const call = (...args) => data.run('python3', r().call, ...args)
const inCall = () => JSON.parse(call('status'))
/** The call folder whose name ends in a slug. */
const folderOf = (slug) => data.callDir(slug)
/** A file of a prepared recording (its capture's output, its meta.json). */
const prepared = (recording, ...p) => data.read(data.run('python3', sp().recordings, 'path'), recording, ...p)
/** A copy of a prepared recording in the data's calls, under a title: its folder. */
const copy = (recording, title) => data.run('python3', sp().recordings, 'copy', recording, '--title', title).split('\n').at(-1)
/** A track's line of what a capture printed at its end: its seconds and restarts. */
const trackLine = (err, file) => err.split('\n').map((l) => new RegExp(fill(r().track, { file: literal(file) })).exec(l)).filter(Boolean)

/** The running app's watcher (`echoctl status`'s), or nothing. */
const watcher = () => data.status().watcher || {}
/** Until the call has gone on this long (seconds, on its own clock: the conversation's). */
const callAt = (seconds) => expect.poll(() => inCall().for_s, { timeout: (seconds + 30) * 1000, intervals: [200] }).toBeGreaterThanOrEqual(seconds)
/** The call folders added since a listing. */
const calls = () => fs.readdirSync(path.join(data.home, 'calls'))
const added = (before) => calls().filter((n) => !before.includes(n))
/** The names of the events since the `from`th. */
const events = (from) => data.events(from).map((e) => e.event)
/** The lines of a text that hold any of these words. */
const linesWith = (text, words) => text.split('\n').filter((l) => words.some((w) => l.includes(w))).length
/** A call's transcript once the app has written it (`wait` ms). */
const transcriptOf = async (dir, wait = 120_000) => {
  await expect.poll(() => fs.existsSync(path.join(dir, sp().transcript)), { timeout: wait }).toBe(true)
  return data.read(dir, sp().transcript)
}
/** A wait the conversation's timeline asks for (a drop's few seconds), not a wait for a condition. */
const seconds = (n) => new Promise((resolve) => setTimeout(resolve, n * 1000))
/** No call left on, whatever the stage left (`call.py end` says "no call" when there is none). */
const hangUp = () => call('end')

/** A call of its own (a conversation, held), `echoctl capture` started `startAt` seconds into it; `then` runs on the
 *  capture, and the capture and the call end after it whatever happens. */
async function captureIn(conversation, startAt, title, then) {
  expect(call('end')).toBe(r().noCall)
  call('start', conversation, '--hold')
  let cap
  try {
    await expect.poll(() => inCall().for_s, { intervals: [100] }).toBeGreaterThanOrEqual(startAt)
    cap = data.start('capture', '--title', title)
    await expect.poll(() => cap.err, { timeout: 30_000 }).toContain(r().recording)
    await then(cap)
  } finally {
    if (cap) {
      cap.kill('SIGKILL')
      await cap.ends()
    }
    call('end')
  }
}

// The scenario's prelude, step 1: the test's speech model linked into the run's config and in use, its language
// English, so a capture transcribes as it records (stages 4–5); without the test's models, stage 4 records without.
test.beforeAll(() => {
  if (!models()) return
  fs.symlinkSync(models(), path.join(data.env.ECHOCTL_CONFIG_DIR, sp().models))
  for (const args of [['model', 'use', set().model], ['config', '--languages', set().language]]) {
    const r = data.cli(...args)
    if (r.code !== 0) throw new Error(`${args.join(' ')}: ${r.err}`)
  }
})

let app // the prelude's app: stages 1–11 use it, stage 11 quits it
const stage7 = {} // stage 7's recording and where its events start, read by stage 8 (its call goes on there)

scenario(test, 'S-18', 'Recording off the happy path: cancels, expiry, opt-in, Ctrl-C, a crash, mid-call starts, stops, ask mode, a drop, silence, two voices', () => {
  // The prelude, step 1's rest and step 2: nothing asked, no automatic minutes, no live notes; then the app.
  test.beforeAll(async () => {
    for (const args of [['config', '--languages', set().language, '--unscheduled', 'manual', '--scheduled', 'manual', '--auto-minutes', 'false'], ['ai', 'live', 'off']]) {
      const r = data.cli(...args)
      if (r.code !== 0) throw new Error(`${args.join(' ')}: ${r.err}`)
    }
    app = await data.app()
    const w = data.status()
    if (w.watcher.recording || w.watcher.armed || w.armed) throw new Error(`the app isn't idle: ${JSON.stringify(w)}`)
  })

  test('1 · A cancelled arm leaves nothing armed: the arm is taken, then `echoctl stop` disarms it', covers('recording/TC-003', 'F-REC-02'), async () => {
    const s = set().cancel
    const arm = data.cli('arm', '--title', s.title)
    expect(arm.out.trim()).toBe(a().armed)
    await expect.poll(() => watcher().armed, { timeout: 10_000 }).toBe(true)
    expect(data.status().armed.title).toBe(s.title)
    const stop = data.cli('stop')
    expect(stop.out.trim()).toBe(a().disarmed)
    await expect.poll(() => watcher().armed, { timeout: 10_000 }).toBe(false)
    expect(data.status().armed).toBeNull()
    expect(data.exists(data.home, a().stateDir, a().armedFile)).toBe(false)
  })

  test('2 · An arm older than 15 minutes expires: nothing stays armed, and the events say so', covers('recording/TC-004', 'F-REC-02'), async () => {
    const s = set().expire
    const from = data.events().length
    const arm = data.cli('arm', '--title', s.title)
    expect(arm.out.trim()).toBe(a().armed)
    // The arm made 16 minutes old instead of waiting them out.
    const file = path.join(data.home, a().stateDir, a().armedFile)
    const armed = JSON.parse(fs.readFileSync(file, 'utf8'))
    armed.armed_at = new Date(Date.now() - s.minutes * 60_000).toISOString().replace(/\.\d+Z$/, 'Z')
    fs.writeFileSync(file, JSON.stringify(armed))
    await expect.poll(() => events(from), { timeout: 10_000 }).toContain(ev().armExpired)
    await expect.poll(() => [data.status().armed, watcher().armed], { timeout: 10_000 }).toEqual([null, false])
    expect(data.events(from).at(-1)).toMatchObject({ event: ev().armExpired, title: s.title })
  })

  test('3 · A call nobody opted in to, right after a cancelled and an expired arm, records nothing', covers('privacy/TC-001', 'F-PRIV-01', 'F-REC-03', 'F-REC-02'), async () => {
    const s = set().optIn
    const from = data.events().length
    const before = calls()
    const t0 = Date.now()
    let during
    try {
      call('start', s.conversation, '--hold')
      await expect.poll(() => watcher().in_call, { timeout: 15_000 }).toBe(true)
      // A watcher's recording starts within a second of the call: these seconds check that none does.
      await seconds(s.quiet)
      during = data.status()
    } finally {
      hangUp()
    }
    expect(during.watcher).toMatchObject({ recording: null, armed: false })
    expect(during.armed).toBeNull()
    expect(calls()).toEqual(before)
    const started = data.events(from).filter((e) => [ev().callStarted, ev().recordingStarted].includes(e.event))
    expect(started).toHaveLength(1)
    expect(started[0]).toMatchObject({ event: ev().callStarted, armed: false, ask: false })
    // No audio made since the call began, under the data or the config (links not followed).
    const newWavs = [data.home, data.env.ECHOCTL_CONFIG_DIR].flatMap((d) => fs.readdirSync(d, { recursive: true, withFileTypes: true })
      .filter((e) => e.isFile() && e.name.endsWith(sp().audio) && fs.statSync(path.join(e.parentPath, e.name)).mtimeMs >= t0))
    expect(newWavs).toEqual([])
  })

  test('4 · A capture stopped with Ctrl-C ends by itself, closed as "user", its two tracks holding what was said until then', covers('recording/TC-016', 'F-REC-05', 'F-REC-07'), async () => {
    test.setTimeout(3 * 60_000)
    const s = set().stop
    let ended, err, stillOn
    await captureIn(s.conversation, s.startAt, s.title, async (cap) => {
      // The call's timeline: Ctrl-C 20 s after the capture started, 2 s into the call.
      await expect.poll(() => inCall().for_s, { timeout: 60_000, intervals: [100] }).toBeGreaterThanOrEqual(s.stopAt)
      cap.kill('SIGINT')
      ended = await cap.ends()
      err = cap.err
      stillOn = inCall().in_call
    })
    expect(ended).toEqual({ code: 0, signal: null })
    expect(stillOn).toBe(true)
    expect(err.indexOf(r().recording)).toBeGreaterThanOrEqual(0)
    for (const file of r().tracks) {
      const lines = trackLine(err, file)
      expect(lines, file).toHaveLength(1)
      expect(err.indexOf(lines[0][0]), file).toBeGreaterThan(err.indexOf(r().recording))
      expect(Number(lines[0][1]), file).toBeGreaterThanOrEqual(s.seconds[0])
      expect(Number(lines[0][1]), file).toBeLessThanOrEqual(s.seconds[1])
    }
    const dir = folderOf(s.slug)
    const meta = data.json(dir, sp().meta)
    expect(meta.ended_at).toBeTruthy()
    expect(meta.stop_reason).toBe(r().stop.user)
    expect(meta.tracks.map((t) => t.file).sort()).toEqual([...r().tracks].sort())
    for (const t of meta.tracks) expect(t.frames, t.file).toBeGreaterThan(0)
    const files = fs.readdirSync(dir)
    for (const file of r().tracks) expect(files).toContain(file)
    const remote = wavInfo(path.join(dir, r().remote))
    expect(remote.seconds).toBeGreaterThanOrEqual(s.seconds[0])
    expect(remote.seconds).toBeLessThanOrEqual(s.seconds[1])
    expect(remote.peak).toBeGreaterThan(s.peak)
  })

  test('5 · A capture killed while recording leaves an unclosed call; `echoctl transcribe` repairs it as "recovered" and transcribes it up to the kill', covers('recording/TC-011', 'F-REC-11'), async () => {
    test.skip(!models(), `${sp().testModels} isn't set: the speech model comes only from the test's own folder, never the installed Echo's`)
    test.setTimeout(4 * 60_000)
    const s = set().crash
    let ended, out
    await captureIn(s.conversation, s.startAt, s.title, async (cap) => {
      await expect.poll(() => inCall().for_s, { timeout: 60_000, intervals: [100] }).toBeGreaterThanOrEqual(s.killAt)
      cap.kill('SIGKILL')
      ended = await cap.ends()
      out = cap.out
    })
    expect(ended).toEqual({ code: null, signal: 'SIGKILL' })
    expect(out).toBe('')
    const dir = folderOf(s.slug)
    expect(fs.readdirSync(dir).sort()).toEqual(expect.arrayContaining(s.files))
    expect(fs.readdirSync(dir)).not.toContain(sp().transcript)
    let meta = data.json(dir, sp().meta)
    expect(meta.ended_at).toBeNull()
    expect(meta.stop_reason).toBeNull()

    const t = data.cli('transcribe', dir)
    expect(t.code, t.err).toBe(0)
    expect((t.out + t.err).split('\n').filter(Boolean).at(-1)).toMatch(new RegExp(r().doneIn))
    meta = data.json(dir, sp().meta)
    expect(meta.ended_at).toBeTruthy()
    expect(meta.stop_reason).toBe(r().stop.recovered)
    const files = fs.readdirSync(dir)
    for (const f of s.transcribed) expect(files).toContain(f)
    expect(files.filter((f) => f.endsWith(sp().audio))).toEqual([])
    const text = data.read(dir, sp().transcript)
    expect(text).toContain(s.said)
    for (const words of s.after) expect(text).not.toContain(words)
    for (const seg of data.json(dir, sp().segments)) expect(seg.start).toBeLessThan(s.before)
  })

  test('6 · A call armed in its middle records from the arm on: what was said before it is not in the transcript', covers('recording/TC-002', 'F-REC-01'), async () => {
    test.skip(!models(), `${sp().testModels} isn't set: the speech model comes only from the test's own folder, never the installed Echo's`)
    test.setTimeout(4 * 60_000)
    const s = set().midCall
    const from = data.events().length
    let inCallState, arm, recording
    expect(hangUp()).toBe(r().noCall)
    try {
      call('start', s.conversation, '--hold')
      await expect.poll(() => watcher().in_call, { timeout: 15_000 }).toBe(true)
      inCallState = data.status()
      // The call's timeline: the arm about 13 s in, then the rest of the conversation.
      await callAt(s.armAt)
      arm = data.cli('arm', '--title', s.title)
      await expect.poll(() => watcher().recording || '', { timeout: 10_000 }).toMatch(new RegExp(`-${literal(s.slug)}$`))
      recording = watcher().recording
      await callAt(s.endAt)
    } finally {
      hangUp()
    }
    expect(inCallState.watcher).toMatchObject({ in_call: true, recording: null })
    expect(inCallState.armed).toBeNull()
    expect(arm.out.trim()).toBe(a().armedInCall)
    // One recording, after the call's start: the call began unarmed.
    expect(events(from).filter((e) => e === ev().recordingStarted)).toHaveLength(1)
    const all = events(0)
    expect(all.lastIndexOf(ev().callStarted)).toBeLessThan(all.lastIndexOf(ev().recordingStarted))
    await expect.poll(() => events(from), { timeout: 30_000 }).toContain(ev().recordingStopped)
    const text = await transcriptOf(recording)
    const files = fs.readdirSync(recording)
    for (const f of [sp().transcript, sp().segments]) expect(files).toContain(f)
    expect(files.filter((f) => f.endsWith(sp().audio))).toEqual([])
    expect(linesWith(text, s.before)).toBe(0)
    expect(linesWith(text, s.after)).toBeGreaterThanOrEqual(s.afterLines)
  })

  test('7 · `echoctl arm` and `echoctl stop` drive a recording from the terminal; the call goes on without it', covers('cli/TC-004', 'F-CLI-11', 'F-REC-02', 'F-REC-13'), async () => {
    test.setTimeout(2 * 60_000)
    const s = set().terminal
    expect(hangUp()).toBe(r().noCall)
    stage7.from = data.events().length
    const before = calls()
    const arm = data.cli('arm', '--title', s.title)
    expect(arm.out.trim()).toBe(a().armed)
    await expect.poll(() => watcher().armed, { timeout: 10_000 }).toBe(true)
    expect(data.status().armed.title).toBe(s.title)
    // The call goes on for stage 8: hung up there.
    call('start', s.conversation, '--hold')
    await expect.poll(() => watcher().recording || '', { timeout: 20_000 }).toMatch(new RegExp(`-${literal(s.slug)}$`))
    const recording = watcher().recording
    expect(watcher().armed).toBe(false)
    // The call's timeline: the stop about 20 s in.
    await callAt(s.stopAt)
    const stop = data.cli('stop')
    expect(stop.out.trim()).toBe(fill(a().stopping, { folder: recording }))
    await expect.poll(() => watcher().recording, { timeout: 10_000 }).toBeNull()
    expect(data.status().armed).toBeNull()
    const c = inCall()
    expect(c.in_call).toBe(true)
    expect(c.for_s).toBeGreaterThanOrEqual(s.stopAt)
    expect(c.for_s).toBeLessThan(s.stoppedBy)
    expect(added(before)).toEqual([path.basename(recording)])
    const seen = events(stage7.from).filter((e) => [ev().callStarted, ev().recordingStarted, ev().recordingStopped].includes(e))
    expect(seen).toEqual([ev().callStarted, ev().recordingStarted, ev().recordingStopped])
    expect(data.cli('stop').out.trim()).toBe(a().nothingToStop)
    stage7.recording = recording
  })

  test('8 · After `echoctl stop` the call runs on unrecorded to its end; its transcript ends at the stop, closed as "user"', covers('recording/TC-015', 'F-REC-05'), async () => {
    test.skip(!models(), `${sp().testModels} isn't set: the speech model comes only from the test's own folder, never the installed Echo's`)
    test.setTimeout(3 * 60_000)
    const s = set().terminal
    let state, during, after
    try {
      state = data.status()
      during = inCall()
      // The conversation runs out (38 s on the call's clock), then the hang-up.
      await callAt(s.endAt)
      after = data.status()
    } finally {
      hangUp()
    }
    expect(state.watcher).toMatchObject({ recording: null, in_call: true })
    expect(state.armed).toBeNull()
    expect(during.in_call).toBe(true)
    expect(after.watcher.recording).toBeNull()
    await expect.poll(() => events(stage7.from), { timeout: 30_000 }).toContain(ev().callEnded)
    expect(calls().filter((n) => n.endsWith(`-${s.slug}`))).toHaveLength(1)
    expect(events(stage7.from).filter((e) => e === ev().recordingStarted)).toHaveLength(1)
    expect(events(stage7.from).filter((e) => e === ev().recordingStopped)).toHaveLength(1)
    const text = await transcriptOf(stage7.recording)
    const files = fs.readdirSync(stage7.recording)
    for (const f of [sp().transcript, sp().segments]) expect(files).toContain(f)
    expect(files.filter((f) => f.endsWith(sp().audio))).toEqual([])
    expect(data.json(stage7.recording, sp().meta).stop_reason).toBe(r().stop.user)
    expect(linesWith(text, [s.said])).toBeGreaterThanOrEqual(1)
    expect(linesWith(text, s.notSaid)).toBe(0)
  })

  test('9 · The pill\'s Stop asks first: Keep recording keeps it, Stop ends the recording and not the call', covers('recording/TC-005', 'F-REC-05'), async ({ page }) => {
    test.skip(!models(), `${sp().testModels} isn't set: the speech model comes only from the test's own folder, never the installed Echo's`)
    test.setTimeout(4 * 60_000)
    const s = set().pill
    const window = new AppPage(page, data)
    await window.goto()
    expect(hangUp()).toBe(r().noCall)
    const from = data.events().length
    let recording, kept, stopped, c
    try {
      data.cli('arm', '--title', s.title)
      await expect.poll(() => watcher().armed, { timeout: 10_000 }).toBe(true)
      call('start', s.conversation, '--hold')
      await expect.poll(() => watcher().recording || '', { timeout: 20_000 }).toMatch(new RegExp(`-${literal(s.slug)}$`))
      recording = watcher().recording
      // No reload: the page follows the app.
      await window.pillStop.click()
      await expect(window.stopQuestion).toBeVisible()
      await expect(window.stopRecording).toBeVisible()
      await window.keepRecording.click()
      await expect(window.recordingPill).toBeVisible()
      await expect(window.stopQuestion).toBeHidden()
      kept = watcher().recording
      await window.pillStop.click()
      await window.stopRecording.click()
      await expect.poll(() => watcher().recording, { timeout: 4_000 }).toBeNull()
      stopped = data.status()
      c = inCall()
      // The conversation runs out, then the hang-up.
      await callAt(s.endAt)
    } finally {
      hangUp()
    }
    expect(kept).toBe(recording)
    expect(stopped.watcher.in_call).toBe(true)
    expect(stopped.armed).toBeNull()
    expect(c.for_s).toBeLessThan(s.stoppedBy)
    await expect.poll(() => events(from), { timeout: 30_000 }).toContain(ev().callEnded)
    await expect.poll(() => events(from), { timeout: 120_000 }).toContain(ev().transcribed)
    const order = events(from).filter((e) => [ev().callStarted, ev().recordingStarted, ev().recordingStopped, ev().callEnded, ev().transcribed].includes(e))
    expect(order.slice(0, 3)).toEqual([ev().callStarted, ev().recordingStarted, ev().recordingStopped])
    expect(order.slice(3).sort()).toEqual([ev().callEnded, ev().transcribed].sort())
    expect(calls().filter((n) => n.endsWith(`-${s.slug}`))).toHaveLength(1)
    expect(data.json(recording, sp().meta).stop_reason).toBe(r().stop.user)
    expect(linesWith(data.read(recording, sp().transcript), s.notSaid)).toBe(0)
  })

  test('11 · A call dropped and rejoined within seconds is one recording, one folder and one transcript', covers('recording/TC-006', 'F-REC-08'), async () => {
    test.skip(!models(), `${sp().testModels} isn't set: the speech model comes only from the test's own folder, never the installed Echo's`)
    test.setTimeout(5 * 60_000)
    const s = set().rejoin
    expect(hangUp()).toBe(r().noCall)
    const from = data.events().length
    const before = calls()
    let recording, rejoined, after, quit
    try {
      data.cli('arm', '--title', s.title)
      await expect.poll(() => watcher().armed, { timeout: 10_000 }).toBe(true)
      call('start', s.conversation, '--hold')
      await expect.poll(() => watcher().recording || '', { timeout: 20_000 }).toMatch(new RegExp(`-${literal(s.slug)}$`))
      recording = watcher().recording
      // The call's timeline: the first conversation runs out, the drop, the rejoin 4 s later, then 5 s into it.
      await callAt(s.endAt)
      hangUp()
      await seconds(s.drop)
      call('start', s.then, '--hold')
      await callAt(s.checkAt)
      rejoined = watcher()
      await callAt(s.thenEndAt)
      hangUp()
      await expect.poll(() => events(from), { timeout: 30_000 }).toContain(ev().recordingStopped)
      await expect.poll(() => events(from), { timeout: 120_000 }).toContain(ev().transcribed)
      after = events(from)
    } finally {
      hangUp()
      // The last stage on the prelude's app.
      quit = await app.quit()
    }
    expect(quit.code, quit.err).toBe(0)
    expect(data.status().watcher).toBeNull()
    expect(rejoined).toMatchObject({ in_call: true, recording })
    expect(added(before)).toEqual([path.basename(recording)])
    expect(after.slice(after.indexOf(ev().recordingStarted) + 1)).toEqual([ev().callEnded, ev().callResumed, ev().callEnded, ev().recordingStopped, ev().transcribed])
    expect(after.filter((e) => e === ev().recordingStarted)).toHaveLength(1)
    const text = data.read(recording, sp().transcript)
    expect(linesWith(text, s.first)).toBeGreaterThanOrEqual(s.lines)
    expect(linesWith(text, s.second)).toBeGreaterThanOrEqual(s.lines)
    // The gap is silence: the first conversation's last line is followed by the second's first turn (in one segment
    // or split in two), nothing between them.
    const segs = data.json(recording, sp().segments).map((x) => x.text)
    const last = segs.findLastIndex((t) => t.includes(s.lastOfFirst))
    expect(last).toBeGreaterThanOrEqual(0)
    const words = (t) => t.toLowerCase().match(/[a-z0-9]+/g)
    const turn = words(s.secondFirstTurn)
    const next = segs.slice(last + 1, last + 3)
    const end = next.findIndex((t) => t.includes(s.joining))
    expect(end).toBeGreaterThanOrEqual(0)
    for (const w of next.slice(0, end + 1).flatMap(words)) expect(turn).toContain(w)
  })

  test('12 · Twenty seconds of silence in a recorded call give no restart and no text', covers('recording/TC-017', 'F-STT-03', 'F-REC-07'), async () => {
    test.skip(!!rec.none, rec.none)
    const s = set().silence
    const err = prepared(s.recording, sp().captureErr)
    for (const file of r().tracks) {
      const lines = trackLine(err, file)
      expect(lines, file).toHaveLength(1)
      expect(Number(lines[0][2]), file).toBe(0)
    }
    expect(err).not.toContain(r().restarting)
    const meta = JSON.parse(prepared(s.recording, sp().meta))
    const [a, b] = meta.tracks.map((t) => t.frames / t.sample_rate)
    expect(meta.tracks).toHaveLength(2)
    expect(Math.abs(a - b)).toBeLessThanOrEqual(s.skew)
    for (const t of meta.tracks) expect(t.events.map((e) => e.kind)).not.toContain(r().writeFailed)
    const segs = data.json(copy(s.recording, s.title), sp().segments)
    expect(segs.length).toBeGreaterThanOrEqual(s.segments)
    expect(segs.filter((x) => x.start > s.quiet[0] && x.start < s.quiet[1])).toEqual([])
    // Who spoke, in turn: Me, the remote side, Me.
    const runs = segs.map((x) => x.speaker).filter((who, i, all) => who !== all[i - 1])
    expect(runs).toEqual(s.speakers)
  })

  test("13 · Two voices on the remote track are both Remote, the user's lines Me (step 1)", covers('recording/TC-018'), async () => {
    test.skip(!!rec.none, rec.none)
    const s = set().voices
    const call = copy(s.recording, s.title)
    // Each segment matched to the conversation's turn it falls in (make_audio.py's score): the speaker it was given.
    const score = JSON.parse(data.run('python3', sp().makeAudio, 'score', call))
    const expected = data.json(call, sp().expected).turns
    expect(score.turns).toHaveLength(expected.length)
    expect([...new Set(expected.map((t) => t.who))].sort()).toEqual(Object.keys(s.sides).sort())
    for (const [i, turn] of expected.entries()) {
      expect(turn.speaker, turn.who).toBe(s.sides[turn.who])
      expect(score.turns[i], turn.who).toMatchObject({ speaker: s.sides[turn.who], speaker_ok: true })
    }
  })

  test('25 · A Stop while Echo is frozen for a moment ends the recording once it is free, and the next meeting stays armed', covers('recording/TC-025', 'F-REC-05'), async () => {
    test.setTimeout(3 * 60_000)
    const s = set().busy
    expect(hangUp()).toBe(r().noCall)
    // Its own app: the prelude's quit first (stage 11 quits it; not when this stage runs alone).
    if (!app.ended) expect((await app.quit()).code).toBe(0)
    const own = await data.app()
    const from = data.events().length
    let recording, next, stop, files, after, quit
    try {
      data.cli('arm', '--title', s.title)
      await expect.poll(() => watcher().armed, { timeout: 10_000 }).toBe(true)
      call('start', s.conversation, '--hold')
      await expect.poll(() => watcher().recording || '', { timeout: 20_000 }).toMatch(new RegExp(`-${literal(s.slug)}$`))
      recording = watcher().recording
      next = data.cli('arm', '--title', s.next)
      // The app frozen as if a device call hung: its heartbeat goes stale.
      own.kill('SIGSTOP')
      await seconds(s.frozen)
      stop = data.cli('stop')
      files = [a().stopRequest, a().armedFile].filter((f) => data.exists(data.home, a().stateDir, f))
      own.kill('SIGCONT')
      await expect.poll(() => events(from), { timeout: 15_000 }).toContain(ev().recordingStopped)
      await expect.poll(() => watcher().pid, { timeout: 10_000 }).toBe(own.pid)
      after = data.status()
    } finally {
      own.kill('SIGCONT')
      hangUp()
      data.cli('stop')
      quit = await own.quit()
    }
    expect(quit.code, quit.err).toBe(0)
    expect(next.out.trim()).toBe(a().armedInCall)
    expect(stop.out.trim()).toBe(`${fill(a().stopping, { folder: recording })} ${a().busy}`)
    expect(files).toEqual([a().stopRequest, a().armedFile])
    expect(after.watcher).toMatchObject({ recording: null, armed: true })
    expect(data.json(recording, sp().meta).stop_reason).toBe(r().stop.user)
  })
})
