export type LiveLine = {
  speaker: 'me' | 'remote'
  start: number
  end: number
  lang: string
  text: string
  /** who the remote speaker is, when known (1:1 invite) */
  name?: string
}

/** One message of the meeting's Teams chat (`<call>/chat.json`). */
export type ChatLine = {
  /** seconds from the recording start (negative: before it); null if unknown */
  t: number | null
  from: string
  text: string
  files: string[]
}

export type LiveView = {
  /** Call folder being recorded or just recorded; null when idle */
  dir: string | null
  title: string | null
  /** ms since epoch */
  startedAt: number | null
  phase: 'idle' | 'recording' | 'transcribing' | 'writing-notes' | 'done'
  /** set when the notes turn was asked for */
  notesRequested: boolean
  lines: LiveLine[]
  /** the meeting chat so far */
  chat: ChatLine[]
}

export type Insight = {
  summary: string[]
  highlights: string[]
  actions: string[]
  /** transcript chars covered by this insight */
  coveredChars: number
  updatedAt: number | null
  /** answer to a live quick action ("What did I miss?", "Suggest a question") */
  answer: { title: string; text: string } | null
  asking: 'missed' | 'question' | null
}

/** One recorded call in the pane's call browser. */
export type CallItem = {
  dir: string
  when: string
  title: string
  minutes: number | null
  status: 'recording' | 'transcribing' | 'transcript' | 'notes' | 'audio only'
}

export type DocTab = 'notes' | 'transcript' | 'actions' | 'chat'

export type ActionItem = {
  task: string
  owner: string | null
  due: string | null
  confidence: 'high' | 'low'
  /** seconds into the call where it was said */
  t: number | null
  status: 'todo' | 'done'
}

export type CallMeta = { title: string; when: string; minutes: number | null; people: string[] }

export type Browse = {
  mode: 'live' | 'calls'
  calls: CallItem[]
  selected: string | null
  tab: DocTab
  meta: CallMeta | null
  /** notes tab: Markdown */
  doc: string
  /** notes.md exists for the selected call */
  hasNotes: boolean
  /** transcript tab */
  transcript: LiveLine[]
  /** transcript tab starts here (s), after a jump from a [mm:ss] anchor; null = from the start */
  from: number | null
  /** chat tab: the meeting chat */
  chat: ChatLine[]
  /** actions tab; null = not written yet */
  actions: ActionItem[] | null
  /** asking "delete this call?" for the selected call */
  confirmDelete: boolean
  /** what the pane is generating for the selected call right now */
  generating: 'notes' | 'actions' | null
}

/** What the band above the prompt shows (from the watcher heartbeat + agenda). */
export type Band = {
  watching: boolean
  recording: boolean
  armed: boolean
  next: string | null
  /** muted in the call app (the mic track records silence meanwhile) */
  muted: boolean
}

declare module 'claude-code' {
  interface PluginState {
    echo: { live: LiveView; insight: Insight; band: Band; browse: Browse }
  }
}
