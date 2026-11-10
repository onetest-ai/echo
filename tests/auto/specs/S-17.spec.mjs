// S-17 A recorded call end to end — the stages with fixed checks that need no Echo app (small data, the fake Claude Code,
// the test clock): what the recordings prepared once a build say about `echoctl capture` and `echoctl transcribe` (the
// capture's two tracks, no audio kept: stages 4–5, their `prepare` halves), a language pinned for one call (6), a call
// too short for automatic minutes (7), and the call's files handed to macOS from Call actions (10, `open` only logged).
// The prepared recordings come only from the test's own speech models (ECHOCTL_TEST_MODELS): without them, the stages
// that read one skip.
// Not here:
// - the prelude and stages 1–3 (window/TC-014 notes while recording, window/TC-013 the call filling in, recording/TC-001
//   record the next call): the main call is armed and recorded by the Echo app (`echoctl app run`);
// - stage 4 steps 1–3 and stage 5 step 3's first half (recording/TC-014, privacy/TC-003): they read that main call;
// - stages 8–9 (claude/TC-003 automatic minutes, claude/TC-010 a note moves them): the app writes them;
// - stage 11 (recording/TC-019, the tray's icon): a person's;
// - stages 12–13 (claude/TC-019, window/TC-024): an app instance recording a call of its own;
// - stage 14 (claude/TC-020): an app instance and a signed-in Claude Code.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { useRecordings } from '../support/recordings.mjs'
import { CallsPage } from '../pages/calls.page.mjs'

const data = useData(test, { set: 'small', ai: 'fake', args: ['--clock', 'fast'] })
const rec = useRecordings(test, data)

const r = () => data.text.recording
const sp = () => data.text.speech
const cs = () => data.text.callStates
const set = () => data.set.s17
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

scenario(test, 'S-17', 'A recorded call end to end: record, take notes, transcript, automatic minutes, the files', () => {
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
})
