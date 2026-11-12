// ── Plugins' client halves (docs/plugins-design.md, v4) ──
// Each enabled plugin's `client` module is loaded into this page (trusted, as in dsh-desktop) and its
// `apply(ctx)` adds to Echo: buttons in Echo's own places (slots), panel views, its card in Settings →
// Plugins, ⌘K actions. `ctx.call` runs a command of its host half (`/api/plugin/<id>/call/<command>`).
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
}
// `knowledge.tab`: a tab of the Knowledge activity ({id, label, render() → HTML, after(box)}), `#brain/<plugin>:<id>/<its route>`.
// `knowledge.settings`: rows in Settings → Knowledge ({render() → setRow(…) HTML}), above Recall's card.
// `task.actions` also on a plugin's work row, for an item whose `when` is true for {kind: 'plugin', source, row}.
// `thread.profile`: a row of a thread's Profile, under its Sources ({id, label, render(thread) → HTML, after(box, thread)}),
// `thread` = {id, name, client}.
const SLOTS = ['call.menu', 'calendar.event', 'task.actions', 'knowledge.tab', 'knowledge.settings', 'thread.profile']
// ── thread slot: the plugins' rows of a thread's Profile, each drawn on its own — one that throws is left out (and
// logged), the Profile draws without it.
function threadSlotRows(items, thread, log) {
  const out = []
  for (const x of items || []) {
    try {
      const html = x.render(thread)
      out.push({ key: `${x.plugin}:${x.id}`, label: String(x.label || x.id), html: html == null ? '' : String(html), after: x.after })
    } catch (e) { log(`${x.plugin}: ${x.label || x.id}: ${(e && e.message) || e}`) }
  }
  return out
}
// end of the thread slot block
/** The thread a `thread.profile` row is drawn for. */
const slotThread = (p) => ({ id: p.id, name: p.name, client: p.client || null })
function pluginCtx(id) {
  const add = (list, item) => { list.push({ ...item, plugin: id }); render() }
  const owned = Plugins.owned[id] = Plugins.owned[id] || { listeners: [], styles: [] }
  return {
    id,
    /** A document listener that goes when the plugin is switched off, updated or removed. */
    listen: (type, fn, opts) => { document.addEventListener(type, fn, opts); owned.listeners.push([type, fn, opts]) },
    call: async (command, args = {}) => {
      const r = await get(`api/plugin/${encodeURIComponent(id)}/call/${encodeURIComponent(command)}?args=${encodeURIComponent(JSON.stringify(args))}`)
      if (r && r.error) throw new Error(r.error)
      return r ? r.value : null
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
     *  render() → HTML of the page, after(box) — after each render (draw a graph)}. */
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
    /** Open one of Echo's own panel views, e.g. `claude-folder` (the Claude · Echo folder session). */
    openEchoPanel: (view) => { if (view.includes(':')) return; go('calls'); setTimeout(() => { Layout.show(view, panelViews()); layoutChanged() }, 50) },
    /** Where the window is: the activity and the Settings section (`#settings/<plugin>:<id>`). */
    where: () => ({ view: S.view, section: S.setSection, route: S.plRoute || '', tab: S.view === 'brain' ? S.br.tab : undefined }),
    go: (hash) => go(hash),
    palette: { add: (item) => add(Plugins.palette, item) },
    /** Echo's own pieces, so a plugin looks like Echo. */
    ui: {
      esc, setRow, setCard, sw, ICON, relDay, ago, notice: (text) => { S.notice = text; render() },
      /** Its CSS in the page, taken out when it unloads. */
      style: (css) => { const el = document.createElement('style'); el.dataset.plugin = id; el.textContent = String(css); document.head.appendChild(el); owned.styles.push(el) },
      /** Echo's graph (Sigma over graphology, as the Brain's): draw(box, key, build, {click}), focus, select, find. */
      graph: GraphView,
      newGraph: () => new graphology.Graph(),
    },
    render: () => { if (!Plugins.drawing) render() },
  }
}
/** Load the client halves of the enabled plugins not loaded yet (or at another version). */
async function loadPluginClients(snapshot) {
  let p = snapshot
  if (!p) try { p = await get('api/plugins') } catch { return }
  const want = (p.installed || []).filter((x) => x.enabled && x.client)
  for (const id of Object.keys(Plugins.loaded)) if (!want.some((x) => x.id === id && x.version === Plugins.loaded[id])) unloadPlugin(id)
  for (const x of want) {
    if (Plugins.loaded[x.id]) continue
    Plugins.loaded[x.id] = x.version
    try {
      const m = await import(new URL(`plugin/${encodeURIComponent(x.id)}/${x.client}?v=${encodeURIComponent(x.version)}`, location.href).href)
      if (typeof m.apply !== 'function') throw new Error(`${x.client} exports no apply(ctx)`)
      await m.apply(pluginCtx(x.id))
      Plugins.applied[x.id] = true
      delete Plugins.errors[x.id]
    } catch (e) { Plugins.errors[x.id] = String((e && e.message) || e) }
  }
  render()
}
function unloadPlugin(id) {
  for (let i = ACTIVITIES.length - 1; i >= 0; i--) if (ACTIVITIES[i].plugin === id) ACTIVITIES.splice(i, 1)
  // Switched off or removed (not an update, which loads it again right away): its page goes.
  setTimeout(() => { if (S.view && S.view.startsWith(id + ':') && !ACTIVITIES.some((a) => a.id === S.view)) { S.view = 'calls'; go('calls') } }, 1500)
  const owned = Plugins.owned[id]
  if (owned) {
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
  delete Plugins.loaded[id]
  delete Plugins.applied[id]
}
/** A slot's items for `target`: those whose `when` says yes — a plugin's work row (`kind: 'plugin'`) only to the items
 *  that ask for it (a `when` true for it), never to one written for Echo's tasks. */
const slotFor = (slot, target) => (Plugins.slots[slot] || []).filter((x) => { try { return x.when ? x.when(target) : !(target && target.kind === 'plugin') } catch { return false } })
/** A slot's buttons for `target`: `menu` (menu items) or `buttons`; `key` when the slot is drawn for several targets at
 *  once (each row its own). */
function slotItems(slot, target, style = 'buttons', key = '') {
  const items = slotFor(slot, target)
  if (!items.length) return ''
  Plugins.targets[slot + (key ? `#${key}` : '')] = target
  const k = key ? ` data-plug-key="${esc(key)}"` : ''
  return items.map((x, i) => style === 'menu'
    ? `<button class="menu-item" role="menuitem" data-plug-slot="${slot}" data-plug-i="${i}"${k}>${x.icon || ''}${esc(x.label)}</button>`
    : `<button class="btn" data-plug-slot="${slot}" data-plug-i="${i}"${k} title="${esc(x.title || x.label)}">${x.icon || ''}${esc(x.label)}</button>`).join('')
}
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-plug-slot]')
  if (!t) return
  const slot = t.dataset.plugSlot, target = Plugins.targets[slot + (t.dataset.plugKey ? `#${t.dataset.plugKey}` : '')]
  const items = slotFor(slot, target)
  const item = items[+t.dataset.plugI]
  S.menuOpen = false
  if (item) Promise.resolve().then(() => item.run(target)).catch((err) => { S.notice = `${item.label}: ${(err && err.message) || err}`; render() })
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
      setRow(`Echo ${esc(st.version)}`, st.update ? `Version ${esc(st.update)} is available.` : 'Echo checks for updates every 6 hours.', upd),
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
        setRow('Read the Teams window', "Who is speaking, mute sync and the call's name, read from Teams' call window (Accessibility). Teams answers every read itself, which costs it about 10–20% of its CPU: on a slower computer, turn it off. It applies from the next call; Teams gets its CPU back fully once Echo or Teams restarts. Off, calls are still recorded and transcribed as you and the others, without names; two calls straight after each other may become one recording, and a call started from a chat isn't named after it.", sw('capture_ui', st.capture_ui, 'Read the Teams window')),
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
    const kind = (p) => p.kind === 'claude' ? 'Claude account' : p.kind === 'local' ? `Model · ${esc(p.host || 'this computer')}` : 'Built in'
    const localDefault = a.profiles.find((p) => p.id === a.default && p.kind === 'local' && !p.host)
    const rows = a.profiles.map((p) => {
      const s = p.signin || {}
      const out = p.kind === 'claude' && s.state === 'signed_out'
      const who = p.kind === 'claude' ? (s.account ? `Signed in as ${esc(s.account)}` : out ? '<span class="warn">Not signed in — its calls wait for it</span>' : esc(p.account || '')) : esc(profileWhere(p))
      const isDef = p.id === a.default
      const ctl = (out ? `<button class="btn primary" data-signin="${esc(p.id)}">Sign in…</button>` : '') + (isDef ? '' : `<button class="btn" data-set="default_profile" data-val='${esc(JSON.stringify(p.id))}'>Make default</button>`)
      return setRow(`${esc(p.name)} ${isDef ? '<span class="tag rec">Default for new calls</span>' : ''}<span class="tag">${kind(p)}</span>`, `${who}${p.notes_model ? ` · minutes: ${esc(p.notes_model)}` : ''}${p.live_model ? ` · live notes: ${esc(p.live_model)}` : ''}`, ctl, '', p.name)
    })
    const add = '<div class="set-sub" style="padding:10px 14px">Add a profile: <code>echoctl ai add-claude &lt;name&gt;</code> (a Claude account) or <code>echoctl ai add-local &lt;name&gt; --url …</code> (a model on this computer) — or ask Claude in the window.</div>'
    return `<h2>AI &amp; minutes</h2><p class="set-lead">Which model and account writes each call's live notes and minutes. A call keeps the profile it was recorded with.</p>` +
      `<section class="set-sec" aria-label="Profiles"><h3>Profiles</h3><div class="set-card">${rows.join('')}${add}</div></section>` +
      setCard('Minutes', [
        setRow('Write minutes automatically', "15 min after a call ends (or after your last note), with the call's profile.", sw('auto_minutes', st.auto_minutes, 'Write minutes automatically')),
        setRow("Keep threads' work current", "After a thread's calls, Claude reads them into its stories, tasks, decisions, risks and questions — with the thread's AI profile.", sw('project_sync', st.project_sync, "Keep threads' work current")),
        setRow('Live notes during calls', "A running summary and the Live tab's questions, with the call's profile." + (localDefault ? ` Not with ${esc(localDefault.name)}: its model runs on this computer, where it would slow your calls down.` : ''), sw('live_notes', a.live_notes, 'Live notes')),
        setRow('Minutes model', 'The model that writes minutes for the default profile.', actBtn('notes_model', 'Change…')),
        setRow('Where Claude Code runs for minutes', st.notes_in ? `<code>${esc(st.notes_in)}</code> — its .claude/settings.json applies.` : 'The data folder.', actBtn('notes_choose', 'Choose…') + (st.notes_in ? actBtn('notes_anywhere', 'Use the data folder') : '')),
        setRow('Also write a minutes page file', "An HTML file in the call's folder, for sharing.", sw('minutes_page', st.minutes_page, 'Minutes page file')),
      ]) +
      setCard('Claude in the window', [
        setRow('Claude in the window: no permission prompts', `Starts it with <code>--dangerously-skip-permissions</code>: Claude runs any command and tool without asking — it can talk to your other sessions and close your tasks in other systems. ${st.folder_skip_permissions ? '<span class="warn">On.</span> ' : ''}Takes effect the next time the session starts.`, sw('folder_skip_permissions', st.folder_skip_permissions, 'Claude in the window: no permission prompts'), '', 'Claude in the window: no permission prompts'),
      ])
  },
  calendar(st) {
    return `<h2>Calendar &amp; chat</h2><p class="set-lead">Read-only, through Echo's own Microsoft 365 sign-in.</p>` + setCard('Microsoft 365', [
      setRow('Account', st.calendar ? `Signed in as ${esc(st.calendar)}` : 'Not connected: the calendar shows only what your tray knows about today.', st.calendar ? actBtn('calendar_sync', 'Sync now') + actBtn('calendar_logout', 'Sign out') : actBtn('calendar_login', 'Sign in…', '', true), '', 'Account'),
      setRow('Meeting chat', "Each recorded meeting's Teams chat, kept with the call.", sw('capture_chat', st.capture_chat, 'Meeting chat'), '', 'Meeting chat'),
      setRow('App registration', st.graph_custom ? "Your organisation's own app." : 'The default (CodeMie). Your IT can register another.', actBtn('calendar_client_id', 'Change…'), '', 'App registration'),
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

