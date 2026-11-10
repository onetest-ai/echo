// The recordings prepared once a build (tests/fixtures/recordings.py prepare: real capture from files and
// transcription, a few minutes), for every spec that copies or scores them: prepared before the spec's first test when
// this build has none yet, whichever spec runs first. Speech models only from the folder ECHOCTL_TEST_MODELS names,
// never the installed Echo's: without it, nothing is prepared and the spec's stages that need a recording skip, saying
// why (`test.skip(!!rec.none, rec.none)`).
//
//   const data = useData(test, { set: 'small' })
//   const rec = useRecordings(test, data)
import fs from 'node:fs'

/** Registers the check (after `useData`'s, so the data's environment is there); its state says why there are none. */
export function useRecordings(test, data) {
  const state = { none: null }
  test.beforeAll(async () => {
    test.setTimeout(15 * 60_000)
    const s = data.text.speech
    try {
      data.run('python3', s.recordings, 'list')
      return
    } catch {
      // none for this build yet
    }
    if (!process.env[s.testModels]) {
      state.none = `no recordings prepared for this build, and ${s.testModels} isn't set (the test's own speech models; never the installed Echo's)`
      return
    }
    data.run('python3', s.recordings, 'prepare', { timeout: 14 * 60_000 })
  })
  return state
}

/** A track as a recording wrote it (16-bit PCM wav): its length in seconds and its loudest sample (0–32768). */
export function wavInfo(file) {
  const b = fs.readFileSync(file)
  let at = 12, rate = 0, channels = 1, pcm = null
  while (at + 8 <= b.length) {
    const id = b.toString('ascii', at, at + 4)
    const size = b.readUInt32LE(at + 4)
    if (id === 'fmt ') [channels, rate] = [b.readUInt16LE(at + 10), b.readUInt32LE(at + 12)]
    if (id === 'data') { pcm = b.subarray(at + 8, Math.min(b.length, at + 8 + size)); break }
    at += 8 + size + (size % 2)
  }
  let peak = 0
  for (let i = 0; i + 1 < pcm.length; i += 2) peak = Math.max(peak, Math.abs(pcm.readInt16LE(i)))
  return { seconds: pcm.length / 2 / channels / rate, peak }
}
