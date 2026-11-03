import { expect, test } from 'claude-code/testing'

import { hm, minutesSince, newLines, parseLines } from './view'

test('parseLines skips blank and partial lines', async () => {
  const got = parseLines<{ a: number }>('{"a":1}\n\n{"a":2}\n{"a":')
  expect(got.map(x => x.a)).toEqual([1, 2])
})

test('newLines hands out whole lines only and survives truncation', async () => {
  expect(newLines('a\nb\nc', 0)).toEqual({ lines: 'a\nb\n', next: 4 })
  expect(newLines('a\nb\nc', 4)).toEqual({ lines: '', next: 4 })
  expect(newLines('x\n', 10)).toEqual({ lines: '', next: 2 })
})

test('the status line counts whole minutes and shows times as 24 h', async () => {
  const t = new Date(2026, 10, 3, 9, 5).getTime()
  expect(hm(t)).toBe('09:05')
  expect(minutesSince(t, t + 125_000)).toBe(2)
  expect(minutesSince(t, t - 1000)).toBe(0)
})
