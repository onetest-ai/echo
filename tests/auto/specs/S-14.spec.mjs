// S-14 Transcription quality on known conversations — the stages with fixed checks (small data, CLI only): the
// recordings prepared once a build scored by make_audio.py (its 15% word-error limit, speakers, languages, silence:
// stages 1–4) and a vocabulary nobody says costing no second decode (stage 8). Speech models only from the folder
// ECHOCTL_TEST_MODELS names, never the installed Echo's: without it, a stage that needs a model is skipped.
// Not here: stages 5–6 (a transcription server on a second computer; stage 6's form also reads the Ollama on this
// computer) and stage 7 (Parakeet v3 and the NeMo-Speech runtime of an installed Echo).
import { test, expect } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { useData, fill } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { useRecordings } from '../support/recordings.mjs'

const data = useData(test, { set: 'small' })
/** The folder of the test's own speech models (ECHOCTL_TEST_MODELS), if set. */
const models = () => process.env[data.text.speech.testModels]
// The recordings of this build, prepared when it has none yet — only with the test's own models; `state.none` says why
// there are none.
const state = useRecordings(test, data)

/** A copy of a prepared recording in the data's calls, under a title: its folder. */
const copy = (recording, title) => data.run('python3', data.text.speech.recordings, 'copy', recording, '--title', title).split('\n').at(-1)
/** `make_audio.py score <call>`, read (it exits 1 when a check fails: the run then fails with its JSON). */
const score = (call) => JSON.parse(data.run('python3', data.text.speech.makeAudio, 'score', call))
/** The data's calls folder (where a copy or a built call goes). */
const callsFolder = () => path.dirname(data.callDir(data.set.calls.withMinutes.folder))
/** What the prepared recording's own files say: the cache folder of this build. */
const prepared = (...p) => data.read(data.run('python3', data.text.speech.recordings, 'path'), ...p)
/** The "  language: …" lines of a text. */
const languageLines = (text) => text.split('\n').filter((l) => new RegExp(data.text.speech.languageLine).test(l))

scenario(test, 'S-14', 'Transcription quality on known conversations: words, speakers, languages, silence', () => {
  test('1 · The transcript has the words, speakers and order of the call', covers('transcription/TC-001', 'F-TR-01', 'F-STT-04', 'F-STT-03'), async () => {
    test.skip(!!state.none, state.none)
    const { s14 } = data.set
    const s = data.text.speech
    const call = copy(s14.acme.recording, s14.acme.title)
    expect(path.dirname(call)).toBe(callsFolder())
    const files = fs.readdirSync(call)
    for (const f of s.files) expect(files).toContain(f)
    expect(files.filter((f) => f.endsWith(s.audio))).toEqual([])
    const r = score(call)
    expect(r.pass).toBe(true)
    expect(r.wer).toBeLessThanOrEqual(s14.maxWer)
    for (const turn of r.turns) expect(turn).toMatchObject({ speaker_ok: true, lang_ok: true })
    expect(r.transcribed_in_silence).toEqual([])
    // The transcript: turns in time order, each said by whom the conversation has it, the words as said, the model.
    const text = data.read(call, s.transcript)
    const turns = text.split('\n').map((l) => new RegExp(s.turnLine).exec(l)).filter(Boolean)
    const times = turns.map((m) => m[2])
    expect(times).toEqual([...times].sort())
    const expected = data.json(call, s.expected).turns
    expect(turns.map((m) => m[1])).toEqual(expected.map((t) => s.speakers[t.speaker]))
    for (const words of s14.acme.says) expect(text).toContain(words)
    expect(text).toContain(fill(s.modelLine, { model: s14.model }))
  })

  test('2 · A call started without a language is in the default one', covers('transcription/TC-003', 'F-STT-02', 'F-SET-07'), async () => {
    test.skip(!!state.none, state.none)
    const { s14 } = data.set
    const s = data.text.speech
    const { recording, title, language } = s14.twoRemotes
    expect(JSON.parse(prepared(recording, s.meta)).language).toBe(language)
    const call = copy(recording, title)
    const r = score(call)
    expect(r.pass).toBe(true)
    expect(r.wer).toBeLessThanOrEqual(s14.maxWer)
    expect([...new Set(data.json(call, s.segments).map((seg) => seg.lang))]).toEqual([language])
    // The default language, written when it started: nothing detected it.
    expect(languageLines(prepared(recording, s.transcribeOut))).toEqual([fill(s.language, { lang: language })])
  })

  test('3 · A call comes out in its one language', covers('transcription/TC-002', 'F-STT-02'), async () => {
    test.skip(!!state.none, state.none)
    const { s14 } = data.set
    const s = data.text.speech
    for (const { recording, title, language } of s14.oneLanguage) {
      // Only the language it was started in, in the live pass and the transcription: nothing detected it.
      const lines = [...languageLines(prepared(recording, s.captureErr)), ...languageLines(prepared(recording, s.transcribeOut))]
      expect(lines.length, recording).toBeGreaterThan(0)
      for (const l of lines) expect(l, recording).toBe(fill(s.language, { lang: language }))
      const call = copy(recording, title)
      const r = score(call)
      expect(r.pass, recording).toBe(true)
      expect(r.wer, recording).toBeLessThanOrEqual(s14.maxWer)
      for (const turn of r.turns) expect(turn, recording).toMatchObject({ lang: language, lang_ok: true })
      expect(data.json(call, s.meta).language, recording).toBe(language)
    }
  })

  test('4 · Silence produces no made-up text', covers('transcription/TC-005', 'F-STT-02'), async () => {
    test.skip(!!state.none, state.none)
    const { s14 } = data.set
    const r = score(copy(s14.silence.recording, s14.silence.title))
    expect(r.pass).toBe(true)
    expect(r.wer).toBeLessThanOrEqual(s14.maxWer)
    expect(r.transcribed_in_silence).toEqual([])
  })

  test('8 · A vocabulary term nobody says costs no second decode', covers('transcription/TC-009', 'F-VOC-04'), async () => {
    const s = data.text.speech
    test.skip(!models(), `${s.testModels} isn't set: the speech model comes only from the test's own folder, never the installed Echo's`)
    test.setTimeout(5 * 60_000)
    const { s14 } = data.set
    const v = s14.vocabulary
    // The test's own speech model, linked into the run's config.
    fs.symlinkSync(models(), path.join(data.env.ECHOCTL_CONFIG_DIR, s.models))
    expect(data.cli('model', 'use', s14.model).code).toBe(0)
    const day = data.callName(data.set.calls.withMinutes.folder).slice(0, 10)
    const transcribe = (suffix) => {
      const call = path.join(callsFolder(), day + suffix)
      expect(data.run('python3', s.makeAudio, 'build', v.conversation, call)).toContain(fill(s.built, { n: v.turns }))
      const r = data.cliWith({ [s.debug]: '1' }, 'transcribe', call)
      expect(r.code, r.err).toBe(0)
      return { call, err: r.err }
    }
    expect(data.cli('config', '--vocabulary', v.terms).code).toBe(0)
    const withIt = transcribe(v.withIt)
    expect(withIt.err).toContain(fill(s.hint, { terms: v.hint }))
    expect(data.cli('config', '--vocabulary', '').code).toBe(0)
    const plain = transcribe(v.plain)
    expect(plain.err).not.toContain(fill(s.hint, { terms: '' }).trim())
    // One decode an utterance with the vocabulary, as with none.
    const decodes = (err) => err.split('\n').filter((l) => l.includes(s.noSpeech)).length
    expect(decodes(withIt.err)).toBeGreaterThan(0)
    expect(decodes(withIt.err)).toBe(decodes(plain.err))
    const text = data.read(withIt.call, s.transcript)
    for (const term of v.unsaid) expect(text).not.toContain(term)
    expect(v.said.some((words) => text.includes(words))).toBe(true)
  })
})
