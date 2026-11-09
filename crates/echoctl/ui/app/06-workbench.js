// ── workbench: what the window is made of, and where each view goes ────────────────────────
// The window is fixed areas (activity bar, primary view, right and bottom panels) filled from
// these lists; Echo's own views are entries like a plugin's will be. No DOM in here:
// `workbench.test.mjs` drives it from node.
const ICON = (d) => `<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`
/** Activities (the activity bar, in order). `layout`: `list-detail` (a list beside the item) or `single`. */
/** The Brain is built in and switched on (Settings → Brain); off, it shows nowhere. */
const brainOn = () => !(typeof S !== 'undefined' && S.state && S.state.brain === false)
/** The activities shown: those whose `when` (if any) holds. */
const shownActivities = () => ACTIVITIES.filter((a) => !a.when || a.when())
const ACTIVITIES = [
  { id: 'today', label: 'Today', layout: 'single', icon: ICON('<circle cx="12" cy="12" r="4"/><path d="M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M5.3 18.7l1.8-1.8M16.9 7.1l1.8-1.8"/>') },
  { id: 'calls', label: 'Calls', layout: 'list-detail', icon: ICON('<path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"/>') },
  { id: 'calendar', label: 'Calendar', layout: 'single', icon: ICON('<rect x="3.5" y="5" width="17" height="15" rx="2.5"/><path d="M3.5 10h17M8 3v4M16 3v4"/>') },
  { id: 'projects', label: 'Threads', layout: 'single', icon: ICON('<path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/><path d="M8 13h8"/>') },
  { id: 'tasks', label: 'Tasks', layout: 'single', icon: ICON('<rect x="4" y="4" width="16" height="16" rx="3.5"/><path d="M8.5 12.2l2.4 2.4 4.6-4.9"/>') },
  { id: 'brain', label: 'Knowledge', layout: 'single', when: () => brainOn() || knowledgeTabs().length > 0, icon: ICON('<path d="M9 4.5a3 3 0 0 0-3 3 3 3 0 0 0-2 5.2A3.2 3.2 0 0 0 7.5 18 2.5 2.5 0 0 0 12 19V6a2 2 0 0 0-3-1.5z"/><path d="M15 4.5a3 3 0 0 1 3 3 3 3 0 0 1 2 5.2A3.2 3.2 0 0 1 16.5 18 2.5 2.5 0 0 1 12 19"/>') },
  { id: 'settings', label: 'Settings', layout: 'single', bottom: true, icon: ICON('<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>') },
]
/** A call's tabs, in order. `x` = what the call has: `{recording, chat, frames}` (counts). One Transcript tab: live
 *  while the call records, the final transcript after — the transcript alone either way (the live notes are in My
 *  notes). No Actions tab: the minutes list the action items, What moved (a right panel view) the user's tasks. */
const CALL_TABS = [
  { id: 'prep', label: 'Prep', when: (x) => !!x.prep },
  { id: 'minutes', label: 'Minutes' },
  { id: 'transcript', label: 'Transcript' },
  { id: 'chat', label: 'Chat', when: (x) => x.chat > 0, count: (x) => x.chat },
  { id: 'screens', label: 'Screens', when: (x) => x.frames > 0, count: (x) => x.frames },
]
/** The tabs a call shows: `[id, label, count ('' = none)]`; while it records, Transcript comes first
 *  with a fourth element, `true` (its recording dot). */
const callTabs = (x) => {
  const tabs = CALL_TABS.filter((t) => !t.when || t.when(x)).map((t) => [t.id, t.label, t.count ? t.count(x) : ''])
  if (!x.recording) return tabs
  const i = tabs.findIndex((t) => t[0] === 'transcript')
  return [[...tabs[i], true], ...tabs.filter((_, j) => j !== i)]
}
/** The app a calendar meeting is in, from the event's `app`: `{name, other}` (`other`: one Echo can't record yet,
 *  Zoom or Google Meet, #228), or null when no app is known. */
const meetingApp = (e) => {
  const name = { teams: 'Teams', zoom: 'Zoom', meet: 'Google Meet' }[e && e.app]
  return typeof name === 'string' ? { name, other: e.app !== 'teams' } : null
}
const otherApp = (e) => { const a = meetingApp(e); return !!a && a.other }
/** What a calendar event's card offers, first match wins: `e` the event, `st` its timing
 *  (`{past, live, recordable}`). The card draws each kind (`eventDetail`). */
const EVENT_ACTIONS = [
  { kind: 'open-notes', when: (e) => !!e.recorded },
  // Echo records Teams calls only: a Zoom or Google Meet meeting says so instead of offering Record.
  { kind: 'other-app', when: (e, st) => st.recordable && otherApp(e) },
  { kind: 'import', when: (e, st) => st.past && !!e.join_url && !otherApp(e) && !e.cancelled && !e.declined },
  { kind: 'scheduled', when: (e, st) => st.recordable && !!e.scheduled },
  { kind: 'join', when: (e, st) => st.recordable && st.live && !!e.join_url },
  { kind: 'record', when: (e, st) => st.recordable },
]
const eventAction = (e, st) => (EVENT_ACTIONS.find((a) => a.when(e, st)) || {}).kind || null
/** Views that open in the right or bottom panel: `{id, label, area, when(), render()}` (`when`:
 *  shown with the current activity; added where their renderers live: My notes below the Notes code). */
const PANEL_VIEWS = []

/** Where panel views go and which panels are open: `<config dir>/window-layout.json`, per computer,
 *  saved over IPC (`save_layout`). A view the user never moved sits in its own `area`. */
const Layout = (() => {
  const AREAS = ['right', 'bottom']
  const SIZE = { right: 320, bottom: 220 }
  // `bar`: the activity bar, folded to its icons (the default) or open with their names.
  const fresh = () => ({ right: { open: false, size: SIZE.right, active: null }, bottom: { open: false, size: SIZE.bottom, active: null }, views: {}, bar: { open: false } })
  let L = fresh()
  const num = (v, lo, hi, d) => (typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : d)
  const areaOf = (v) => (L.views[v.id] && AREAS.includes(L.views[v.id].area) ? L.views[v.id].area : v.area)
  return {
    AREAS,
    /** Take a saved layout; anything malformed falls back to the default. */
    load(o) {
      L = fresh()
      if (!o || typeof o !== 'object') return
      for (const a of AREAS) {
        const s = o[a] || {}
        L[a] = { open: typeof s.open === 'boolean' ? s.open : L[a].open, size: num(s.size, 160, 900, SIZE[a]), active: typeof s.active === 'string' ? s.active : null }
      }
      for (const [id, v] of Object.entries(o.views || {})) if (v && AREAS.includes(v.area)) L.views[id] = { area: v.area }
      if (o.bar && typeof o.bar.open === 'boolean') L.bar.open = o.bar.open
    },
    /** The window opening: the right panel starts closed, whatever was saved — it opens when asked for
     *  (2026-11-04, the user: "closed unless the user opens it, on start always"). Its size and view stay. */
    started() { L.right.open = false },
    toJSON() { return JSON.parse(JSON.stringify(L)) },
    /** The known views in an area, in their own order. */
    views(area, known) { return known.filter((v) => areaOf(v) === area).map((v) => v.id) },
    area(id, known) { const v = known.find((x) => x.id === id); return v ? areaOf(v) : null },
    /** Move a view to the other area and show it there. */
    move(id, area, known) {
      if (!AREAS.includes(area) || !known.some((v) => v.id === id)) return false
      L.views[id] = { area }
      for (const a of AREAS) if (L[a].active === id && a !== area) L[a].active = null
      L[area].open = true
      L[area].active = id
      return true
    },
    toggle(area) { if (AREAS.includes(area)) L[area].open = !L[area].open },
    /** The activity bar: open with its names, or folded to its icons. */
    toggleBar() { L.bar.open = !L.bar.open },
    /** A panel dragged to `size` px (null: back to its default). */
    resize(area, size) { if (AREAS.includes(area)) L[area].size = size == null ? SIZE[area] : num(size, 160, 900, SIZE[area]) },
    show(id, known) { const a = this.area(id, known); if (a) { L[a].open = true; L[a].active = id } },
    /** Forget views that no longer exist (a plugin removed). */
    prune(known) { for (const id of Object.keys(L.views)) if (!known.some((v) => v.id === id)) delete L.views[id] },
    /** What to draw at this window size: an area with no views is not drawn; below 900 px wide the
     *  right panel opens over the page — only when asked for there (`overlay`: this session's
     *  choice, never saved) — and below 600 px high the bottom one is folded to its tab strip. The activity bar
     *  open takes its width only at 900 px and up; narrower, it opens over the page when asked (`barOver`). */
    effective(width, height, known, overlay = false, barOver = false) {
      const out = { narrow: width < 900 }
      out.bar = { open: out.narrow ? barOver : L.bar.open, over: out.narrow && barOver }
      for (const a of AREAS) {
        const ids = this.views(a, known)
        const active = ids.includes(L[a].active) ? L[a].active : ids[0] || null
        out[a] = { views: ids, active, shown: ids.length > 0 && L[a].open, size: L[a].size }
      }
      if (out.narrow && !overlay) out.right.shown = false
      out.right.overlay = out.right.shown && out.narrow
      out.bottom.folded = out.bottom.shown && height < 600
      return out
    },
  }
})()
// end of the workbench block

