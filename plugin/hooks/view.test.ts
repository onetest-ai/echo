import { expect, test } from 'claude-code/testing'

import type { LiveLine, LiveView } from '../types'
import { actionsMarkdown, afterTranscribed, anchors, askAgain, callFromDir, clock, insightFromLive, minutesPhase, newLines, renamedTo, statusBadge, parseLines, speakerLabel, tailThatFits } from './view'
import type { MinutesAnswer } from './view'

const line = (text: string, speaker: 'me' | 'remote' = 'remote', start = 0): LiveLine =>
  ({ speaker, start, end: start + 1, lang: 'en', text })

test('parseLines skips blank and partial lines', async () => {
  const got = parseLines<{ a: number }>('{"a":1}\n\n{"a":2}\n{"a":')
  expect(got.map(x => x.a)).toEqual([1, 2])
})

test('newLines hands out whole lines only and survives truncation', async () => {
  expect(newLines('a\nb\nc', 0)).toEqual({ lines: 'a\nb\n', next: 4 })
  expect(newLines('a\nb\nc', 4)).toEqual({ lines: '', next: 4 })
  expect(newLines('x\n', 10)).toEqual({ lines: '', next: 2 })
})

test('tailThatFits keeps the newest lines that fit, oldest first', async () => {
  const lines = [line('one', 'me', 1), line('two', 'remote', 2), line('three', 'remote', 3)]
  const got = tailThatFits(lines, 2, 80)
  expect(got.map(l => l.text)).toEqual(['two', 'three'])
  const long = line('x'.repeat(200))
  expect(tailThatFits([long], 2, 80)).toEqual([]) // needs 3 rows, only 2 available
})

test('the mic track is "Me" under both its names', async () => {
  expect(speakerLabel(line('hi', 'me'))).toBe('Me')
  expect(speakerLabel({ ...line('hi'), speaker: 'mic' as never })).toBe('Me')
  expect(speakerLabel(line('hi', 'remote'))).toBe('Remote')
  expect(speakerLabel({ ...line('hi', 'remote'), name: 'Daniel Osei' })).toBe('Daniel Osei')
})

test('call folders and action items render for the browser', async () => {
  expect(callFromDir('2026-10-09_183453-lumen-freight-kickoff')).toEqual({ when: 'Oct 09 18:34', title: 'lumen freight kickoff' })
  expect(callFromDir('2026-10-09_182054')).toEqual({ when: 'Oct 09 18:20', title: 'call' })
  expect(actionsMarkdown('[{"task":"send deck","owner":"Me","due":"Friday","confidence":"high"}]')).toBe('- [ ] **Me** — send deck _(due Friday)_')
  expect(actionsMarkdown('[]')).toBe('_No action items._')
})

test('status badges and clocks for the call browser', async () => {
  expect(statusBadge('notes')).toEqual({ label: '✓ minutes', color: 'success' })
  expect(statusBadge('recording').color).toBe('claude')
  expect(clock(65)).toBe('01:05')
  expect(clock(3725)).toBe('1:02:05')
})

test('notes anchors become transcript jumps', async () => {
  const got = anchors('- Agreed X [02:31]\n- Again [02:31]\n- Late [1:05:09] and [bad] [7:03]')
  expect(got).toEqual([
    { label: '02:31', seconds: 151 },
    { label: '1:05:09', seconds: 3909 },
    { label: '7:03', seconds: 423 },
  ])
})

test('the app\'s live notes become the pane\'s sections', async () => {
  const i = insightFromLive({
    now: 'Budget for phase one', updated_at: '2026-10-23T01:20:00+02:00',
    decisions: [{ text: 'Go-live on 15 Jan', t: 196 }, { text: '' }],
    questions: [{ text: 'Who signs the SOW?' }],
    actions: [{ owner: 'Me', task: 'Send the plan', t: 62 }, { owner: '', task: 'Book the room' }],
  })
  expect(i.summary).toEqual(['Budget for phase one'])
  expect(i.highlights).toEqual(['Decided: Go-live on 15 Jan [03:16]', 'Open: Who signs the SOW?'])
  expect(i.actions).toEqual(['Me — Send the plan [01:02]', '? — Book the room'])
  expect(i.updatedAt).toBe(Date.parse('2026-10-23T01:20:00+02:00'))
  expect(insightFromLive(null)).toEqual({ summary: [], highlights: [], actions: [], updatedAt: null })
})

const answer = (a: Partial<MinutesAnswer>): MinutesAnswer => ({ requested: false, waiting: false, failed: false, reason: '', ...a })
const view = (v: Partial<LiveView>): LiveView =>
  ({ dir: '/calls/a', title: 'A', startedAt: 1, phase: 'transcribing', lines: [], notesRequested: false, chat: [], ...v })

test('the app\'s answer about the minutes: wait, done, or ask again', async () => {
  expect(minutesPhase(answer({ waiting: true, reason: 'Echo writes the minutes itself at 17:45' })))
    .toEqual({ phase: 'waiting-minutes', notesNote: 'Echo writes the minutes itself at 17:45' })
  expect(minutesPhase(answer({ reason: 'automatic minutes are off' }))).toEqual({ phase: 'done', notesNote: 'automatic minutes are off' })
  // An older app ran them; a CLI that couldn't answer is asked again (the minutes may still come).
  expect(minutesPhase(answer({ requested: true }))).toEqual({ phase: 'writing-notes', notesNote: null, notesRequested: true })
  expect(minutesPhase(answer({ failed: true, reason: "Echo's CLI didn't answer" })).phase).toBe('waiting-minutes')
})

test('a transcribed call changes only its own view', async () => {
  const waiting = answer({ waiting: true, reason: 'at 17:45' })
  // The next call records meanwhile, or the pane follows none: untouched.
  const b = view({ dir: '/calls/b', phase: 'recording' })
  expect(afterTranscribed(b, '/calls/a', waiting)).toBe(b)
  const none = view({ dir: null, phase: 'idle' })
  expect(afterTranscribed(none, '/calls/a', waiting)).toBe(none)
  // Its own: waits; a final answer only says why (the files end the phase and open the call).
  expect(afterTranscribed(view({}), '/calls/a', waiting)).toMatchObject({ phase: 'waiting-minutes', notesNote: 'at 17:45' })
  expect(afterTranscribed(view({}), '/calls/a', answer({ reason: 'too short' }))).toMatchObject({ phase: 'transcribing', notesNote: 'too short' })
})

test('the pane asks at once on the way in, then once a minute', async () => {
  expect(askAgain('transcribing', 1_000, 900)).toBe(true)
  expect(askAgain('writing-notes', 1_000, 900)).toBe(true) // a run withdrawn for a later note
  expect(askAgain('waiting-minutes', 60_000, 1_000)).toBe(false)
  expect(askAgain('waiting-minutes', 61_001, 1_000)).toBe(true)
})

test('a call its run retitled is found by its stamp, never guessed', async () => {
  const old = '2026-10-23_170000'
  expect(renamedTo([`${old}-weekly-sync`, '2026-10-23_090000-standup'], old)).toBe(`${old}-weekly-sync`)
  expect(renamedTo([old, '2026-10-24_100000-x'], `${old}-untitled`)).toBe(old)
  // Gone, two with that stamp (two calls in one second), or not a call folder: no guess.
  expect(renamedTo(['2026-10-24_100000-x'], old)).toBeNull()
  expect(renamedTo([`${old}-a`, `${old}-b`], old)).toBeNull()
  expect(renamedTo([`${old}0-x`], old)).toBeNull()
  expect(renamedTo(['notes-x'], 'notes')).toBeNull()
})
