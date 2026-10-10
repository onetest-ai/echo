import { expect, test } from 'claude-code/testing'

import type { LiveLine } from '../types'
import { actionsMarkdown, anchors, callFromDir, clock, extractJson, newLines, statusBadge, parseLines, speakerLabel, strings, tailThatFits } from './view'

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

test('extractJson reads a fenced or chatty model reply', async () => {
  expect(extractJson('Sure!\n```json\n{"summary":["a"]}\n```')).toEqual({ summary: ['a'] })
  expect(strings(['a', '', 3, 'b'], 2)).toEqual(['a', '3'])
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
  expect(statusBadge('notes')).toEqual({ label: '✓ notes', color: 'success' })
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
