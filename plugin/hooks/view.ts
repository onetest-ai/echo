// Pure helpers for the Echo mod (no `$`): parsing the app's files and the status line's times.

export function parseLines<T>(text: string): T[] {
  const out: T[] = []
  for (const line of text.split('\n')) {
    if (!line.trim()) continue
    try {
      out.push(JSON.parse(line) as T)
    } catch {
      /* a partial line still being written */
    }
  }
  return out
}

/** Whole new lines after `offset`, and the offset past them (a partial last line waits). */
export function newLines(text: string, offset: number): { lines: string; next: number } {
  if (offset > text.length) return { lines: '', next: text.length } // truncated/rotated
  const fresh = text.slice(offset)
  const cut = fresh.lastIndexOf('\n') + 1
  return { lines: fresh.slice(0, cut), next: offset + cut }
}

export function hm(ms: number): string {
  const d = new Date(ms)
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

export function minutesSince(ms: number, now: number): number {
  return Math.max(0, Math.floor((now - ms) / 60_000))
}
