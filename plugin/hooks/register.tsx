// Echo mod: the Claude Code client of the echoctl app.
//
// The app (tray icon + recorder, started at login) works without Claude Code. This mod adds:
// - Band above the prompt while the app runs: [ Live ] plus a one-line status.
// - Pane `echo-live`: live transcript on the left, rolling summary / highlights / action
//   items on the right (stacked when narrow), and a browser of past calls.
// - Notes: when a call is transcribed, the session claims it (`echoctl claim`, so one of
//   several open sessions does it) and runs /echo:process-call by itself; calls
//   transcribed while no session was open are picked up when one opens (`echoctl pending`).
// - It starts the app if it is set up but not running.
//
// It reads files the app writes in its data folder (`echoctl app` → home):
//   .echo/watch.json     app heartbeat (recording? armed? where?)
//   .echo/events.jsonl   app events
//   .echo/agenda.json    today's meetings (for "next: …")
//   calls/<dir>/live.jsonl    live transcript lines
import { atom, read, update } from 'claude-code'
import type { Register } from 'claude-code'

import type { ActionItem, Band, Browse, CallItem, CallMeta, ChatLine, DocTab, Insight, LiveLine, LiveView } from '../types'
import { anchors, callFromDir, clock, extractJson, hm, isMe, minutesSince, newLines, parseLines, speakerLabel, statusBadge, strings, tailThatFits, transcriptText } from './view'

const PANE = 'echo-live'
// The app's data folder, asked from the CLI at session start (absolute; independent of the
// session's workspace).
let HOME = ''
const stateDir = () => `${HOME}/.echo`
const callsDir = () => `${HOME}/calls`
let IS_WINDOWS = false
// This session's folder: the app may limit automatic notes to sessions in one folder.
let SESSION_CWD = ''
const CATCH_UP_MAX = 3
const SUMMARY_EVERY_MS = 90_000
const SUMMARY_MIN_NEW_CHARS = 400
const SUMMARY_WINDOW_CHARS = 16_000
const MAX_LINES = 400
const TWO_COLUMNS_FROM = 100
const ORANGE = '#D97757'

const live = atom({ plugin: 'echo', key: 'live' } as const, {
  dir: null, title: null, startedAt: null, phase: 'idle', lines: [], notesRequested: false, chat: [],
} as LiveView)
const insight = atom({ plugin: 'echo', key: 'insight' } as const, {
  summary: [], highlights: [], actions: [], coveredChars: 0, updatedAt: null, answer: null, asking: null,
} as Insight)
const browse = atom({ plugin: 'echo', key: 'browse' } as const, {
  mode: 'calls', calls: [], selected: null, tab: 'notes', meta: null, doc: '', transcript: [], actions: null,
  confirmDelete: false, generating: null, hasNotes: false, from: null, chat: [],
} as Browse)
const band = atom({ plugin: 'echo', key: 'band' } as const, {
  watching: false, recording: false, armed: false, next: null, muted: false,
} as Band)

type Event = { event: string; [k: string]: unknown }

// Module variables reset on reload; that only costs one fresh read of the files.
let eventsOffset: number | null = null
let summarizing = false
let lastSummaryAt = 0
let didAutostart = false
/** Model for the live summary and quick questions (`echoctl config --live-model`). */
let LIVE_MODEL = 'haiku'
let lastCallsLoad = 0

type CliResult = { exitCode: number; stdout: string; stderr: string }

/** Run the Echo CLI through the plugin's launcher (it finds the installed app). */
async function cli($: any, args: string[], timeoutMs = 30_000): Promise<CliResult> {
  const argv = IS_WINDOWS
    ? ['cmd', '/c', `${$.plugin.root}\\scripts\\echoctl.cmd`, ...args]
    : [`${$.plugin.root}/scripts/echoctl`, ...args]
  return $.process.run(argv, { timeoutMs })
}

/** Take a call for notes and run process-call on it, unless another session already has it. */
async function processCall($: any, dir: string): Promise<boolean> {
  const r = await cli($, ['claim', dir, '--for', SESSION_CWD]).catch(() => null)
  if (!r || r.exitCode !== 0) return false
  void $.prompt.submit({ text: `/echo:process-call ${dir}` })
  return true
}

async function openPane($: any): Promise<void> {
  await $.ui.open({ id: PANE, title: 'Echo · live', columns: 400, rows: 40 })
}

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    IS_WINDOWS = /^[A-Za-z]:[\\/]/.test(e.cwd)
    SESSION_CWD = e.cwd
    // A run Echo started for one call (`echoctl notes`): no pane, no catch-up, no claims.
    if (await $.env.get('ECHOCTL_HEADLESS')) return next(e)
    // `/echo:live` is a skill of the plugin (mods can't register names with a colon); the
    // command.run hook below answers it before the model sees it, so it costs no context.
    void connect($)
    return next(e)
  })

  // Typed at the prompt: opens the pane and leaves nothing in the conversation.
  for (const name of ['echo:live']) {
    on('command.run', { command: name }, async $ => {
      await openPane($)
      return {}
    }).catch(() => ({}))
  }

  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const b = await read($, band)
    if (!b.watching || e.props.hasSurvey) return next(e)
    const v = await read($, live)
    const { Box, Button, Text } = $.ui.resolve(e)
    const status = b.recording
      ? `● Recording${v.title ? ` · ${v.title}` : ''}${v.startedAt ? ` · ${minutesSince(v.startedAt, Date.now())} min` : ''}${b.muted ? ' · 🔇 muted — your mic is not transcribed' : ''}`
      : b.armed
        ? '◌ Armed — recording starts with the next Teams call'
        : `Echo${b.next ? ` · next: ${b.next}` : ' · watching'}`
    return (
      <Box gap={1}>
        <Button key="open-live" label="Live" hotkey="l" variant={b.recording ? 'primary' : 'secondary'} onPress={() => openPane($)} />
        <Text color={b.recording ? ORANGE : undefined} dimColor={!b.recording}>{status}</Text>
      </Box>
    )
  })

  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    const { Box, Button, Markdown, Text } = $.ui.resolve(e)
    const v = await read($, live)
    const ins = await read($, insight)
    const br = await read($, browse)
    const bandNow = await read($, band)
    const width = Math.max(40, e.props.bodyColumns ?? 80)
    const rows = Math.max(10, e.props.scroll?.bodyRows ?? 30)
    const twoColumns = width >= TWO_COLUMNS_FROM

    const switcher = (
      <Box gap={1}>
        <Button key="mode-live" label="Live" hotkey="v" variant={br.mode === 'live' ? 'primary' : 'secondary'}
          onPress={() => update($, browse, (b): Browse => ({ ...b, mode: 'live' }))} />
        <Button key="mode-calls" label="Calls" hotkey="c" variant={br.mode === 'calls' ? 'primary' : 'secondary'}
          onPress={() => update($, browse, (b): Browse => ({ ...b, mode: 'calls' }))} />
      </Box>
    )

    if (br.mode === 'calls') {
      const listWidth = twoColumns ? Math.max(38, Math.floor(width * 0.38)) : width
      const docWidth = twoColumns ? width - listWidth - 2 : width
      const list = (
        <Box flexDirection="column" width={listWidth} borderStyle="round" borderColor="subtle" paddingX={1}>
          <Text bold color="claude">Calls</Text>
          {br.calls.length === 0 && <Text dimColor>No recorded calls yet.</Text>}
          {br.calls.slice(0, Math.max(3, Math.floor((rows - 6) / 2))).map(c => {
            const isSel = c.dir === br.selected
            const badge = statusBadge(c.status)
            const inner = listWidth - 6 // border + padding + marker
            return (
              <Box key={`row:${c.dir}`} flexDirection="column" marginBottom={0}>
                <Button key={`call:${c.dir}`} plain onPress={() => selectCall($, c.dir, br.tab)}>
                  <Text color={isSel ? 'claude' : 'subtle'} bold>{isSel ? '▸ ' : '  '}</Text>
                  <Text bold color={isSel ? 'claude' : undefined}>{fitText(c.title, inner)}</Text>
                </Button>
                <Text wrap="truncate">
                  {'  '}
                  <Text dimColor>{c.when}{c.minutes !== null ? ` · ${c.minutes} min` : ''} · </Text>
                  <Text color={badge.color}>{badge.label}</Text>
                </Text>
              </Box>
            )
          })}
        </Box>
      )

      const tab = (id: DocTab, label: string, hotkey: string) => (
        <Button key={`tab:${id}`} label={label} hotkey={hotkey} variant={br.tab === id ? 'primary' : 'secondary'}
          onPress={() => (br.selected ? selectCall($, br.selected, id) : undefined)} />
      )

      const body =
        br.tab === 'chat' ? (
          <Box flexDirection="column">
            {br.chat.length === 0 && <Text dimColor>No meeting chat for this call.</Text>}
            {br.chat.map((m, i) => (
              <Box key={`c-${i}`} flexDirection="column" marginBottom={1}>
                <Text>
                  <Text dimColor>{m.t === null ? '' : m.t < 0 ? 'before ' : `${clock(m.t)} `}</Text>
                  <Text bold color="suggestion">{m.from}</Text>
                </Text>
                <Text wrap="wrap">{m.text}</Text>
                {m.files.length > 0 && <Text color="claude">📎 {m.files.join(', ')}</Text>}
              </Box>
            ))}
          </Box>
        ) : br.tab === 'transcript' ? (
          <Box flexDirection="column">
            {br.transcript.length === 0 && <Text dimColor>Not transcribed yet.</Text>}
            {br.from !== null && (
              <Box gap={1} marginBottom={1}>
                <Text color="claude">From {clock(br.from)}</Text>
                <Button key="from-all" label="Show all" hotkey="s"
                  onPress={() => update($, browse, (b): Browse => ({ ...b, from: null }))} />
              </Box>
            )}
            {br.transcript.filter(l => br.from === null || l.end >= br.from).slice(0, 600).map((l, i) => (
              <Text key={`t-${i}`} wrap="wrap">
                <Text dimColor>{clock(l.start)} </Text>
                <Text bold color={isMe(l) ? 'success' : 'suggestion'}>{speakerLabel(l)}</Text>
                <Text> {l.text}</Text>
              </Text>
            ))}
          </Box>
        ) : br.tab === 'actions' ? (
          <Box flexDirection="column">
            {br.actions === null && (
              <Box flexDirection="column" gap={1}>
                <Text dimColor>No action items yet.</Text>
                {br.generating === 'actions' ? (
                  <Text color="warning">… Claude Code is writing the notes and action items (a minute or two)</Text>
                ) : br.transcript.length > 0 ? (
                  <Box>
                    <Button key="gen-actions" label="Write notes and actions" hotkey="p" variant="primary"
                      onPress={() => generate($, br.selected!, 'actions')} />
                  </Box>
                ) : null}
              </Box>
            )}
            {br.actions !== null && br.actions.length === 0 && <Text dimColor>No action items in this call.</Text>}
            {(br.actions ?? []).map((x, i) => (
              <Box key={`a-${i}`} flexDirection="column" marginBottom={1}>
                <Box gap={1}>
                  <Button key={`done-${i}`} label={x.status === 'done' ? 'Undo' : 'Done'}
                    onPress={() => toggleAction($, br.selected!, i)} />
                  {x.t !== null && (
                    <Button key={`at-${i}`} label={clock(x.t)} plain
                      onPress={() => update($, browse, (b): Browse => ({ ...b, tab: 'transcript', from: Math.max(0, x.t! - 15) }))} />
                  )}
                </Box>
                <Text wrap="wrap" strikethrough={x.status === 'done'} dimColor={x.status === 'done'}>
                  <Text color={x.confidence === 'low' ? 'subtle' : 'claude'}>{x.status === 'done' ? '☑ ' : '☐ '}</Text>
                  <Text bold color={x.owner === 'Me' ? 'success' : x.owner ? 'suggestion' : 'subtle'}>{x.owner ?? 'unassigned'}</Text>
                  <Text dimColor={x.confidence === 'low'}> — {x.task}</Text>
                </Text>
                {(x.due || x.confidence === 'low') && (
                  <Text>
                    {'    '}
                    {x.due && <Text color="warning">due {x.due}</Text>}
                    {x.due && x.confidence === 'low' && <Text dimColor> · </Text>}
                    {x.confidence === 'low' && <Text dimColor italic>uncertain</Text>}
                  </Text>
                )}
              </Box>
            ))}
          </Box>
        ) : !br.hasNotes || br.generating === 'notes' ? (
          <Box flexDirection="column" gap={1}>
            <Text dimColor>No notes for this call yet.</Text>
            {br.generating === 'notes' ? (
              <Text color="warning">… Claude Code is writing the notes and action items (a minute or two)</Text>
            ) : br.transcript.length > 0 ? (
              <Box>
                <Button key="gen-notes" label="Write notes and actions" hotkey="g" variant="primary"
                  onPress={() => generate($, br.selected!, 'notes')} />
              </Box>
            ) : (
              <Text dimColor>Not transcribed yet.</Text>
            )}
          </Box>
        ) : (
          <Box flexDirection="column">
            <Markdown key={`doc:${br.selected}`} text={br.doc} />
            {anchors(br.doc).length > 0 && (
              <Box gap={1} marginTop={1} flexWrap="wrap">
                <Text dimColor>Jump to:</Text>
                {anchors(br.doc).map(a => (
                  <Button key={`jump-${a.seconds}`} label={a.label} plain
                    onPress={() => update($, browse, (b): Browse => ({ ...b, tab: 'transcript', from: Math.max(0, a.seconds - 15) }))} />
                ))}
              </Box>
            )}
          </Box>
        )

      const doc = (
        <Box flexDirection="column" width={docWidth}>
          {br.selected && br.meta ? (
            <>
              <Box flexDirection="column" borderStyle="round" borderColor="claude" paddingX={1}>
                <Text bold color="claude" wrap="truncate">{br.meta.title}</Text>
                <Text dimColor wrap="truncate">
                  {br.meta.when}
                  {br.meta.minutes !== null ? ` · ${br.meta.minutes} min` : ''}
                  {br.meta.people.length > 0 ? ` · ${br.meta.people.join(', ')}` : ''}
                </Text>
              </Box>
              <Box gap={1} marginTop={1}>
                {tab('notes', 'Notes', 'n')}
                {tab('transcript', 'Transcript', 't')}
                {tab('actions', `Actions${br.actions?.length ? ` (${br.actions.length})` : ''}`, 'a')}
                {br.chat.length > 0 && tab('chat', `Chat (${br.chat.length})`, 'h')}
                <Text dimColor>│</Text>
                <Button key="delete" label="Delete" hotkey="d"
                  onPress={() => update($, browse, (b): Browse => ({ ...b, confirmDelete: true }))} />
              </Box>
              {br.confirmDelete && (
                <Box marginTop={1} gap={1} borderStyle="round" borderColor="error" paddingX={1}>
                  <Text color="error" bold>Delete “{fitText(br.meta.title, 40)}”?</Text>
                  <Text dimColor>It moves to the trash in the data folder (30 days).</Text>
                  <Button key="delete-yes" label="Yes, delete" hotkey="y" variant="primary" onPress={() => deleteCall($, br.selected!)} />
                  <Button key="delete-no" label="Cancel" hotkey="x"
                    onPress={() => update($, browse, (b): Browse => ({ ...b, confirmDelete: false }))} />
                </Box>
              )}
              <Box marginTop={1} flexDirection="column">{body}</Box>
            </>
          ) : (
            <Box borderStyle="round" borderColor="subtle" paddingX={1}>
              <Text dimColor>Select a call to read its notes, transcript and action items.</Text>
            </Box>
          )}
        </Box>
      )
      return (
        <Box flexDirection="column" width={width}>
          {switcher}
          <Box marginTop={1} flexDirection={twoColumns ? 'row' : 'column'} gap={2}>
            {list}
            {doc}
          </Box>
        </Box>
      )
    }

    const header =
      v.phase === 'recording'
        ? `● Recording${v.title ? ` · ${v.title}` : ''}${v.startedAt ? ` · since ${hm(v.startedAt)}` : ''}${bandNow.muted ? ' · 🔇 muted' : ''}`
        : v.phase === 'transcribing'
          ? `Transcribing ${v.title ?? 'the call'}… (on this Mac, ~¼ of the call's length)`
          : v.phase === 'writing-notes'
            ? `Writing notes… they appear in the session${v.notesRequested ? '' : ' when it is free'}`
            : v.phase === 'done'
              ? `Done${v.title ? ` · ${v.title}` : ''} — open Calls to read the notes`
              : 'Not recording — Record from the Echo menu-bar icon or /echo:record'

    const section = (title: string, items: string[]) =>
      items.length === 0 ? null : (
        <Box flexDirection="column" marginBottom={1}>
          <Text bold>{title}</Text>
          {items.map((t, i) => (
            <Text key={`${title}-${i}`} wrap="wrap">• {t}</Text>
          ))}
        </Box>
      )

    const quick = v.phase === 'recording' && v.lines.length > 0 ? (
      <Box flexDirection="column" marginBottom={1}>
        <Box gap={1}>
          <Button key="q-missed" label="What did I miss?" hotkey="w" onPress={() => quickAction($, 'missed')} />
          <Button key="q-question" label="Suggest a question" hotkey="q" onPress={() => quickAction($, 'question')} />
        </Box>
        {ins.asking !== null && <Text color="warning">… thinking</Text>}
        {ins.answer !== null && ins.asking === null && (
          <Box flexDirection="column" borderStyle="round" borderColor="claude" paddingX={1} marginTop={1}>
            <Text bold color="claude">{ins.answer.title}</Text>
            <Text wrap="wrap">{ins.answer.text}</Text>
          </Box>
        )}
      </Box>
    ) : null

    const notes = (
      <Box flexDirection="column">
        {quick}
        {section('Summary', ins.summary)}
        {section('Highlights', ins.highlights)}
        {section('Action items', ins.actions)}
        {v.chat.length > 0 && (
          <Box flexDirection="column" marginBottom={1}>
            <Text bold>Chat</Text>
            {v.chat.slice(-6).map((m, i) => (
              <Text key={`chat-${i}`} wrap="wrap">
                <Text color="suggestion" bold>{m.from}</Text>
                <Text> {m.text}{m.files.length ? ` 📎 ${m.files.join(', ')}` : ''}</Text>
              </Text>
            ))}
          </Box>
        )}
        {ins.updatedAt === null ? (
          <Text dimColor>{v.phase === 'recording' ? 'Notes appear after a couple of minutes of talk.' : ''}</Text>
        ) : (
          <Text dimColor>updated {hm(ins.updatedAt)}</Text>
        )}
      </Box>
    )

    const notesRows = 3 + ins.summary.length + ins.highlights.length + ins.actions.length + 4
    const transcriptWidth = twoColumns ? Math.floor(width * 0.58) - 2 : width
    const transcriptRows = twoColumns ? rows - 5 : Math.max(4, rows - 5 - notesRows)
    const shown = tailThatFits(v.lines, transcriptRows, transcriptWidth)

    const transcript = (
      <Box flexDirection="column">
        <Text bold>Transcript</Text>
        {shown.length === 0 && <Text dimColor>{v.phase === 'recording' ? 'Listening…' : '—'}</Text>}
        {shown.map((l, i) => (
          <Text key={`l-${l.start}-${i}`} wrap="wrap">
            <Text bold color={isMe(l) ? 'green' : 'cyan'}>{speakerLabel(l)}</Text>
            <Text dimColor> {l.lang} </Text>
            {l.text}
          </Text>
        ))}
      </Box>
    )

    return (
      <Box flexDirection="column" width={width}>
        {switcher}
        <Text bold color={v.phase === 'recording' ? ORANGE : undefined} wrap="truncate">{header}</Text>
        <Box marginTop={1} flexDirection={twoColumns ? 'row' : 'column'} gap={2}>
          {twoColumns ? (
            <>
              <Box width={transcriptWidth} flexDirection="column">{transcript}</Box>
              <Box flexGrow={1} flexDirection="column">{notes}</Box>
            </>
          ) : (
            <>
              {notes}
              {transcript}
            </>
          )}
        </Box>
      </Box>
    )
  })
}

/** Find the app's data folder, start the app if it is set up, pick up calls without notes. */
async function connect($: any): Promise<void> {
  let info: { home?: string; home_source?: string; running?: number | null; live_model?: string } = {}
  try {
    const r = await cli($, ['app'])
    if (r.exitCode !== 0) return // app not installed: the plugin stays quiet until setup
    info = JSON.parse(r.stdout)
  } catch {
    return
  }
  if (!info.home) return
  if (info.live_model) LIVE_MODEL = info.live_model
  HOME = info.home.replace(/\\/g, '/')
  const setUp = info.home_source !== 'default'
  if (setUp && !info.running && !didAutostart) {
    didAutostart = true
    await cli($, ['app', 'start'], 20_000).catch(() => null)
  }
  $.clock.every(1000, () => void tick($))
  if (!setUp) return
  // Calls transcribed while no session was open (the app writes no notes by itself).
  try {
    const r = await cli($, ['pending', '--for', SESSION_CWD])
    const pending = r.exitCode === 0 ? (JSON.parse(r.stdout) as { dir: string }[]) : []
    let n = 0
    for (const p of pending.slice(-CATCH_UP_MAX)) if (await processCall($, p.dir)) n++
    if (n > 0) $.ui.toast(`echo: writing notes for ${n} call${n > 1 ? 's' : ''} recorded earlier`)
  } catch {
    /* best effort */
  }
}

/** The meeting chat file, tolerant of a missing or half-written file. */
function readChat(text: string): ChatLine[] {
  try {
    const a = JSON.parse(text) as Partial<ChatLine>[]
    return Array.isArray(a)
      ? a.map(m => ({ t: typeof m.t === 'number' ? m.t : null, from: String(m.from ?? 'someone'), text: String(m.text ?? ''), files: Array.isArray(m.files) ? m.files.map(String) : [] }))
      : []
  } catch {
    return []
  }
}

function fitText(s: string, width: number): string {
  return s.length <= width ? s : `${s.slice(0, Math.max(1, width - 1))}…`
}

const DOC_LIMIT = 90_000 // a tree draws up to 100k characters of text

/** Load one call (header, notes, transcript lines, action items) into the browser. */
async function selectCall($: any, dir: string, tab: DocTab): Promise<void> {
  const read1 = (f: string) => $.fs.read(`${callsDir()}/${dir}/${f}`).catch(() => null) as Promise<string | null>
  const parse = <T,>(text: string | null, fallback: T): T => {
    try {
      return text === null ? fallback : (JSON.parse(text) as T)
    } catch {
      return fallback
    }
  }
  const metaJson = parse<Record<string, unknown>>(await read1('meta.json'), {})
  const meeting = parse<{ subject?: string; organizer?: { name?: string }; attendees?: { name?: string }[] }>(await read1('meeting.json'), {})
  const base = callFromDir(dir)
  const a = Date.parse(String(metaJson.started_at ?? ''))
  const b = Date.parse(String(metaJson.ended_at ?? ''))
  const people = [meeting.organizer?.name, ...(meeting.attendees ?? []).map(p => p.name)]
    .filter((n): n is string => Boolean(n))
    .filter((n, i, all) => all.indexOf(n) === i)
    .slice(0, 6)
  const meta: CallMeta = {
    title: String(metaJson.title ?? meeting.subject ?? base.title),
    when: base.when,
    minutes: Number.isNaN(a) || Number.isNaN(b) ? null : Math.max(1, Math.round((b - a) / 60_000)),
    people,
  }

  const notes = await read1('notes.md')
  const hasTranscript = (await $.fs.exists(`${callsDir()}/${dir}/transcript.md`)) as boolean
  let doc = notes ?? (hasTranscript
    ? '_No notes for this call (too short, or still being written). See **Transcript**._'
    : '_Not transcribed yet._')
  if (doc.length > DOC_LIMIT) doc = `${doc.slice(0, DOC_LIMIT)}\n\n_…cut here; the full file is in calls/${dir}._`

  const segs = parse<LiveLine[]>(await read1('segments.json'), [])
  const transcript = (segs.length > 0 ? segs : parseLines<LiveLine>((await read1('live.jsonl')) ?? '')).slice(0, 2000)
  const chat = readChat((await read1('chat.json')) ?? '[]')
  const rawActions = await read1('actions.json')
  const actions = rawActions === null
    ? null
    : parse<Partial<ActionItem>[]>(rawActions, []).map(x => ({
        task: String(x.task ?? ''),
        owner: x.owner ?? null,
        due: x.due ?? null,
        confidence: x.confidence === 'low' ? 'low' as const : 'high' as const,
        t: typeof x.t === 'number' ? x.t : null,
        status: x.status === 'done' ? 'done' as const : 'todo' as const,
      }))

  await update($, browse, (cur): Browse => ({
    ...cur, mode: 'calls', selected: dir, tab, meta, doc, transcript, actions, confirmDelete: false, hasNotes: notes !== null,
    from: null, chat,
  }))
}

/** Notes and action items for a call: the app runs the same process-call skill with Claude
 *  Code (`echoctl notes`, headless, the model set under menu → Claude Code → Notes model), so
 *  this session's own turn isn't used. The tick follows `notes-run.json` until it finishes. */
async function generate($: any, dir: string, what: 'notes' | 'actions'): Promise<void> {
  await update($, browse, (b): Browse => ({ ...b, generating: what }))
  const r = await cli($, ['notes', dir], 25 * 60_000).catch((err: unknown) => ({ exitCode: 1, stdout: '', stderr: String(err) }))
  if (r.exitCode !== 0) {
    $.ui.toast(`echo: couldn't write notes — ${String(r.stderr).trim().split('\n').pop()?.replace(/^Error: /, '') ?? 'error'}`)
    await update($, browse, (b): Browse => ({ ...b, generating: null }))
  }
}

/** While the app writes notes for the selected call: done → show them; failed → say why. */
async function followNotesRun($: any): Promise<void> {
  const b = await read($, browse)
  if (!b.generating || !b.selected) return
  let run: { state?: string; message?: string } = {}
  try {
    run = JSON.parse(String(await $.fs.read(`${callsDir()}/${b.selected}/notes-run.json`)))
  } catch {
    return // not started yet
  }
  if (run.state === 'running') return
  const tab = b.generating
  await update($, browse, (x): Browse => ({ ...x, generating: null }))
  if (run.state === 'done') $.ui.toast('Notes and action items written.')
  else $.ui.toast(`echo: ${run.state === 'skipped' ? 'no notes' : "couldn't write notes"} — ${run.message ?? run.state}`, { timeoutMs: 10_000 })
  await selectCall($, b.selected, tab)
  lastCallsLoad = 0
}

/** Delete the selected call through the CLI (mods can't remove files): it goes to the trash. */
async function deleteCall($: any, dir: string): Promise<void> {
  try {
    const r = await cli($, ['delete', dir])
    if (r.exitCode !== 0) {
      $.ui.toast(`echo: couldn't delete — ${String(r.stderr).trim().split('\n').pop() ?? 'error'}`)
      await update($, browse, (b): Browse => ({ ...b, confirmDelete: false }))
      return
    }
    $.ui.toast(`Deleted. Restore with: echoctl restore ${dir}`, { timeoutMs: 8000 })
  } catch (err) {
    $.ui.toast(`echo: couldn't delete — ${String(err)}`)
  }
  await update($, browse, (b): Browse => ({
    ...b, selected: null, meta: null, doc: '', transcript: [], actions: null, confirmDelete: false,
    calls: b.calls.filter(c => c.dir !== dir),
  }))
  lastCallsLoad = 0
}

/** Refresh the call list from calls/ (newest first). */
async function loadCalls($: any): Promise<void> {
  const entries = await $.fs.list(callsDir()).catch(() => [])
  const dirs = (entries as { name: string; kind: string }[])
    .filter(x => x.kind === 'dir' && /^\d{4}-\d{2}-\d{2}_\d{6}/.test(x.name))
    .map(x => x.name)
    .sort()
    .reverse()
    .slice(0, 40)
  const w = await watcherState($)
  const calls: CallItem[] = []
  for (const dir of dirs) {
    const base = callFromDir(dir)
    let title = base.title
    let minutes: number | null = null
    try {
      const meta = JSON.parse(await $.fs.read(`${callsDir()}/${dir}/meta.json`))
      if (meta.title) title = String(meta.title)
      const a = Date.parse(meta.started_at)
      const b = Date.parse(meta.ended_at)
      if (!Number.isNaN(a) && !Number.isNaN(b)) minutes = Math.max(1, Math.round((b - a) / 60_000))
    } catch {
      /* meta.json missing: keep the folder name */
    }
    const has = (f: string) => $.fs.exists(`${callsDir()}/${dir}/${f}`)
    const status: CallItem['status'] = w?.recording?.endsWith(dir)
      ? 'recording'
      : (await has('notes.md'))
        ? 'notes'
        : (await has('transcript.md'))
          ? 'transcript'
          : (await has('.transcribing'))
            ? 'transcribing'
            : 'audio only'
    calls.push({ dir, when: base.when, title, minutes, status })
  }
  await update($, browse, (b): Browse => (JSON.stringify(b.calls) === JSON.stringify(calls) ? b : { ...b, calls }))
}

type WatchState = { recording: string | null; armed: boolean }

/** The live watcher's state, or null when no watcher is running. */
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

async function tick($: any): Promise<void> {
  if (!HOME) return
  await $.fs.write(`${stateDir()}/mod.json`, JSON.stringify({ heartbeat: new Date().toISOString() }))
  await relayEvents($)

  const w = await watcherState($)
  const nextUp = w ? await nextMeeting($) : null
  await update($, band, cur => {
    const fresh: Band = {
      watching: w !== null, recording: Boolean(w?.recording), armed: Boolean(w?.armed), next: nextUp,
      muted: Boolean(w?.recording) && cur.muted,
    }
    return JSON.stringify(cur) === JSON.stringify(fresh) ? cur : fresh
  })

  if (Date.now() - lastCallsLoad > 5000) {
    lastCallsLoad = Date.now()
    await loadCalls($)
  }
  await followNotesRun($)

  const v = await read($, live)
  if (v.dir && v.phase !== 'recording' && v.phase !== 'done') await refreshPhase($, v)
  if (v.phase !== 'recording' || !v.dir) return
  const text = await $.fs.read(`${v.dir}/live.jsonl`).catch(() => '')
  const lines = parseLines<LiveLine>(text).slice(-MAX_LINES)
  if (lines.length !== v.lines.length) await update($, live, cur => ({ ...cur, lines }))
  const chat = readChat(await $.fs.read(`${v.dir}/chat.json`).catch(() => '[]'))
  if (chat.length !== v.chat.length) await update($, live, (cur): LiveView => ({ ...cur, chat }))
  await maybeSummarize($, lines)
}

/** After a call: the files say how far processing got (survives reloads and lost turns). */
async function refreshPhase($: any, v: LiveView): Promise<void> {
  const has = (f: string) => $.fs.exists(`${v.dir}/${f}`)
  let phase: LiveView['phase'] = v.phase
  if (await has('notes.md')) phase = 'done'
  else if (await has('transcript.md')) {
    // Calls under 3 minutes get a transcript only (the notes step skips them).
    const meta = JSON.parse(await $.fs.read(`${v.dir}/meta.json`).catch(() => '{}'))
    const minutes = (Date.parse(meta.ended_at) - Date.parse(meta.started_at)) / 60_000
    phase = minutes < 3 ? 'done' : 'writing-notes'
  } else if (await has('.transcribing')) phase = 'transcribing'
  if (phase !== v.phase) {
    await update($, live, (cur): LiveView => ({ ...cur, phase }))
    // Notes are ready: show them in the call browser.
    if (phase === 'done' && v.dir) await selectCall($, v.dir.split('/').pop() ?? v.dir, 'notes')
  }
}

/** Watcher events → pane state and session turns. Starts at the end: never replays. */
async function relayEvents($: any): Promise<void> {
  const text: string = await $.fs.read(`${stateDir()}/events.jsonl`).catch(() => '')
  if (eventsOffset === null) {
    eventsOffset = text.length
    return
  }
  const { lines, next } = newLines(text, eventsOffset)
  eventsOffset = next
  for (const ev of parseLines<Event>(lines)) await onEvent($, ev)
}

async function onEvent($: any, ev: Event): Promise<void> {
  switch (ev.event) {
    case 'recording_started':
      await update($, live, (): LiveView => ({
        dir: String(ev.dir), title: (ev.title as string | null) ?? null, startedAt: Date.now(), phase: 'recording', lines: [],
        notesRequested: false, chat: [],
      }))
      await update($, insight, (): Insight => ({ summary: [], highlights: [], actions: [], coveredChars: 0, updatedAt: null, answer: null, asking: null }))
      lastSummaryAt = Date.now()
      await update($, browse, (b): Browse => ({ ...b, mode: 'live' }))
      void openPane($)
      return
    case 'recording_stopped':
      // The watcher transcribes by itself; the pane follows the files (see refreshPhase).
      await update($, live, (cur): LiveView => ({ ...cur, phase: 'transcribing' }))
      return
    case 'transcribed':
      if (ev.ok !== true) {
        $.ui.toast(`echo: transcription failed — ${String(ev.error ?? '')}`)
        return
      }
      // A turn of its own once the session is idle (the notes step needs Claude) — only in
      // the session that claims the call.
      if (await processCall($, String(ev.dir))) {
        await update($, live, (cur): LiveView => ({ ...cur, phase: 'writing-notes', notesRequested: true }))
      }
      return
    // meeting_soon / call_started with `ask`: the app asks with its own dialog.
    case 'error':
      $.ui.toast(`echo: ${String(ev.message)}`)
      return
    case 'mic_muted':
    case 'mic_unmuted':
      await update($, band, (cur): Band => ({ ...cur, muted: ev.event === 'mic_muted' }))
      return
    case 'arm_expired':
      $.ui.toast('echo: the arm expired — no call started within 15 min')
      return
  }
}

/** Mark an action item done / to do (the user's state; process-call never resets it). */
async function toggleAction($: any, dir: string, index: number): Promise<void> {
  const path = `${callsDir()}/${dir}/actions.json`
  try {
    const items = JSON.parse(await $.fs.read(path)) as Record<string, unknown>[]
    const it = items[index]
    if (!it) return
    it.status = it.status === 'done' ? 'todo' : 'done'
    await $.fs.write(path, JSON.stringify(items, null, 2) + '\n')
  } catch (err) {
    $.ui.toast(`echo: couldn't update the action item — ${String(err)}`)
  }
  await selectCall($, dir, 'actions')
}

const QUICK = {
  missed: {
    title: 'What you missed',
    window: 5 * 60,
    system:
      'The user stepped away from a call that is still going. From the machine transcript of the last few minutes ' +
      '("Me" is the user, "Remote" the others; expect recognition errors), tell them in 3-5 short bullets what was said, ' +
      'decided or asked — especially anything addressed to them. Plain text bullets, no preamble. Only what is in the transcript.',
  },
  question: {
    title: 'You could ask',
    window: 10 * 60,
    system:
      'The user is in a call that is still going. From the machine transcript ("Me" is the user, "Remote" the others), ' +
      'suggest 2-3 short, specific questions the user could ask next to clarify, unblock or move things forward. ' +
      'Plain text, one per line, no preamble. Base them only on the transcript. Use the language of the call.',
  },
} as const

/** Live quick actions over the recent transcript (small model, answer shown in the pane). */
async function quickAction($: any, kind: 'missed' | 'question'): Promise<void> {
  const cur = await read($, insight)
  if (cur.asking !== null) return
  await update($, insight, (i): Insight => ({ ...i, asking: kind }))
  const q = QUICK[kind]
  try {
    const v = await read($, live)
    const last = v.lines.at(-1)?.end ?? 0
    const recent = transcriptText(v.lines.filter(l => l.end >= last - q.window)).slice(-12_000)
    const r = await $.model.complete({ model: LIVE_MODEL, system: q.system, prompt: recent, maxTokens: 500, timeoutMs: 30_000, effort: 'low' })
    const text = r.isAnswered ? String(r.text).trim() : `No answer (${r.reason ?? 'unknown'})`
    await update($, insight, (i): Insight => ({ ...i, asking: null, answer: { title: q.title, text } }))
  } catch (err) {
    await update($, insight, (i): Insight => ({ ...i, asking: null, answer: { title: q.title, text: `Failed: ${String(err)}` } }))
  }
}

/** Rolling notes over the live transcript with a small model, at most every 90 s. */
async function maybeSummarize($: any, lines: LiveLine[]): Promise<void> {
  if (summarizing || Date.now() - lastSummaryAt < SUMMARY_EVERY_MS) return
  const all = transcriptText(lines)
  const prev = await read($, insight)
  if (all.length - prev.coveredChars < SUMMARY_MIN_NEW_CHARS) return
  summarizing = true
  lastSummaryAt = Date.now()
  try {
    const recent = all.slice(-SUMMARY_WINDOW_CHARS)
    const system =
      'You keep live notes for a call that is still in progress. The transcript is machine-made: ' +
      '"Me" is the user, "Remote" is everyone else; expect recognition errors. Reply with JSON only: ' +
      '{"summary": [up to 3 short bullets: what the call is about and where it stands], ' +
      '"highlights": [up to 5 short bullets: decisions, numbers, dates, notable statements], ' +
      '"actions": [up to 5: "owner — task" for commitments actually made]}. ' +
      'Write in the language most of the transcript is in. Never add anything that is not in the transcript.'
    const prompt =
      (prev.updatedAt
        ? `Previous notes (update them; keep what still holds):\n${JSON.stringify({ summary: prev.summary, highlights: prev.highlights, actions: prev.actions })}\n\n`
        : '') + `Transcript so far${recent.length < all.length ? ' (latest part)' : ''}:\n${recent}`
    const r = await $.model.complete({ model: LIVE_MODEL, system, prompt, maxTokens: 800, timeoutMs: 30_000, effort: 'low' })
    if (!r.isAnswered) return
    const parsed = extractJson(r.text) as Record<string, unknown>
    await update($, insight, (cur): Insight => ({
      ...cur,
      summary: strings(parsed.summary, 3),
      highlights: strings(parsed.highlights, 5),
      actions: strings(parsed.actions, 5),
      coveredChars: all.length,
      updatedAt: Date.now(),
    }))
  } catch {
    // keep the previous notes; try again next period
  } finally {
    summarizing = false
  }
}
