// The recordings prepared once a build (tests/fixtures/recordings.py prepare: real capture from files and
// transcription, a few minutes), for every spec that copies or scores them: prepared before the spec's first test when
// this build has none yet, whichever spec runs first. Speech models only from the folder ECHOCTL_TEST_MODELS names,
// never the installed Echo's: without it, nothing is prepared and the spec's stages that need a recording skip, saying
// why (`test.skip(!!rec.none, rec.none)`).
//
//   const data = useData(test, { set: 'small' })
//   const rec = useRecordings(test, data)

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
