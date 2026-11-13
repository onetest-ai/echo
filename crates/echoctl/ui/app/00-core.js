// Echo window. Vanilla JS, no dependencies. Data comes from echo:// (`api/*`), actions go to the
// app over `window.ipc.postMessage`. Everything shown is escaped first: chat and transcripts are
// written by other people.
'use strict'

const $ = (sel, root = document) => root.querySelector(sel)
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
// The window's IPC; in a browser against `echoctl window --serve` (dev, ECHOCTL_SERVE_IPC=1) the same message is posted,
// and the app's answer — the script the window would run (`Handled.reply`, e.g. a save's `window.echo.saved(…)`), and
// the reload it asks for (`Handled.refresh`) — runs here too, as in the webview.
const send = (msg) => window.ipc ? window.ipc.postMessage(JSON.stringify(msg))
  : location.protocol === 'http:' && location.hostname === '127.0.0.1'
    ? fetch('ipc', { method: 'POST', body: JSON.stringify(msg) }).then((r) => r.ok ? r.json() : null).then((a) => { if (a && a.reply) (0, eval)(a.reply); if (a && a.refresh && window.echo) window.echo.refresh() }).catch(() => {})
    : undefined
const get = async (path) => {
  const r = await fetch(path)
  if (!r.ok) throw new Error(`${r.status} ${path}`)
  return r.json()
}

const S = {
  view: 'calls', // calls | calendar
  cal: { mode: 'week', offset: 0, data: null, selected: null, loading: false, ctx: null },
  calls: null,
  agenda: null,
  state: null,
  selected: null,
  call: null,
  tab: 'minutes',
  query: '',
  find: '',
  jump: null, // seconds to scroll the transcript to
  confirmDelete: false,
  lightbox: null,
  error: null,
  liveDraft: '', // the question being typed in the Live tab (kept across refreshes)
  notesProfile: null, // {call, id}: the AI profile picked for Write minutes
  prepOpen: {}, // call → the "Last time" card's open state, once the user toggled it
  liveMenu: false, // My notes' live notes menu (⋯ at Now) is open
  fromLink: false, // the page was opened at an echo:// link (`#link=`): start-up opens no call for it
  autoTab: null, // the call whose tab was last chosen for the user (Live while it records)
  notice: null, // a message for a moment (an action that couldn't be done)
  maximized: false, // Windows, Echo's own title bar: the window is maximised (its button shows restore)
  closing: false, // the window is saving the user's notes before it closes (`prepareClose`)
  closeError: null, // why they couldn't be saved (the window stays open)
  renderOnBlur: false, // a render held back while the user types their notes
  overlay: false, // a narrow window shows the right panel over the page (asked for, never saved)
  stopAsk: false, // Stop was clicked: the pill asks before the recording ends
  listOpen: false, // a narrow window shows the calls list over the call
  menuOpen: false, // the call's ⋯ menu
  notesRaw: false, // My notes shows the file as text (Edit as text) instead of the timeline
  folderProfile: null, // the AI profile picked for Claude · Echo folder (null: the default)
  sessions: null, // Claude conversations that can be resumed (`<profile>:folder`, `<profile>:<stamp>`)
  settings: null, // Settings' snapshot (`api/settings`)
  tk: { data: null, client: '', project: '', sort: 'due', mode: '', more: {} }, // Tasks: across projects
  br: { tab: 'sources', scope: '', data: null, graph: null, node: null, nodeData: null, edit: null }, // the Brain
  pj: { client: null, adding: null, pfEdit: null, menu: false, work: null, node: null, openOnly: true, view: 'tree', kind: '', owner: '', editField: null, found: false, foundData: null, data: null, sel: null, tab: 'brief', detail: null, briefEdit: null, showClosed: false, confirmDelete: false, newClient: false, picker: false }, // Projects
  setSection: 'general', // Settings' open section
  noteDraft: '', // the note being typed in My notes' box
  noteAt: null, // the call's time when it was started (seconds), while the call records
  noteAtOk: null, // the app's answer for it: does it record the call (`confirmNoteAt`, #358)
  notesHeld: [], // notes and flags taken, waiting for that answer at Add (`held`)
  noteDraftCall: null, // the call the note in the box is for (the one open at its first key)
  noteKind: 'note', // note | question | decision | todo
  selNote: null, // a comment on words selected in the transcript: {t, quote, x, y, open}
  noteEdit: null, // the note being edited in place: {i (its block), text, kind, call}
}

