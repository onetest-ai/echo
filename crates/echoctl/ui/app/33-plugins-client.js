// ── Plugins' client halves (docs/plugins-design.md, v4) ──
// Each enabled plugin's `client` module is loaded into this page (trusted, as in dsh-desktop) and its
// `apply(ctx)` adds to Echo: buttons in Echo's own places (slots), panel views, its card in Settings →
// Plugins, ⌘K actions. `ctx.call` runs a command of its host half (`POST /api/plugin/<id>/call/<command>`).
// Everything a plugin added goes when it is switched off or removed (`unloadPlugin`).
const Plugins = {
  /** Its `page` widgets' drawing, by widget id (`plugin:<id>:<widget>`): ({data, size, theme}) → HTML. */
  widgetDraw: {},
  /** slot name → [{plugin, id, label, icon, when(target), run(target)}] */
  slots: {},
  /** plugin id → {render() → html} */
  settings: {},
  /** Plugins' own Settings sections: [{plugin, id, label, icon, badge(), render()}] */
  sections: [],
  /** [{plugin, label, run, group, keywords}] */
  palette: [],
  /** plugin id → its document listeners and style elements (taken out when it unloads). */
  owned: {},
  loaded: {},
  /** plugin id → true once its client half's apply() has run (a page widget it never drew is then an error) */
  applied: {},
  errors: {},
  /** What each slot was last drawn for: its buttons run on that. */
  targets: {},
  /** plugin id → its `ctx.onFileDrop` handlers */
  drops: {},
}
// `knowledge.tab`: a tab of the Knowledge activity ({id, label, render() → HTML, after(box)}), `#brain/<plugin>:<id>/<its route>`.
// `knowledge.settings`: rows in Settings → Knowledge ({render() → setRow(…) HTML}), above Recall's card.
// `task.actions` also on a plugin's work row, for an item whose `when` is true for {kind: 'plugin', source, row}.
// `thread.profile`: a row of a thread's Profile, under its Sources ({id, label, render(thread) → HTML, after(box, thread)}),
// `thread` = {id, name, client}.
// `client.profile`: a row of a client's Profile, under its Threads, shaped as `thread.profile`; `client` = {id, name}.
// `call.row`: a chip on a call's row in the calls list ({id, label, chip(call) → short text or nothing}), `call` =
// {name, title, started_at, status, memo}.
// `call.menu`: items in a call's ⋯ menu ({label: text or (target) → text, run(target), when?(target)}), each plugin's
// group after a separator; `target` = {kind: 'call', call}.
// `call.header`: a box of the plugin's own on a call's header, before its buttons ({id, label, render(call) → HTML,
// after?(box, call)}), and `call.minutes`: a line in the call's box while its minutes are coming ({id, label, render(call)
// → HTML}); `call` = {name, title, started_at, notes, profile, memo, threads, client} (`slotCallPage`).
const SLOTS = ['call.menu', 'calendar.event', 'task.actions', 'knowledge.tab', 'knowledge.settings', 'thread.profile', 'client.profile', 'call.row', 'call.header', 'call.minutes']
// ── call row slot: the plugins' chips on a call's row (no DOM: `call-row-slot.test.mjs`) — each item's `chip(call)`
// gives a short text (24 characters, the rest cut) or nothing; one that throws is left out (logged) and the row drawn
// without it; a second item with the same plugin and id is left out. `chip` runs as a widget's draw does: a
// `ctx.render()` asked for in it is ignored (no loop).
function callRowChips(items, call, log) {
  const out = [], seen = new Set()
  for (const x of items || []) {
    const key = `${x.plugin}:${x.id}`
    if (seen.has(key)) { log(`${x.plugin}: ${x.label || x.id}: a second chip with the id “${x.id}” — left out`); continue }
    seen.add(key)
    Plugins.drawing = true
    try {
      const t = x.chip(call)
      const text = t == null ? '' : String(t).trim()
      if (text) out.push({ key, text: text.length > 24 ? text.slice(0, 23) + '…' : text, title: String(x.label || x.id) })
    } catch (e) { log(`${x.plugin}: ${x.label || x.id}: ${(e && e.message) || e}`) } finally { Plugins.drawing = false }
  }
  return out
}
/** The call a `call.row` chip is drawn for: the row's own facts. */
const slotCall = (c) => ({ name: c.name, title: c.title, started_at: c.started_at || null, status: c.status, memo: !!c.memo })
// end of the call row slot block
// ── thread slot: the plugins' rows of a thread's or a client's Profile (`thread.profile`, `client.profile`), each
// drawn on its own — one that throws is left out (and logged), the Profile draws without it; a second row with the same
// plugin and id is left out too (logged). `render` runs as a widget's draw does: a `ctx.render()` asked for in it is
// ignored (no loop).
function threadSlotRows(items, target, log) {
  const out = [], seen = new Set()
  for (const x of items || []) {
    const key = `${x.plugin}:${x.id}`
    if (seen.has(key)) { log(`${x.plugin}: ${x.label || x.id}: a second row with the id “${x.id}” — left out`); continue }
    seen.add(key)
    Plugins.drawing = true
    try {
      const html = x.render(target)
      out.push({ key, label: String(x.label || x.id), html: html == null ? '' : String(html), after: x.after })
    } catch (e) { log(`${x.plugin}: ${x.label || x.id}: ${(e && e.message) || e}`) } finally { Plugins.drawing = false }
  }
  return out
}
/** Each row's `after(box, target)` for the boxes marked `data-profile-slot`, once per box drawn afresh (a redraw that
 *  left the page as it was keeps the boxes, marked): `after` may fetch and `ctx.render()` without drawing itself again
 *  forever. */
function threadSlotAfter(root, items, target, log) {
  const boxes = [...root.querySelectorAll('[data-profile-slot]')]
  const seen = new Set()
  for (const x of items || []) {
    const key = `${x.plugin}:${x.id}`
    if (seen.has(key)) continue
    seen.add(key)
    const box = x.after && boxes.find((b) => b.dataset.profileSlot === key)
    if (!box || box._afterFor === target.id) continue
    box._afterFor = target.id
    try { x.after(box, target) } catch (e) { log(`${x.plugin}: ${x.label || x.id}: ${(e && e.message) || e}`) }
  }
}
// end of the thread slot block
/** The thread a `thread.profile` row is drawn for. */
const slotThread = (p) => ({ id: p.id, name: p.name, client: p.client || null })
/** The client a `client.profile` row is drawn for. */
const slotClient = (c) => ({ id: c.id, name: c.name })
/** `ctx.echo.jobs.set` requests waiting for Echo's answer (`window.echo.jobsSet`), by request id. */
const JobsAsked = {}

// A plugin request's answer: its value, or the error thrown.
const pluginValue = (r) => { if (!r || r.error) throw new Error((r && r.error) || 'Echo did not answer'); return r.value }

// A plugin's call with what it sends (`ctx.call`, `ctx.markdown`): JSON in a POST's body, which Echo takes up to 4 MB;
// Echo's answer, its refusal's reason included.
const pluginPost = async (id, what, body) => {
  const path = `api/plugin/${encodeURIComponent(id)}/${what}`
  const r = await fetch(path, { method: 'POST', body: JSON.stringify(body) })
  const v = await r.json().catch(() => null)
  if (!r.ok && !(v && v.error)) throw new Error(`${r.status} ${path}`)
  return v
}

function pluginCtx(id) {
  const add = (list, item) => { list.push({ ...item, plugin: id }); render() }
  const owned = Plugins.owned[id] = Plugins.owned[id] || { listeners: [], windowListeners: [], styles: [], cleanups: [] }
  // Unloaded since (a timer it didn't clear): what it adds now would never be taken out.
  const gone = () => Plugins.owned[id] !== owned
  return {
    id,
    /** A document listener that goes when the plugin is switched off, updated or removed. */
    listen: (type, fn, opts) => { document.addEventListener(type, fn, opts); owned.listeners.push([type, fn, opts]) },
    /** The same for a window listener (resize, focus, message). */
    listenWindow: (type, fn, opts) => { if (gone()) return; window.addEventListener(type, fn, opts); owned.windowListeners.push([type, fn, opts]) },
    /** Its own cleanup (timers, observers), run when it is switched off, updated or removed. */
    onUnload: (fn) => { if (!gone()) owned.cleanups.push(fn) },
    /** Files dropped on the window while one of its views is shown (its activity, its Knowledge tab): `fn(files)`, each
     *  a handle {file, name, size} for `call(…, {files})`. macOS. */
    onFileDrop: (fn) => { if (gone()) return; (Plugins.drops[id] = Plugins.drops[id] || []).push(fn); followDrops() },
    /** Its host half's command; `{files}`: handles it was given (a drop's), the command's own `{files}`. */
    call: async (command, args = {}, opts) => {
      const files = opts && opts.files ? opts.files.map((f) => (f && f.file) || String(f)) : []
      const r = await pluginPost(id, `call/${encodeURIComponent(command)}`, { args, files })
      if (r && r.error) throw new Error(r.error)
      return r ? r.value : null
    },
    /** Markdown as safe HTML by Echo's writer: `markdown(md, {breaks, links: {dest: href | null}, images: {dest: src |
     *  null}})` — only the hrefs Echo allows (https, echo://, its own `#<id>:` and `#brain/<id>:` routes; images from its
     *  folder route), `null` a missing page (`data-md-missing`, no href); `markdown.links(md)` → {links, images}: what to
     *  resolve first. The page goes in the request's body (4 MB at most; Echo renders 1 MB of text). */
    markdown: Object.assign(async (md, opts) => pluginValue(await pluginPost(id, 'markdown', { md: String(md), opts: opts || {} })),
      { links: async (md) => pluginValue(await pluginPost(id, 'markdown/links', { md: String(md) })) }),
    /** Echo's background jobs, when its manifest asks for `uses.echo.jobs` (echo-plugins R25): `list()` — every job, its
     *  settings (value, default, range), last run, runs today, and the night; `set(job, ["key=value", …])` — only on the
     *  user's own click or key (else it throws), checked as `echoctl jobs set`; resolves to the list after. */
    echo: {
      /** Echo's jobs as routines — the Scheduler's one shape for every routine (`uses.echo.jobs`): `list()` →
       *  `{routines, night: {from, to, default}}`; `runs(id, {limit})` → its runs, newest first. Changes: `jobs.set`. */
      routines: {
        list: async () => {
          const r = await get(`api/plugin/${encodeURIComponent(id)}/routines`)
          if (!r || r.error) throw new Error((r && r.error) || 'Echo did not answer')
          return r.value
        },
        runs: async (rid, opts) => {
          opts = opts || {}
          const r = await get(`api/plugin/${encodeURIComponent(id)}/routines/runs?id=${encodeURIComponent(String(rid))}&limit=${encodeURIComponent(String(opts.limit || 20))}`)
          if (!r || r.error) throw new Error((r && r.error) || 'Echo did not answer')
          return r.value
        },
      },
      jobs: {
        list: async () => {
          const r = await get(`api/plugin/${encodeURIComponent(id)}/jobs`)
          if (!r || r.error) throw new Error((r && r.error) || 'Echo did not answer')
          return r.value
        },
        /** A job's runs, newest first (at most `limit`, 20 by default, 500 at most): what `echoctl jobs runs` prints. */
        runs: async (job, opts) => {
          opts = opts || {}
          const r = await get(`api/plugin/${encodeURIComponent(id)}/jobs/runs?job=${encodeURIComponent(String(job))}&limit=${encodeURIComponent(String(opts.limit || 20))}`)
          if (!r || r.error) throw new Error((r && r.error) || 'Echo did not answer')
          return r.value
        },
        set: (job, settings) => {
          if (!copyAllowed(typeof navigator === 'undefined' ? null : navigator)) return Promise.reject(new Error("Echo's jobs change only on your click or key"))
          // Through the page's IPC (never a GET); Echo answers this request by `window.echo.jobsSet`.
          const req = `${id}:${Date.now()}:${Math.random().toString(36).slice(2)}`
          return new Promise((resolve, reject) => {
            JobsAsked[req] = { resolve, reject }
            setTimeout(() => { if (JobsAsked[req]) { delete JobsAsked[req]; reject(new Error('Echo did not answer')) } }, 15000)
            send({ cmd: 'plugin_jobs_set', id, req, job: String(job), settings: (settings || []).map(String) })
          })
        },
      },
    },
    slots: {
      list: () => SLOTS.slice(),
      add: (slot, item) => { if (!SLOTS.includes(slot)) throw new Error(`Echo has no slot “${slot}” (${SLOTS.join(', ')})`); add(Plugins.slots[slot] = Plugins.slots[slot] || [], item) },
    },
    panels: { add: (v) => { PANEL_VIEWS.push({ area: 'right', ...v, id: `${id}:${v.id}`, plugin: id }); render() } },
    /** Its `page` widgets (kind `page` in its manifest): draw(widget, ({data, size, theme}) => HTML) — data from its
     *  host half (provide / set); every text through `ctx.ui.esc`. Draw from `data` only (the same data, the same HTML:
     *  else Today is drawn again on every poll); a render() asked for while drawing is ignored. */
    widgets: { draw: (widget, fn) => { Plugins.widgetDraw[`plugin:${id}:${widget}`] = fn; render() } },
    settings: {
      /** Its card in Settings → Plugins. */
      add: (v) => { Plugins.settings[id] = v; render() },
      /** A section of its own in Settings ("From plugins"); its id is `<plugin>:<id>` in the hash. */
      section: (v) => { Plugins.sections.push({ ...v, id: `${id}:${v.id || 'main'}`, plugin: id }); render() },
    },
    /** An activity of its own in the activity bar, under Echo's (`#<plugin>:<id>/<its route>`): {id, label, icon,
     *  render() → HTML of the page, after(box) — after each render (draw a graph), badge?() → '' | n | '!' on its icon,
     *  editing?() → true while the user edits in it: Echo then leaves its box as it is}. */
    activities: {
      add: (a) => {
        const at = ACTIVITIES.findIndex((x) => x.bottom)
        ACTIVITIES.splice(at < 0 ? ACTIVITIES.length : at, 0, { layout: 'single', ...a, id: `${id}:${a.id}`, plugin: id })
        // Only a hash of this activity is routed again (no other route's side effects re-run).
        const h = hashText().slice(1), aid = `${id}:${a.id}`
        if (h === aid || h.startsWith(aid + '/')) route()
        render()
      },
    },
    /** Open one of its panel views (`ctx.panels.add`). */
    showPanel: (view) => { Layout.show(`${id}:${view}`, panelViews()); layoutChanged() },
    /** Claude Code in a folder the user gave it (`uses.claude` and `uses.folders`): `session({folder, path, mode,
     *  profile})` opens it in Echo's terminal panel, only on the user's click or key (else it throws) — the user types
     *  there, the plugin can't, nor read its screen. Open already: shown again (in another folder it throws). */
    claude: {
      session: (opts) => {
        const key = `claude:plugin:${id}`, e = Terms.get(key)
        const live = !!(e && e.id != null && !e.exited)
        const a = pluginSessionAsk(id, opts, typeof navigator === 'undefined' ? null : navigator, live ? (S.ptyAbout || {})[key] || {} : null)
        // Its own tab (each plugin's session has one), shown; started only when it isn't running.
        if (a.start || !(S.plugSessions || {})[key]) plugSessionNote(id, a.other ? { ...aboutExtra((S.ptyAbout || {})[key]), plugin: id } : a.extra)
        claudePanel(plugSessionView(id))
        if (a.other) throw new Error('Claude is open for this plugin in another folder: close it in its panel first')
        if (a.start) termStart('claude-plugin', key, a.extra)
      },
    },
    /** Open one of Echo's own panel views, e.g. `claude-folder` (the Claude · Echo folder session). */
    openEchoPanel: (view) => { if (view.includes(':')) return; go('calls'); setTimeout(() => { Layout.show(view, panelViews()); layoutChanged() }, 50) },
    /** Where the window is: the activity and the Settings section (`#settings/<plugin>:<id>`). */
    where: () => ({ view: S.view, section: S.setSection, route: S.plRoute || '', tab: S.view === 'brain' ? S.br.tab : undefined, recall: brainOn() }),
    go: (hash) => go(hash),
    palette: { add: (item) => add(Plugins.palette, item) },
    /** Echo's own pieces, so a plugin looks like Echo. */
    ui: {
      esc, setRow, setCard, sw, ICON, relDay, ago, notice: (text) => { S.notice = text; render() },
      /** Echo's own `patch`: replaces a box's HTML unless it is what `patch` last wrote there (the first call on a fresh box
       *  writes it), so redrawn the same, its focus, typed text and scroll stay. */
      patch,
      /** A stylesheet of its installed package (`styles/view.css`), resolved when loaded; taken out when it unloads. */
      stylesheet: (path) => new Promise((resolve, reject) => {
        if (gone()) { reject(new Error('the plugin is no longer loaded')); return }
        let href
        try { href = pluginStylesheetHref(id, path, Plugins.loaded[id]) } catch (e) { reject(e); return }
        const el = document.createElement('link')
        el.rel = 'stylesheet'; el.dataset.plugin = id; el.href = href
        el.onload = () => resolve(); el.onerror = () => reject(new Error(`Echo could not load ${path}`))
        owned.styles.push(el); document.head.appendChild(el)
      }),
      /** Put text on the clipboard (a code to paste elsewhere) on the user's click or key (`copyAllowed`: a guard
       *  against accidental copies, not a security boundary). At most 10,000 characters. */
      copy: (text) => {
        if (!copyAllowed(typeof navigator === 'undefined' ? null : navigator)) throw new Error('the clipboard is filled only on your click')
        send({ cmd: 'copy', text: String(text).slice(0, 10000) })
      },
      /** A sheet over the window (Echo's modal frame around its `html`; `{label, onClose}`): one at a time — another
       *  plugin's or its own earlier one closes first. Esc, a click beside it or `closeSheet()` close it, `onClose` then
       *  called once; its plugin unloaded, it closes without it. Returns the sheet's box; its events through `listen`. */
      sheet: (html, opts) => {
        if (gone()) throw new Error('the plugin is no longer loaded')
        if (!owned.sheet) { owned.sheet = true; owned.cleanups.push(() => sheetShut(id, true)) }
        return sheetShow(id, html, opts || {})
      },
      /** Its sheet closed (its `onClose` called); another plugin's is left open. */
      closeSheet: () => { sheetShut(id) },
      /** Its CSS in the page, taken out when it unloads. */
      style: (css) => { const el = document.createElement('style'); el.dataset.plugin = id; el.textContent = String(css); document.head.appendChild(el); owned.styles.push(el) },
      /** Echo's graph (Sigma over graphology, as the Brain's): draw(box, key, build, {click}), focus, select, find. */
      graph: GraphView,
      newGraph: () => new graphology.Graph(),
    },
    /** The microphone, when its manifest asks for `uses.microphone`: `open(audio)` → a MediaStream, only on the user's
     *  click or key in its own view (its activity, its Knowledge tab) and never while Echo records (else it throws why);
     *  Echo's recording sign shows while it is open. It stops when the view is left, the plugin is switched off,
     *  updated or removed, or Echo starts recording; `close()` stops it sooner. `audio`: getUserMedia's audio
     *  constraints (true by default). */
    microphone: {
      open: (audio) => micOpen(id, audio),
      close: () => micClose(id),
      isOpen: () => !!(Mic.stream && Mic.id === id),
    },
    render: () => { if (!Plugins.drawing) render() },
  }
}
// ── plugin microphone: why a plugin's window half may not open the microphone now, or '' (no DOM:
// `plugin-mic.test.mjs`) — only on the user's click or key (`copyAllowed`) in its own view (its activity
// `#<plugin>:<id>…`, its Knowledge tab `#brain/<plugin>:<id>…`), while Echo records nothing and no other plugin has it.
// Echo checks again (`plugin_mic`): the plugin on, its install sheet, nothing recording.
function micRefusal(id, { nav, hash, recording, holder }) {
  if (recording) return 'Echo is recording — a plugin gets the microphone once the recording stops'
  if (holder && holder !== id) return 'another plugin has the microphone now'
  if (!copyAllowed(nav)) return 'the microphone opens only on your click'
  if (micView(hash) !== id) return "the microphone opens only in the plugin's own view"
  return ''
}
/** The plugin whose own view the address shows, or null. */
function micView(hash) {
  const parts = String(hash || '').replace(/^#/, '').split('/')
  const m = (parts[0] === 'brain' ? parts[1] || '' : parts[0]).match(/^([a-z0-9-]+):/)
  return m ? m[1] : null
}
// end of the plugin microphone block
/** The plugin microphone open in this page (one at a time): its plugin, its name for the sign, its stream. */
const Mic = { id: null, name: '', stream: null }
async function micOpen(id, audio) {
  if (Mic.stream && Mic.id === id) return Mic.stream
  const why = micRefusal(id, { nav: typeof navigator === 'undefined' ? null : navigator, hash: hashText(), recording: !!(S.state && S.state.recording), holder: Mic.id })
  if (why) throw new Error(why)
  // Echo checks and arms the webview's one grant first (its answer: the plugin's name); only then the page asks for it.
  const name = String(pluginValue(await pluginPost(id, 'microphone', {})) || id)
  Mic.id = id; Mic.name = name; Mic.stream = null
  render()
  let stream
  try { stream = await navigator.mediaDevices.getUserMedia({ audio: audio == null ? true : audio, video: false }) } catch (e) {
    micClose(id)
    throw new Error(`The microphone didn't open: ${(e && e.message) || e}`)
  }
  // Closed while the system asked (its view left, the plugin gone, a recording started): never handed over.
  if (Mic.id !== id) { stream.getTracks().forEach((t) => t.stop()); throw new Error('the microphone stopped before it opened') }
  Mic.stream = stream
  return stream
}
/** The plugin's microphone stopped (any plugin's: no id), Echo told; `why` said on the page. */
function micClose(id, why) {
  if (!Mic.id || (id && Mic.id !== id)) return
  const s = Mic.stream, was = Mic.id
  Mic.id = null; Mic.name = ''; Mic.stream = null
  if (s) s.getTracks().forEach((t) => t.stop())
  send({ cmd: 'plugin_mic', id: was })
  if (why) S.notice = why
  render()
}
/** On each read of Echo's state: a recording started, or the plugin stopped every track itself — the microphone closes. */
function micFollow() {
  if (!Mic.id) return
  if (S.state && S.state.recording) micClose(Mic.id, "Echo started recording, so the plugin's microphone stopped")
  else if (Mic.stream && Mic.stream.getTracks().every((t) => t.readyState === 'ended')) micClose(Mic.id)
}
// Its view left: it stops.
window.addEventListener('hashchange', () => { if (Mic.id && micView(hashText()) !== Mic.id) micClose(Mic.id) })
// A page loaded (again): nothing it had is open any more.
send({ cmd: 'plugin_mic' })
/** Load the client halves of the enabled plugins not loaded yet (or at another version). */
// One load at a time: a later one (a fresher list) waits for the one running, never applying a half twice.
let pluginLoads = Promise.resolve()
function loadPluginClients(snapshot) {
  pluginLoads = pluginLoads.then(() => loadPluginClientsNow(snapshot), () => loadPluginClientsNow(snapshot))
  return pluginLoads
}
async function loadPluginClientsNow(snapshot) {
  let p = snapshot
  if (!p) try { p = await get('api/plugins') } catch { return }
  const want = (p.installed || []).filter((x) => x.enabled && x.client)
  // Drawn again only when a client half came or went (nothing changed: nothing to draw).
  let changed = false
  // An update changes its commit (a local plugin's, with its files), its version not always: the new half replaces the old.
  const key = (x) => `${x.version}@${x.commit || ''}`
  for (const id of Object.keys(Plugins.loaded)) if (!want.some((x) => x.id === id && key(x) === Plugins.loaded[id])) { unloadPlugin(id); changed = true }
  for (const x of want) {
    if (Plugins.loaded[x.id]) continue
    changed = true
    Plugins.loaded[x.id] = key(x)
    try {
      const m = await import(new URL(`plugin/${encodeURIComponent(x.id)}/${x.client}?v=${encodeURIComponent(key(x))}`, location.href).href)
      if (typeof m.apply !== 'function') throw new Error(`${x.client} exports no apply(ctx)`)
      await m.apply(pluginCtx(x.id))
      Plugins.applied[x.id] = true
      delete Plugins.errors[x.id]
    } catch (e) { Plugins.errors[x.id] = String((e && e.message) || e) }
  }
  if (changed) render()
}
// A plugin installed, updated, switched on or off, or removed (here or in a terminal; `state.plugins` changed): its
// client half, its widgets in Add a widget and on the page.
async function pluginsChanged() {
  await loadPluginClients()
  if (S.tdEdit) { try { S.tdCatalog = await get('api/widgets') } catch { /* app restarting */ } }
  if (S.view === 'today') await loadToday()
  if (S.dash) await loadDashboard()
  render()
}
function unloadPlugin(id) {
  micClose(id)
  for (let i = ACTIVITIES.length - 1; i >= 0; i--) if (ACTIVITIES[i].plugin === id) ACTIVITIES.splice(i, 1)
  // Switched off or removed (not an update, which loads it again right away): its page goes.
  setTimeout(() => { if (S.view && S.view.startsWith(id + ':') && !ACTIVITIES.some((a) => a.id === S.view)) { S.view = 'calls'; go('calls') } }, 1500)
  const owned = Plugins.owned[id]
  if (owned) {
    for (const fn of owned.cleanups) { try { fn() } catch (e) { console.error(`plugin ${id}: its cleanup failed:`, e) } }
    for (const [type, fn, opts] of owned.windowListeners) window.removeEventListener(type, fn, opts)
    for (const [type, fn, opts] of owned.listeners) document.removeEventListener(type, fn, opts)
    for (const el of owned.styles) el.remove()
    delete Plugins.owned[id]
  }
  for (const k of Object.keys(Plugins.slots)) Plugins.slots[k] = Plugins.slots[k].filter((x) => x.plugin !== id)
  Plugins.palette = Plugins.palette.filter((x) => x.plugin !== id)
  Plugins.sections = Plugins.sections.filter((x) => x.plugin !== id)
  delete Plugins.settings[id]
  for (const k of Object.keys(Plugins.widgetDraw)) if (k.startsWith(`plugin:${id}:`)) delete Plugins.widgetDraw[k]
  for (let i = PANEL_VIEWS.length - 1; i >= 0; i--) if (PANEL_VIEWS[i].plugin === id) PANEL_VIEWS.splice(i, 1)
  // Its work rows' targets (`task.actions#<plugin>:<row>`) go with it.
  slotForget(Plugins.targets, id)
  // Its Claude session is the user's: it stays while it runs, but its panel goes once it has ended (it can't start again).
  const sk = `claude:plugin:${id}`
  if (S.plugSessions && S.plugSessions[sk]) { const e = Terms.get(sk); if (!(e && e.id != null && !e.exited)) delete S.plugSessions[sk] }
  delete Plugins.loaded[id]
  delete Plugins.applied[id]
  delete Plugins.drops[id]
  followDrops()
}
// ── plugin stylesheet: where `ctx.ui.stylesheet(path)` loads a package's CSS from (no DOM:
// `plugin-client-ctx.test.mjs`). Only a plain relative `.css` path inside the package; versioned so an update reloads it.
function pluginStylesheetHref(id, path, version) {
  const p = String(path)
  if (!/^[A-Za-z0-9_-][A-Za-z0-9_.-]*(\/[A-Za-z0-9_-][A-Za-z0-9_.-]*)*\.css$/.test(p)) throw new Error(`Not a stylesheet of the plugin: ${p}`)
  return `plugin/${encodeURIComponent(id)}/${p}?v=${encodeURIComponent(version || '')}`
}
// end of the plugin stylesheet block
// ── plugin file drops: which plugin files dropped on the window go to (no DOM: `plugin-file-drop.test.mjs`) — the one
// whose view the address shows (its activity `#<plugin>:<id>…`, its Knowledge tab `#brain/<plugin>:<id>…`) when it
// listens (`ctx.onFileDrop`); else none, and the drop is Echo's own (a logo tile's).
function dropTarget(hash, listening) {
  const parts = String(hash || '').replace(/^#/, '').split('/')
  const m = (parts[0] === 'brain' ? parts[1] || '' : parts[0]).match(/^([a-z0-9-]+):/)
  return m && (listening[m[1]] || []).length ? m[1] : null
}
// end of the plugin file drops block
/** The app is told which plugin takes dropped files now (the window's drop handler asks it as a drag comes in). */
let dropsSent
function followDrops() {
  const id = dropTarget(hashText(), Plugins.drops)
  if (id === dropsSent) return
  dropsSent = id
  send({ cmd: 'plugin_drops', id })
}
window.addEventListener('hashchange', followDrops)
followDrops()
/** Files were dropped for plugin `id` (`window.echo.pluginDrop`): its handlers get the handles — its view still shown. */
function pluginDropped(id, files) {
  if (dropTarget(hashText(), Plugins.drops) !== id) return
  for (const fn of Plugins.drops[id]) Promise.resolve().then(() => fn(files)).catch((e) => { S.notice = `${id}: ${(e && e.message) || e}`; render() })
}
// ── plugin slot targets: which of a slot's items a target gets and which target a click is for (no DOM:
// `plugin-work-rows.test.mjs`).
/** A slot's items for `target`: those whose `when` says yes — a plugin's work row (`kind: 'plugin'`) only to the items
 *  that ask for it (a `when` true for it), never to one written for Echo's tasks. */
// ── copy guard: `ctx.ui.copy` only while the page has the user's activation (a click or key in the last moments) — a
// guard against accidental copies (a timer, a fetch's answer at load), not a security boundary: a client half shares
// the page and can reach the window's IPC itself. Fails closed: an engine that can't tell (no `userActivation`) refuses.
const copyAllowed = (nav) => !!(nav && nav.userActivation && nav.userActivation.isActive === true)
// end of the copy guard block
// ── plugin session: what `ctx.claude.session(opts)` asks Echo for (no DOM: `plugin-session.test.mjs`) — only on the
// user's click or key (`copyAllowed`), in a folder it was given (`folder`, a subfolder `path`), `read` (the default) or
// `edit`, on `profile` or the default; Echo checks the rest (its sheet, the grant, the path, the profile). `running`:
// its session already open (`{folder, path}`) — shown again, never started twice; in another folder, `other`.
function pluginSessionAsk(id, o, nav, running) {
  if (!copyAllowed(nav)) throw new Error('Claude opens only on your click')
  o = o || {}
  const folder = typeof o.folder === 'string' ? o.folder : ''
  if (!folder) throw new Error('a Claude session opens in a folder you were given (folder)')
  const mode = o.mode == null || o.mode === '' ? 'read' : String(o.mode)
  if (mode !== 'read' && mode !== 'edit') throw new Error(`mode “${mode}”: a session reads (read) or edits (edit) its folder`)
  const extra = { plugin: id, folder, path: o.path == null ? '' : String(o.path), mode, profile: o.profile == null ? '' : String(o.profile) }
  const other = !!running && (running.folder !== folder || (running.path || '') !== extra.path)
  return { key: `claude:plugin:${id}`, extra, start: !running, other }
}
/** What starts a plugin's session found running (`api/ptys`' `about`) again: the same request on the profile it runs on,
 *  so Start resumes the same conversation (Echo checks the profile again). */
const aboutExtra = (a = {}) => ({ plugin: a.plugin, folder: a.folder, path: a.path || '', mode: a.mode, profile: a.profile || '' })
// end of the plugin session block
const slotFor = (slot, target) => (Plugins.slots[slot] || []).filter((x) => { try { return x.when ? x.when(target) : !(target && target.kind === 'plugin') } catch { return false } })
/** A slot's buttons for `target`: `menu` (menu items) or `buttons`; `key` when the slot is drawn for several targets at
 *  once (each row its own). */
function slotItems(slot, target, style = 'buttons', key = '') {
  const items = slotFor(slot, target)
  if (!items.length) return ''
  Plugins.targets[slot + (key ? `#${key}` : '')] = target
  const k = key ? ` data-plug-key="${esc(key)}"` : ''
  // In a menu each plugin's items are a group, a separator before it (Echo's own items above). An item whose label
  // throws or is empty is left out; the others keep their place (`data-plug-i`).
  let group = null
  return items.map((x, i) => {
    const label = slotLabel(x, target)
    if (!label) return ''
    if (style !== 'menu') return `<button class="btn" data-plug-slot="${slot}" data-plug-i="${i}"${k} title="${esc(x.title || label)}">${x.icon || ''}${esc(label)}</button>`
    const sep = x.plugin !== group ? '<div class="menu-sep" role="separator"></div>' : ''
    group = x.plugin
    return `${sep}<button class="menu-item" role="menuitem" data-plug-slot="${slot}" data-plug-i="${i}"${k}>${x.icon || ''}${esc(label)}</button>`
  }).join('')
}
/** An item's label for `target`: its text, or what its function gives (a `ctx.render()` in it ignored); '' when it throws. */
function slotLabel(x, target) {
  if (typeof x.label !== 'function') return x.label == null ? '' : String(x.label)
  Plugins.drawing = true
  try { const l = x.label(target); return l == null ? '' : String(l).trim() } catch { return '' } finally { Plugins.drawing = false }
}
/** The target a slot's button was drawn for (`data-plug-slot`, `data-plug-key`). */
const slotTarget = (slot, key) => Plugins.targets[slot + (key ? `#${key}` : '')]
/** A plugin's keyed targets (its work rows', `<slot>#<plugin>:<row>`) dropped: it was switched off or removed. */
function slotForget(targets, plugin) {
  for (const k of Object.keys(targets)) if (k.includes('#') && k.slice(k.indexOf('#') + 1).startsWith(`${plugin}:`)) delete targets[k]
}
/** What a slot button's click runs — `{item, target}` — or null: a row's button whose target went (its plugin was
 *  switched off or removed, `slotForget`) runs nothing, never an item meant for Echo's tasks. */
function slotClick(slot, key, i) {
  const target = slotTarget(slot, key)
  if (key && !target) return null
  const item = slotFor(slot, target)[+i]
  return item ? { item, target } : null
}
// end of the plugin slot targets block
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-plug-slot]')
  if (!t) return
  const hit = slotClick(t.dataset.plugSlot, t.dataset.plugKey, t.dataset.plugI)
  S.menuOpen = false
  if (!hit) return
  const { item, target } = hit
  Promise.resolve().then(() => item.run(target)).catch((err) => { S.notice = `${slotLabel(item, target) || item.plugin}: ${(err && err.message) || err}`; render() })
    // On a plugin's work row: what it shows is read again (its plugin's answer may have changed it), Today or the
    // thread's Work tab.
    .then(() => target && target.kind === 'plugin' ? (S.view === 'today' ? loadToday() : S.view === 'projects' ? loadProject() : null) : null)
    .then(() => target && target.kind === 'plugin' && render())
})

/** The transcription server row: an Echo, or an OpenAI-compatible server (Ollama with an audio
 *  model, Speaches, vLLM, LocalAI, parakeet-api…) and what it was found to give. */
function serverLine(srv) {
  const i = srv.info
  if (!srv.client || !i) return "Send each utterance to another Echo on your network, or to an OpenAI-compatible speech-to-text server (Ollama with an audio model, Speaches, vLLM, LocalAI…); this computer's model takes over when it's down."
  if (i.api === 'openai') return `Transcribing on <code>${esc(i.url)}</code> with <b>${esc(i.model || '')}</b> (OpenAI-compatible)${i.words === false ? ' — it gives no word timings: speaker turns split by sentence only' : ''}. This computer's model takes over when it's down.`
  return `Connected to ${esc(srv.client)}; this computer's model takes over when it's down.`
}
function sttForm(srv) {
  const c = srv.connecting || {}
  const f = S.sttForm
  const msg = c.busy ? '<span class="set-sub">Checking the server — its models, then a test transcription…</span>' : c.error ? `<span class="error">${esc(c.error)}</span>` : ''
  return setRow('OpenAI-compatible server', `Its address (it must answer <code>/v1/models</code> and <code>/v1/audio/transcriptions</code>), the model to ask for (empty: the first speech model it lists) and a key only if it wants one.` +
    `<div class="stt-form"><input class="set-in" id="stt-url" value="${esc(f.url)}" placeholder="http://localhost:11434" aria-label="Server address" spellcheck="false">` +
    `<input class="set-in" id="stt-model" value="${esc(f.model)}" placeholder="model — e.g. lfm3-audio, whisper-1" aria-label="Model" spellcheck="false">` +
    `<input class="set-in" id="stt-key" type="password" placeholder="key (optional)" aria-label="Key">` +
    `<button class="btn primary" data-stt-connect${c.busy ? ' disabled' : ''}>Connect</button></div>${msg ? `<div>${msg}</div>` : ''}`, '')
}
document.addEventListener('click', (e) => {
  if (e.target.closest && e.target.closest('[data-stt-form]')) { S.sttForm = S.sttForm ? null : { url: 'http://localhost:11434', model: '' }; return render() }
  if (!(e.target.closest && e.target.closest('[data-stt-connect]'))) return
  const url = ($('#stt-url') || {}).value || '', model = ($('#stt-model') || {}).value || '', key = ($('#stt-key') || {}).value || ''
  S.sttForm = { url, model }
  send({ cmd: 'server_openai', url, model, key })
  const poll = () => loadSettings().then(() => {
    const c = (S.settings && S.settings.server && S.settings.server.connecting) || {}
    if (c.busy) return setTimeout(poll, 1000)
    if (c.ok) S.sttForm = null
    render()
  })
  setTimeout(poll, 500)
})

document.addEventListener('click', (e) => {
  const at = (sel) => e.target.closest && e.target.closest(sel)
  const poll = (then) => loadSettings().then(() => {
    const w = (S.settings || {}).wrapper || {}
    if (w.busy) return setTimeout(() => poll(then), 700)
    if (then) then(w)
    render()
  })
  const go = (msg, then) => { send(msg); setTimeout(() => poll(then), 400) }
  // Added or updated: the form goes; what was done stays said in Echo's notice.
  const done = (w) => { if (w.added || w.updated) { S.wrapForm = null; S.notice = (wrapperState(w) || {}).text; loadState().then(render) } }
  if (at('[data-wrap-form]')) { S.wrapForm = S.wrapForm && !S.wrapForm.plugin ? null : { command: '', name: '' }; return render() }
  if (at('[data-wrap-cancel]')) { S.wrapForm = null; return render() }
  // A profile's own models for haiku, sonnet and opus: edited in its row, saved together.
  const medit = at('[data-models-edit]')
  if (medit) { S.modelsForm = { id: medit.dataset.modelsEdit }; return render() }
  if (at('[data-models-cancel]')) { S.modelsForm = null; return render() }
  const msave = at('[data-models-save]')
  if (msave) {
    const v = { id: msave.dataset.modelsSave }
    for (const k of ['haiku', 'sonnet', 'opus']) v[k] = (($(`#mdl-${k}`) || {}).value || '').trim()
    // Kept open with what was typed until the profile shows it saved (a name refused leaves it, with Echo's notice).
    S.modelsForm = { id: v.id, v }
    send({ cmd: 'settings_set', key: 'profile_models', value: v })
    return setTimeout(() => loadState().then(() => {
      const p = ai().profiles.find((x) => x.id === v.id)
      const m = (p && p.models) || {}
      if (['haiku', 'sonnet', 'opus'].every((k) => (m[k] || '') === v[k])) S.modelsForm = null
      render()
    }), 400)
  }
  const offer = at('[data-wrap-offer]')
  if (offer) {
    const o = wrapperOffers(ai()).find((x) => x.plugin === offer.dataset.wrapOffer)
    S.wrapForm = { plugin: offer.dataset.wrapOffer, command: offer.dataset.wrapCommand, pluginName: o ? o.plugin_name : '', asked: true }
    return go({ cmd: 'wrapper_check', command: S.wrapForm.command })
  }
  if (at('[data-wrap-check]') && S.wrapForm) {
    if (!S.wrapForm.plugin) S.wrapForm = { command: (($('#wrap-command') || {}).value || '').trim(), name: ($('#wrap-name') || {}).value || '' }
    S.wrapForm.asked = true
    return go({ cmd: 'wrapper_check', command: S.wrapForm.command })
  }
  if (at('[data-wrap-add]') && S.wrapForm) {
    // Exactly what Check showed: the program as typed now and the path it was found at (Echo refuses a mismatch).
    const f = S.wrapForm, ws = wrapperState((S.settings || {}).wrapper) || {}
    if (!f.plugin) { f.command = (($('#wrap-command') || {}).value || '').trim(); f.name = ($('#wrap-name') || {}).value || f.name }
    if (ws.kind !== 'ok' || ws.command !== f.command) { f.asked = false; f.stale = true; return render() }
    return go(f.plugin ? { cmd: 'wrapper_add', plugin: f.plugin, command: f.command, path: ws.path } : { cmd: 'wrapper_add', command: f.command, name: f.name, path: ws.path }, done)
  }
  const up = at('[data-wrap-update]')
  if (up) return go({ cmd: 'wrapper_update', id: up.dataset.wrapUpdate }, done)
})

// ── wrapper-profiles: AI & minutes' wrapper profiles (claude-code-runs R12–R13) — what plugins offer, a profile's
// update from its plugin, and the last check, add or update the page asked for (`st.wrapper`). No DOM: tested by
// wrapper-profiles.test.mjs.
/** The wrappers enabled plugins offer that the user hasn't added, by name. */
const wrapperOffers = (a) => ((a || {}).wrapper_offers || []).filter((o) => !o.profile).sort((x, y) => x.name.localeCompare(y.name) || x.plugin_name.localeCompare(y.plugin_name))
/** A wrapper profile's plugin offering another program now: `{plugin_name, command, from}`, else null. */
const wrapperUpdate = (a, id) => {
  const o = ((a || {}).wrapper_offers || []).find((x) => x.profile === id && x.update)
  return o ? { plugin_name: o.plugin_name, command: o.command, from: o.update } : null
}
/** What every wrapper profile says of itself, in Echo's words. */
const WRAPPER_WARNING = 'It sees and can change everything Echo sends to Claude, call transcripts included.'
/** The last wrapper step, for the form: `{text, kind: 'busy'|'ok'|'error'|'done', path?, version?}`, or null. */
function wrapperState(w) {
  if (!w) return null
  if (w.busy) return { kind: 'busy', text: 'Checking it — where your shell finds it, then its version…' }
  if (w.error) return { kind: 'error', text: w.error }
  if (w.checked) return { kind: 'ok', text: `Found ${w.path} — ${w.version}`, path: w.path, version: w.version, command: w.command }
  if (w.added) return { kind: 'done', text: w.path ? `Added ${w.name} — it starts ${w.path}` : `Added ${w.name}` }
  if (w.updated) return { kind: 'done', text: `It now starts ${w.path}` }
  return null
}
// end of the wrapper-profiles block

// ── own title bar row: Settings → Layout's "Echo's own title bar" (#76, on by default) — on Windows only, by the
// app's platform (`state.platform`), not the browser's: absent on macOS, Linux and in `--serve`, so search can't find it.
const ownTitleBarCard = (st, state) => (state || {}).platform !== 'windows' ? '' : setCard('Window', [setRow("Echo's own title bar",
  (st || {}).own_title_bar ? "No Windows title bar: Echo's top strip moves the window and holds minimize, maximize and close." : "Windows draws its title bar above Echo's top strip.",
  sw('own_title_bar', (st || {}).own_title_bar, "Echo's own title bar"))])
// end of the own-title-bar block
const SETTINGS_BODY = {
  templates: () => templatesSettings(),
  brain: (st) => knowledgeSettings() + brainSettings(st),
  plugins: pluginsSettings,
  general(st) {
    const upd = st.update ? actBtn('update', `Update to ${st.update}…`, '', true) : actBtn('check_updates', 'Check for updates')
    return `<h2>General</h2><p class="set-lead">Start at login and the command line are this computer’s, whichever data folder Echo uses; Claude Code is the one Echo’s PATH finds.</p>` + setCard('', [
      setRow('Data folder', `<code>${esc(st.home)}</code> — calls, transcripts and minutes live here.`, actBtn('change_home', 'Change…'), '', 'Data folder'),
      setRow('Appearance', 'Light or dark for Echo’s window on this computer — or as your system is set.', `<div class="seg" role="group" aria-label="Appearance">${[['', 'System'], ['light', 'Light'], ['dark', 'Dark']].map(([v, l]) => `<button data-theme-set="${v}" aria-pressed="${Theme.chosen() === v}">${l}</button>`).join('')}</div>`),
      setRow('Start at login', TRAY.startAtLogin, sw('autostart', st.autostart, 'Start at login')),
      /Mac/.test(navigator.platform || navigator.userAgent) ? setRow('Hide from the Dock when the window is closed', st.hide_dock_icon ? 'Echo shows in the Dock only while its window is open; otherwise only in the menu bar.' : 'Echo stays in the Dock while it runs: click it to open the window, even when a full menu bar hides its icon.', sw('hide_dock_icon', st.hide_dock_icon, 'Hide from the Dock when the window is closed')) : '',
      draftsRow(st),
      setRow('Command line', st.cli ? '<code>echoctl</code> is on your PATH.' : 'Install <code>echoctl</code> so scripts and Claude Code can drive Echo.', st.cli ? '<span class="set-ok">Installed ✓</span>' : actBtn('install_cli', 'Install…')),
      setRow('Claude Code plugin', "Echo's skills in your own Claude Code: record, minutes, doctor.", claudePluginCtl(st.claude_code)),
      setRow(`Echo ${esc(st.version)}`, st.update ? `Version ${esc(st.update)} is available.` : 'Echo checks for updates every 6 hours.', `<button class="btn ghost" data-wn-open>What's new</button>${upd}`),
    ])
  },
  recording(st) {
    const langs = st.languages || []
    const chips = langs.map((l) => `<span class="chip">${esc(langName(l))}<button data-lang-del="${esc(l)}" aria-label="Remove ${esc(langName(l))}">×</button></span>`).join('')
    const add = `<label class="btn split-lang pick">+ Add<select data-lang-add aria-label="Add a language"><option value=""></option>${LANG_LIST.filter((l) => !langs.includes(l)).map((l) => `<option value="${l}">${esc(langName(l))}</option>`).join('')}</select></label>`
    const frames = st.capture_frames && st.frames_permitted === false ? ` <a href="#" data-act="ask_permissions">Grant…</a>` : ''
    return `<h2>Recording</h2><p class="set-lead">Echo records only the calls you choose, and keeps only text.</p>` +
      setCard('When to record', [
        setRow('Calendar meetings', 'Ask: Echo asks when one starts. Manual: you press Record (or “Record when it starts” on the calendar).', seg('scheduled', st.scheduled, [['ask', 'Ask'], ['manual', 'Manual']], 'Calendar meetings')),
        setRow('Other Teams calls', 'Calls not on your calendar.', seg('unscheduled', st.unscheduled, [['ask', 'Ask'], ['manual', 'Manual']], 'Other Teams calls')),
      ]) +
      setCard('Languages', [
        setRow('Languages you record in', `The ones offered when a call starts. Echo never guesses a call's language: it is transcribed in the one chosen, and the model best at it is used.<div class="chips">${chips}${add}</div>`, ''),
        setRow('Default language', 'For calls you start without picking one, and for those Echo records by itself.', langs.length > 1 ? `<label class="btn split-lang pick">${esc(langName(langs[0]))}${CARET}<select data-lang-default aria-label="Default language">${langs.map((l, i) => `<option value="${esc(l)}"${i ? '' : ' selected'}>${esc(langName(l))}</option>`).join('')}</select></label>` : `<span class="set-sub">${esc(langName(langs[0] || 'en'))}</span>`),
      ]) +
      setCard('During a call', [
        setRow("Follow Teams' mute", st.capture_ui === false ? "Off with Read the Teams window: Echo can't see Teams' mute button, so your track records while you're muted in Teams." : "Your track records silence while you're muted in Teams (it needs Read the Teams window).", st.capture_ui === false ? '<button class="sw" role="switch" aria-checked="false" aria-label="Follow Teams\' mute" disabled></button>' : sw('follow_app_mute', st.follow_app_mute, "Follow Teams' mute")),
        setRow('Read the Teams window', "Who is speaking, mute sync and the call's name, read from Teams' call window (Accessibility); and Chrome's window titles, only to tell a Google Meet tab (never kept). Teams answers every read itself, which costs it about 10–20% of its CPU: on a slower computer, turn it off. It applies from the next call; Teams gets its CPU back fully once Echo or Teams restarts. Off, calls are still recorded and transcribed as you and the others, without names; two calls straight after each other may become one recording, and a call started from a chat isn't named after it.", sw('capture_ui', st.capture_ui, 'Read the Teams window')),
        setRow('Meeting chat', "The meeting's Teams chat, with Microsoft 365 connected.", sw('capture_chat', st.capture_chat, 'Meeting chat')),
        setRow('Shared screens', `Key frames while someone else shares (needs Screen Recording; snapshots read Teams' window too, apart from the switch above).${frames}`, sw('capture_frames', st.capture_frames, 'Shared screens')),
      ])
  },
  transcription(st) {
    const m = st.models || { models: [] }
    const dl = st.downloading
    const rows = (m.models || []).map((x) => {
      const tags = [x.active ? '<span class="tag use">In use</span>' : '', x.recommended ? '<span class="tag rec">Recommended for this computer</span>' : '', x.runnable === false ? '<span class="tag">Not in this build</span>' : ''].join('')
      let ctl
      if (dl && dl.name === x.name) ctl = `<span class="set-sub">Downloading… ${dl.pct}%</span>`
      else if (x.downloaded) ctl = (x.active || x.runnable === false ? '' : actBtn('use_model', 'Use', x.name)) + (x.active ? '' : actBtn('remove_model', 'Remove', x.name))
      else if (x.runnable === false) ctl = ''
      else ctl = dl ? '<span class="set-sub">Another download is running</span>' : actBtn('download_model', `Download (${x.size_mb} MB)`, x.name)
      const bar = dl && dl.name === x.name ? `<div class="bar" role="progressbar" aria-valuenow="${dl.pct}" aria-valuemin="0" aria-valuemax="100" aria-label="${esc(`Downloading ${x.name}`)}"><span style="width:${dl.pct}%"></span></div>` : ''
      return setRow(`${esc(x.name)} ${tags}`, `${esc(x.summary || '')} · ${x.size_mb} MB${bar}`, ctl, '', x.name)
    })
    const srv = st.server || null
    return `<h2>Transcription</h2><p class="set-lead">On this computer — or on one you connect to yourself. Never a cloud service.${st.engine ? ` Speech engine: ${esc(st.engine)}.` : ''}</p>` +
      setCard('Speech models', rows) +
      (srv ? setCard('Transcription server', [
        setRow('Use a server', serverLine(srv), srv.client ? actBtn('disconnect_server', 'Disconnect') : actBtn('connect_server', 'Echo server…') + `<button class="btn" data-stt-form>OpenAI-compatible…</button>`),
        ...(!srv.client && S.sttForm ? [sttForm(srv)] : []),
        setRow('Transcribe for other computers', srv.serving ? `Shared at <code>${esc(srv.url || '')}</code> — key <code>${esc(srv.key || '')}</code>` : "Share this computer's model on your network.", `<button class="sw${srv.serving ? ' on' : ''}" role="switch" aria-checked="${!!srv.serving}" aria-label="Transcribe for other computers" data-act="toggle_serving"></button>`),
      ]) : '') +
      setCard('Audio', [
        setRow('Echo cancellation on speakers', "Removes the others' voices from your mic when you don't wear headphones.", sw('echo_cancel', st.echo_cancel, 'Echo cancellation')),
        setRow('Keep audio files', 'Off: the recordings are deleted once the transcript is written — text only.', sw('keep_audio', st.keep_audio, 'Keep audio files')),
      ])
  },
  ai(st) {
    const a = ai()
    const kind = (p) => p.kind === 'claude' ? 'Claude account' : p.kind === 'local' ? `Model · ${esc(p.host || 'this computer')}` : p.kind === 'wrapper' ? 'Wrapper' : 'Built in'
    const ws = wrapperState(st.wrapper)
    const busy = !!(ws && ws.kind === 'busy')
    const localDefault = a.profiles.find((p) => p.id === a.default && p.kind === 'local' && !p.host)
    const rows = a.profiles.map((p) => {
      const s = p.signin || {}
      const out = p.kind === 'claude' && s.state === 'signed_out'
      const up = p.kind === 'wrapper' ? wrapperUpdate(a, p.id) : null
      const who = p.kind === 'claude' ? (s.account ? `Signed in as ${esc(s.account)}` : out ? '<span class="warn">Not signed in — its calls wait for it</span>' : esc(p.account || ''))
        : p.kind === 'wrapper' ? `Starts <code>${esc(p.command || '')}</code> in place of Claude Code${p.offered_by ? ` · offered by ${esc(p.offered_name || p.offered_by)}` : ''}. ${WRAPPER_WARNING}`
        : esc(profileWhere(p))
      const upLine = up ? `<div class="warn">${esc(up.plugin_name)}'s update offers <code>${esc(up.command)}</code> instead — this profile keeps starting its program until you take it.</div>` : ''
      const isDef = p.id === a.default
      const m = p.models || {}
      const mapped = ['haiku', 'sonnet', 'opus'].filter((k) => m[k]).map((k) => `${k} → ${esc(m[k])}`).join(' · ')
      const draft = S.modelsForm && S.modelsForm.id === p.id ? S.modelsForm.v || m : null
      const modelsLine = p.kind === 'system' ? '' : draft
        ? `<div class="stt-form">${['haiku', 'sonnet', 'opus'].map((k) => `<label class="set-sub">${k[0].toUpperCase() + k.slice(1)} → <input class="set-in" id="mdl-${k}" value="${esc(draft[k] || '')}" placeholder="${k}" aria-label="${k[0].toUpperCase() + k.slice(1)} runs" spellcheck="false"></label>`).join('')}<button class="btn primary" data-models-save="${esc(p.id)}">Save</button><button class="btn ghost" data-models-cancel>Cancel</button></div>`
        : `<div class="set-sub">${mapped ? `Models: ${mapped}` : 'Models: Claude\'s names as they are'} <button class="btn ghost" data-models-edit="${esc(p.id)}">Models…</button></div>`
      const ctl = (out ? `<button class="btn primary" data-signin="${esc(p.id)}">Sign in…</button>` : '') + (up ? `<button class="btn primary" data-wrap-update="${esc(p.id)}"${busy ? ' disabled' : ''}>Use it</button>` : '') + (isDef ? '' : `<button class="btn" data-set="default_profile" data-val='${esc(JSON.stringify(p.id))}'>Make default</button>`)
      return setRow(`${esc(p.name)} ${isDef ? '<span class="tag rec">Default for new calls</span>' : ''}<span class="tag">${kind(p)}</span>`, `${who}${p.notes_model ? ` · minutes: ${esc(p.notes_model)}` : ''}${p.live_model ? ` · live notes: ${esc(p.live_model)}` : ''}${upLine}${modelsLine}`, ctl, '', p.name)
    })
    const offers = wrapperOffers(a).map((o) => setRow(`${esc(o.name)} <span class="tag">Offered by ${esc(o.plugin_name)}</span>`,
      `Starts <code>${esc(o.command)}</code> — a program you install yourself — in place of Claude Code. ${WRAPPER_WARNING}`,
      `<button class="btn" data-wrap-offer="${esc(o.plugin)}" data-wrap-command="${esc(o.command)}"${busy ? ' disabled' : ''}>Add…</button>`, '', o.name))
    const f = S.wrapForm
    // Only what this form asked for: an earlier check, add or update says nothing here.
    const mine = !!(f && f.asked && ws)
    const msg = mine ? `<div class="${ws.kind === 'error' ? 'error' : ws.kind === 'busy' ? 'set-sub' : ''}">${esc(ws.text)}</div>`
      : f && f.stale ? '<div class="set-sub">The program changed since its check — Check it again.</div>' : ''
    const ready = mine && ws.kind === 'ok' && ws.command === f.command
    const form = !f ? [] : [setRow(f.plugin ? `Add the wrapper ${esc(f.pluginName || f.plugin)} offers` : 'Add a wrapper',
      `A program you installed that starts Claude Code its own way — a name on your PATH or its full path. Echo starts it in place of Claude Code, on your own Claude Code setup. ${WRAPPER_WARNING}` +
      `<div class="stt-form">${f.plugin ? `<code>${esc(f.command)}</code>` : `<input class="set-in" id="wrap-command" value="${esc(f.command)}" placeholder="acme-claude" aria-label="Program" spellcheck="false"><input class="set-in" id="wrap-name" value="${esc(f.name || '')}" placeholder="name (optional)" aria-label="Name">`}` +
      `<button class="btn" data-wrap-check${busy ? ' disabled' : ''}>Check</button><button class="btn primary" data-wrap-add${ready && !busy ? '' : ' disabled'}>Add profile</button><button class="btn ghost" data-wrap-cancel>Cancel</button></div>${msg}`, '', 'Add a wrapper')]
    const add = '<div class="set-sub" style="padding:10px 14px">Add a profile: <code>echoctl ai add-claude &lt;name&gt;</code> (a Claude account), <code>echoctl ai add-local &lt;name&gt; --url …</code> (a model on this computer), <code>echoctl ai add-wrapper &lt;name&gt; --command …</code> or <button class="btn ghost" data-wrap-form>Add a wrapper…</button> (a program you installed that starts Claude Code) — or ask Claude in the window.</div>'
    return `<h2>AI &amp; minutes</h2><p class="set-lead">Which model and account writes each call's live notes and minutes. A call keeps the profile it was recorded with.</p>` +
      `<section class="set-sec" aria-label="Profiles"><h3>Profiles</h3><div class="set-card">${rows.join('')}${offers.join('')}${form.join('')}${add}</div></section>` +
      setCard('Minutes', [
        setRow('Write minutes automatically', "15 min after a call ends (or after your last note), with the call's profile.", sw('auto_minutes', st.auto_minutes, 'Write minutes automatically')),
        setRow("Keep threads' work current", "After a thread's calls, Claude reads them into its stories, tasks, decisions, risks and questions — with the thread's AI profile.", sw('project_sync', st.project_sync, "Keep threads' work current")),
        setRow('Live notes during calls', "A running summary and the Live tab's questions, with the call's profile." + (localDefault ? ` Not with ${esc(localDefault.name)}: its model runs on this computer, where it would slow your calls down.` : ''), sw('live_notes', a.live_notes, 'Live notes')),
        setRow('Minutes model', 'The model that writes minutes for the default profile.', actBtn('notes_model', 'Change…')),
        setRow('Where Claude Code runs for minutes', st.notes_in ? `<code>${esc(st.notes_in)}</code> — its .claude/settings.json applies.` : 'The data folder.', actBtn('notes_choose', 'Choose…') + (st.notes_in ? actBtn('notes_anywhere', 'Use the data folder') : '')),
        setRow('Also write a minutes page file', "An HTML file in the call's folder, for sharing.", sw('minutes_page', st.minutes_page, 'Minutes page file')),
      ]) +
      setCard("Claude's permissions", [
        setRow('Claude in the window', `Auto: Claude Code's auto mode (<code>--permission-mode auto</code>) decides each action without asking. Skip all permission checks: Claude starts with <code>--dangerously-skip-permissions</code> and runs any command and tool without asking — it can talk to your other sessions, close your tasks in other systems and send. Ask each time: Claude Code asks you first. ${st.folder_permissions === 'skip' ? '<span class="warn">Skips every check.</span> ' : ''}Takes effect the next time the session starts.`,
          seg('folder_permissions', st.folder_permissions, [['ask', 'Ask each time'], ['auto', 'Auto'], ['skip', 'Skip all permission checks']], 'Claude in the window: permissions'), '', 'Claude in the window'),
        ...[['routine_permissions', 'Routines acting on their own', "A routine's outcome lists what it changed and sent; one you set in its own editor keeps its choice."], ['yolo_permissions', 'Yolo acting on its own', "A Yolo run's outcome lists what it changed and sent."]].map(([key, label, whose]) =>
          setRow(label, `Auto: Claude Code's auto mode (<code>--permission-mode auto</code>) decides each action. Skip all permission checks: <code>--dangerously-skip-permissions</code>, nothing checked. ${whose} ${st[key] === 'skip' ? '<span class="warn">Skips every check.</span> ' : ''}`,
            seg(key, st[key], [['auto', 'Auto'], ['skip', 'Skip all permission checks']], `${label}: permissions`), '', label)),
        // What Auto gets on each profile: auto mode needs a recent Claude model; elsewhere edits are accepted.
        `<div class="set-sub" style="padding:10px 14px" data-auto-on>Auto on your profiles: ${(st.auto_on_profiles || []).map(([name, got]) => `${esc(name)} — ${esc(got)}`).join(' · ')}</div>`,
      ])
  },
  calendar(st) {
    return `<h2>Calendar &amp; chat</h2><p class="set-lead">Read-only, through Echo's own Microsoft 365 sign-in.</p>` + setCard('Microsoft 365', [
      setRow('Account', st.calendar ? `Signed in as ${esc(st.calendar)}` : 'Not connected: the calendar shows only what your tray knows about today.', st.calendar ? actBtn('calendar_sync', 'Sync now') + actBtn('calendar_logout', 'Sign out') : actBtn('calendar_login', 'Sign in…', '', true), '', 'Account'),
      setRow('Meeting chat', "Each recorded meeting's Teams chat, kept with the call.", sw('capture_chat', st.capture_chat, 'Meeting chat'), '', 'Meeting chat'),
      // Colleagues' photos: a sign-in of their own (the organisation may refuse it), then a switch.
      setRow("Colleagues' photos", !st.calendar ? 'In place of initials, once Microsoft 365 is connected.'
        : (st.photos || [])[1] ? 'In place of initials: colleagues from your own domain, kept on this computer.'
        : "In place of initials. Echo asks Microsoft 365 for one more permission, to read your colleagues' photos — your organisation may refuse it.",
        !st.calendar ? '' : (st.photos || [])[1] ? sw('people_photos', (st.photos || [])[0], "Colleagues' photos") : actBtn('photos_login', 'Allow…'), '', "Colleagues' photos"),
      setRow('App registration', st.graph_custom ? "Your organisation's own app." : 'The default (CodeMie). Your IT can register another.', actBtn('calendar_client_id', 'Change…'), '', 'App registration'),
      setRow('OneDrive on this computer', st.onedrive === 'none' ? 'Off: Echo treats no folder here as OneDrive.'
        : st.onedrive_now ? `<code>${esc(st.onedrive_now)}</code>${st.onedrive ? '' : ' (found by Echo)'}. Claude in Echo doesn't make a folder there that OneDrive keeps online but doesn't sync to this computer.`
        : 'Not found on this computer.',
        actBtn('onedrive_choose', 'Choose…') + (st.onedrive ? `<button class="btn" data-set="onedrive" data-val='"auto"'>Find it</button>` : '') + (st.onedrive !== 'none' ? `<button class="btn" data-set="onedrive" data-val='"none"'>Off</button>` : ''), '', 'OneDrive on this computer'),
    ])
  },
  deliver() {
    if (!S.deliver) { if (!S.deliverLoading) { S.deliverLoading = true; loadDeliver().then(() => { S.deliverLoading = false; render() }) } return '<h2>Sends to you</h2><div class="skeleton" style="padding:20px"><i></i><i></i></div>' }
    return `<h2>Sends to you</h2><p class="set-lead">Where Echo sends you things — routines’ answers, tests. Only to you: your own Teams chat, your mailbox, and the ways plugins add.</p>${deliverCard(S.deliver)}`
  },
  me(st) {
    const me = st.me || {}
    return `<h2>About me</h2><p class="set-lead">So Echo knows which voice is you, whose work is whose, and how to write to you.</p>` + setCard('', [
      setRow('Name', '', `<input class="search set-input" id="me-name" value="${esc(me.name || '')}" placeholder="Your name" aria-label="Your name">`),
      setRow('Email', 'Tells your colleagues from external people.', `<input class="search set-input" id="me-email" value="${esc(me.email || '')}" placeholder="you@company.com" aria-label="Your email">`),
    ]) + meProfileCard(st.me_profile)
  },
  layout(st) {
    const known = PANEL_VIEWS
    const rows = known.filter((v) => v.id !== 'shell').map((v) => {
      const area = Layout.area(v.id, known)
      return setRow(`${v.icon || ''} ${esc(v.label)}`, '', `<div class="seg" role="group" aria-label="${esc(v.label)}">${['right', 'bottom'].map((a) => `<button data-layout-move="${v.id}" data-to="${a}" aria-pressed="${area === a}">${a === 'right' ? 'Right' : 'Bottom'}</button>`).join('')}</div>`)
    })
    const bar = ownTitleBarCard(st, S.state)
    return `<h2>Layout</h2><p class="set-lead">Where each view opens. Also from a panel's own move button; drag a panel's edge to resize it.</p>` + bar + setCard('Panel views', rows) +
      setCard('', [setRow('Reset the layout', 'Every view back to its own place and size.', '<button class="btn" data-layout-reset>Reset</button>')])
  },
  health(st) {
    const rows = (st.health || []).map((c) => {
      const ok = c.status === 'granted'
      const mark = ok ? '<span class="set-ok">✓</span>' : c.status === 'denied' ? '<span class="warn">✗</span>' : '<span class="set-sub">?</span>'
      return setRow(`${mark} ${esc(c.name)}`, ok ? esc(c.why) : `${esc(c.why)}<div class="set-sub">${esc(c.how)}</div>`, ok || !c.url ? '' : actBtn('open_settings_url', 'Open settings', c.url), '', c.name)
    })
    const stale = st.stale_plugins || []
    const plugin = setCard('Claude Code', claudeRows(st.claude_code || { checking: true }).concat(stale.length ? [setRow('<span class="warn">✗</span> An older Echo plugin', `${stale.map((s) => `${esc(s.version)} (${esc(s.scope)}${s.project ? ', ' + esc(String(s.project).split(/[\\/]/).pop()) : ''})`).join(', ')} — an older one can hold calls back from Write minutes. Restart Claude Code after updating.`, '<button class="btn" data-cmd-ipc="update_plugins">Update it</button>', '', 'An older Echo plugin')] : []))
    return `<h2>Health</h2><p class="set-lead">What Echo needs from this computer — the same whichever data folder it uses.</p>` + plugin + setCard('Permissions', rows) +
      setCard('', [setRow('Ask again', 'macOS asks for each permission Echo is missing.', actBtn('ask_permissions', 'Ask for permissions') + actBtn('recheck', 'Check again'))])
  },
}

