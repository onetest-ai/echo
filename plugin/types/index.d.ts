/** What the status line above the prompt shows (from the app's heartbeat, the agenda and the recording's meta). */
export type Band = {
  watching: boolean
  recording: boolean
  armed: boolean
  next: string | null
  /** muted in the call app (the mic track records silence meanwhile) */
  muted: boolean
  /** the call folder being recorded */
  dir: string | null
  title: string | null
  /** ms since epoch */
  startedAt: number | null
}

declare module 'claude-code' {
  interface PluginState {
    echo: { band: Band }
  }
}
