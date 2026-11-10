// S-18 Recording off the happy path — the stages with fixed checks that need no Echo app (small data, the fake Claude
// Code): a capture stopped by Ctrl-C (stage 4) and one killed (5), each recorded from files (`tests/fixtures/call.py`,
// no device, no sound) on a call of its own; silence (12) and two remote voices (13) in the recordings prepared once a
// build. Speech models only from the test's own folder (ECHOCTL_TEST_MODELS): without it, the stages that transcribe or
// read a prepared recording skip.
// Not here:
// - the prelude and stages 1–3 and 6–11 (recording/TC-003, TC-004, privacy/TC-001, recording/TC-002, cli/TC-004,
//   recording/TC-015, TC-005, TC-012, TC-006): the Echo app's watcher (`echoctl app run`), its arm, stop, pill and
//   "Record?" dialog;
// - stages 14–24 (privacy/TC-002, recording/TC-023, TC-007, TC-008, permissions/TC-006, recording/TC-009, TC-010,
//   transcription/TC-006, recording/TC-020, TC-021, TC-022): a person's (real devices, Teams, the Mac's sleep, System
//   Settings);
// - stages 25–27 (recording/TC-025, transcription/TC-010, TC-012): each an app instance of its own;
// - stage 13 steps 2–3 (recording/TC-018, `echoctl speakers` and two speaker labels): the test's models folder has no
//   speaker model, so the prepared recording has no detected speakers (and F-STT-04, two voices told apart, isn't
//   claimed: step 1 checks only that both stay Remote).
// Stages 4–5 run on their own (each case's "on its own"): a call started for the capture and hung up after it, not the
// scenario's main call.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { useRecordings, wavInfo } from '../support/recordings.mjs'

const data = useData(test, { set: 'small', ai: 'fake' })
const rec = useRecordings(test, data)

const r = () => data.text.recording
const sp = () => data.text.speech
const set = () => data.set.s18
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

scenario(test, 'S-18', 'Recording off the happy path: cancels, expiry, opt-in, Ctrl-C, a crash, mid-call starts, stops, ask mode, a drop, silence, two voices', () => {
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
})
