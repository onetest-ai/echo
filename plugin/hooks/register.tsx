// Echo mod: the Claude Code client of the echoctl app — a status line above the prompt, nothing more.
//
// The app (tray icon + recorder, started at login) works without Claude Code, and its own window
// shows calls, live transcripts, live notes and minutes. In a Claude Code session this mod only:
// - shows a one-line status above the prompt while the app runs (recording · title · minutes,
//   muted, armed, or the next meeting), with a button that opens the call in Echo's window;
// - starts the app if it is set up but not running;
// - says in a toast when a transcription fails or an arm expires.
//
// It reads files the app writes in its data folder (`echoctl app` → home):
//   .echo/watch.json     app heartbeat (recording? armed? where?)
//   .echo/events.jsonl   app events (muted, errors)
//   .echo/agenda.json    today's meetings (for "next: …")
import { atom, read, update } from 'claude-code'
import type { Register } from 'claude-code'

import type { Band } from '../types'
import { hm, minutesSince, newLines, parseLines } from './view'

// The app's data folder, asked from the CLI at session start (absolute; independent of the
// session's workspace).
let HOME = ''
const stateDir = () => `${HOME}/.echo`
let IS_WINDOWS = false
const ORANGE = '#D97757'

const band = atom({ plugin: 'echo', key: 'band' } as const, {
  watching: false, recording: false, armed: false, next: null, muted: false, dir: null, title: null, startedAt: null,
} as Band)

type Event = { event: string; [k: string]: unknown }

// Module variables reset on reload; that only costs one fresh read of the files.
let eventsOffset: number | null = null
let didAutostart = false

type CliResult = { exitCode: number; stdout: string; stderr: string }

/** Run the Echo CLI through the plugin's launcher (it finds the installed app). */
async function cli($: any, args: string[], timeoutMs = 30_000): Promise<CliResult> {
  const argv = IS_WINDOWS
    ? ['cmd', '/c', `${$.plugin.root}\\scripts\\echoctl.cmd`, ...args]
    : [`${$.plugin.root}/scripts/echoctl`, ...args]
  return $.process.run(argv, { timeoutMs })
}

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    IS_WINDOWS = /^[A-Za-z]:[\\/]/.test(e.cwd)
    // A run Echo started for one call (`echoctl notes`): nothing to show.
    if (await $.env.get('ECHOCTL_HEADLESS')) return next(e)
    void connect($)
    return next(e)
  })

  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const b = await read($, band)
    if (!b.watching || e.props.hasSurvey) return next(e)
    const { Box, Button, Text } = $.ui.resolve(e)
    const status = b.recording
      ? `● Recording${b.title ? ` · ${b.title}` : ''}${b.startedAt ? ` · ${minutesSince(b.startedAt, Date.now())} min` : ''}${b.muted ? ' · 🔇 muted — your mic is not transcribed' : ''}`
      : b.armed
        ? '◌ Armed — recording starts with the next Teams call'
        : `Echo${b.next ? ` · next: ${b.next}` : ' · watching'}`
    return (
      <Box gap={1}>
        <Button key="open-echo" label="Echo" hotkey="e" variant={b.recording ? 'primary' : 'secondary'} onPress={() => openWindow($)} />
        <Text color={b.recording ? ORANGE : undefined} dimColor={!b.recording}>{status}</Text>
      </Box>
    )
  })
}

/** Echo's window, on the call being recorded (else as it was). */
async function openWindow($: any): Promise<void> {
  const b = await read($, band)
  const name = b.recording && b.dir ? b.dir.split(/[\\/]/).pop() : null
  const r = await cli($, name ? ['app', 'open', '--call', name] : ['app', 'open']).catch(() => null)
  if (!r || r.exitCode !== 0) $.ui.toast(`echo: couldn't open Echo's window${r ? ` — ${String(r.stderr).trim().split('\n').pop()?.replace(/^Error: /, '') ?? ''}` : ''}`)
}

/** Find the app's data folder and start the app if it is set up but not running. */
async function connect($: any): Promise<void> {
  let info: { home?: string; home_source?: string; running?: number | null } = {}
  try {
    const r = await cli($, ['app'])
    if (r.exitCode !== 0) return // app not installed: the plugin stays quiet until setup
    info = JSON.parse(r.stdout)
  } catch {
    return
  }
  if (!info.home) return
  HOME = info.home.replace(/\\/g, '/')
  if (info.home_source !== 'default' && !info.running && !didAutostart) {
    didAutostart = true
    await cli($, ['app', 'start'], 20_000).catch(() => null)
  }
  $.clock.every(1000, () => void tick($))
}

type WatchState = { recording: string | null; armed: boolean }

/** The app's state, or null when it isn't running. */
async function watcherState($: any): Promise<WatchState | null> {
  try {
    const w = JSON.parse(await $.fs.read(`${stateDir()}/watch.json`))
    if (!(Date.now() - Date.parse(w.heartbeat) < 5000)) return null
    return { recording: w.recording ?? null, armed: Boolean(w.armed) }
  } catch {
    return null
  }
}

/** "18:30 Weekly check-in" for the next meeting on today's agenda, if any. */
async function nextMeeting($: any): Promise<string | null> {
  try {
    const agenda = JSON.parse(await $.fs.read(`${stateDir()}/agenda.json`)) as { subject?: string; start?: string }[]
    const now = Date.now()
    const upcoming = agenda
      .map(m => ({ subject: m.subject ?? 'Meeting', at: Date.parse(m.start ?? '') }))
      .filter(m => !Number.isNaN(m.at) && m.at > now - 4 * 60_000 && new Date(m.at).toDateString() === new Date(now).toDateString())
      .sort((a, b) => a.at - b.at)[0]
    return upcoming ? `${hm(upcoming.at)} ${upcoming.subject}` : null
  } catch {
    return null
  }
}

/** The recording's title and start, from its `meta.json`. */
async function recordingMeta($: any, dir: string): Promise<{ title: string | null; startedAt: number | null }> {
  try {
    const m = JSON.parse(await $.fs.read(`${dir}/meta.json`)) as { title?: string; started_at?: string }
    const at = Date.parse(m.started_at ?? '')
    return { title: m.title || null, startedAt: Number.isNaN(at) ? null : at }
  } catch {
    return { title: null, startedAt: null }
  }
}

async function tick($: any): Promise<void> {
  if (!HOME) return
  await $.fs.write(`${stateDir()}/mod.json`, JSON.stringify({ heartbeat: new Date().toISOString() }))
  await relayEvents($)
  const w = await watcherState($)
  const nextUp = w ? await nextMeeting($) : null
  const cur = await read($, band)
  const dir = w?.recording ?? null
  const meta = dir && dir !== cur.dir ? await recordingMeta($, dir) : { title: cur.title, startedAt: cur.startedAt }
  await update($, band, prev => {
    const fresh: Band = {
      watching: w !== null, recording: dir !== null, armed: Boolean(w?.armed), next: nextUp,
      muted: dir !== null && prev.muted, dir, title: dir ? meta.title : null, startedAt: dir ? meta.startedAt : null,
    }
    return JSON.stringify(prev) === JSON.stringify(fresh) ? prev : fresh
  })
}

/** App events → the band and toasts. Starts at the end: never replays. */
async function relayEvents($: any): Promise<void> {
  const text: string = await $.fs.read(`${stateDir()}/events.jsonl`).catch(() => '')
  if (eventsOffset === null) {
    eventsOffset = text.length
    return
  }
  const { lines, next } = newLines(text, eventsOffset)
  eventsOffset = next
  for (const ev of parseLines<Event>(lines)) {
    switch (ev.event) {
      case 'transcribed':
        if (ev.ok !== true) $.ui.toast(`echo: transcription failed — ${String(ev.error ?? '')}`)
        break
      case 'error':
        $.ui.toast(`echo: ${String(ev.message)}`)
        break
      case 'mic_muted':
      case 'mic_unmuted':
        await update($, band, (cur): Band => ({ ...cur, muted: ev.event === 'mic_muted' }))
        break
      case 'arm_expired':
        $.ui.toast('echo: the arm expired — no call started within 15 min')
        break
    }
  }
}
