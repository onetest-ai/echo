// Pure helpers for the Echo mod (no `$`): parsing the CLI's files and laying out text.
import type { CallItem, LiveLine } from '../types'

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

/** The user's own track ("me"; older builds wrote the track name "mic"). */
export function isMe(l: LiveLine): boolean {
  return (l.speaker as string) === 'me' || (l.speaker as string) === 'mic'
}

export function speakerLabel(l: LiveLine): string {
  return isMe(l) ? 'Me' : l.name || 'Remote'
}

export function transcriptText(lines: LiveLine[]): string {
  return lines.map(l => `${speakerLabel(l)}: ${l.text}`).join('\n')
}

/** Rows a line takes when wrapped to `width` columns. */
export function wrappedRows(text: string, width: number): number {
  return Math.max(1, Math.ceil(text.length / Math.max(10, width)))
}

/** The newest lines that fit in `rows` rows of `width` columns, oldest first. */
export function tailThatFits(lines: LiveLine[], rows: number, width: number): LiveLine[] {
  const out: LiveLine[] = []
  let used = 0
  for (let i = lines.length - 1; i >= 0; i--) {
    const l = lines[i]
    if (!l) continue
    const need = wrappedRows(`${speakerLabel(l)} ${l.lang} ${l.text}`, width)
    if (used + need > rows) break
    used += need
    out.unshift(l)
  }
  return out
}

/** Pull the JSON object out of a model reply (it may be fenced or chatty). */
export function extractJson(text: string): unknown {
  const start = text.indexOf('{')
  const end = text.lastIndexOf('}')
  if (start < 0 || end <= start) throw new Error('no JSON object in reply')
  return JSON.parse(text.slice(start, end + 1))
}

export function strings(x: unknown, max: number): string[] {
  return Array.isArray(x) ? x.map(String).filter(s => s.trim() !== '').slice(0, max) : []
}

/** "2026-10-09_183453-lumen-freight-kickoff" → its timestamp label and fallback title. */
export function callFromDir(name: string): { when: string; title: string } {
  const m = /^(\d{4})-(\d{2})-(\d{2})_(\d{2})(\d{2})(\d{2})(?:-(.*))?$/.exec(name)
  if (!m) return { when: '', title: name }
  const month = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'][Number(m[2]) - 1] ?? m[2]
  return { when: `${month} ${m[3]} ${m[4]}:${m[5]}`, title: (m[7] ?? 'call').replace(/-/g, ' ') }
}

/** Status of a call as a short label and a theme colour. */
export function statusBadge(status: CallItem['status']): { label: string; color: string } {
  switch (status) {
    case 'notes':
      return { label: '✓ notes', color: 'success' }
    case 'transcript':
      return { label: '◐ transcript', color: 'suggestion' }
    case 'transcribing':
      return { label: '… transcribing', color: 'warning' }
    case 'recording':
      return { label: '● recording', color: 'claude' }
    default:
      return { label: '○ audio only', color: 'subtle' }
  }
}

/** "mm:ss" (or "h:mm:ss") for a time into the call. */
/** `[mm:ss]` / `[h:mm:ss]` anchors in notes, in order of appearance (unique, at most `max`). */
export function anchors(md: string, max = 12): { label: string; seconds: number }[] {
  const out: { label: string; seconds: number }[] = []
  for (const m of md.matchAll(/\[(?:(\d{1,2}):)?(\d{1,2}):(\d{2})\]/g)) {
    const seconds = Number(m[1] ?? 0) * 3600 + Number(m[2]) * 60 + Number(m[3])
    if (out.some(a => a.seconds === seconds)) continue
    out.push({ label: m[0].slice(1, -1), seconds })
    if (out.length >= max) break
  }
  return out
}

export function clock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds))
  const h = Math.floor(s / 3600)
  const mm = String(Math.floor((s % 3600) / 60)).padStart(2, '0')
  const ss = String(s % 60).padStart(2, '0')
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`
}

export function callRow(c: CallItem): string {
  const mins = c.minutes === null ? '' : ` · ${c.minutes} min`
  const status = c.status === 'notes' ? '✓ notes' : c.status === 'transcript' ? 'transcript' : c.status
  return `${c.when} · ${c.title}${mins} · ${status}`
}

type Action = { task?: string; owner?: string | null; due?: string | null; confidence?: string }

/** actions.json → a Markdown checklist. */
export function actionsMarkdown(json: string): string {
  let items: Action[] = []
  try {
    items = JSON.parse(json) as Action[]
  } catch {
    return '_No action items._'
  }
  if (!Array.isArray(items) || items.length === 0) return '_No action items._'
  return items
    .map(a => `- [ ] **${a.owner ?? 'unassigned'}** — ${a.task ?? ''}${a.due ? ` _(due ${a.due})_` : ''}${a.confidence === 'low' ? ' _(uncertain)_' : ''}`)
    .join('\n')
}
