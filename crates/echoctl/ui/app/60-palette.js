// ── ⌘K palette: actions in context. Its actions are the window's own — each runs as the click on
// the button that does it elsewhere (`press`), so a palette action never differs from the button.
// The target is the open call, the selected calendar event, or nothing (Echo's own actions). ───
const PAL = { open: false, query: '', sel: 0, target: null, items: [] }
/** Run a window action the way its button does: a click on an element with these `data-` values. */
function press(data) {
  const b = document.createElement('button')
  for (const [k, v] of Object.entries(data)) b.dataset[k] = v
  b.hidden = true
  document.body.appendChild(b)
  b.click()
  b.remove()
}
/** What the palette acts on now. */
function paletteTarget() {
  if (S.view === 'calendar' && S.cal.selected && S.cal.data) {
    const e = (S.cal.data.events || []).find((x) => x.id === S.cal.selected)
    if (e) return { kind: 'event', e, label: e.subject || 'Event' }
  }
  if (S.view === 'calls' && S.selected && S.call && !S.call.error) return { kind: 'call', c: S.call, label: S.call.title }
  if (S.view === 'calls' && S.upSel) return { kind: 'upcoming', id: S.upSel, label: (S.upPrep && S.upPrep.meeting && S.upPrep.meeting.subject) || 'Coming up' }
  if (S.view === 'today' && S.today) return { kind: 'today', d: S.today, label: 'Today' }
  return null
}
/** Every action for the target, Echo's own, and where to go. `disabled`: why it can't run now. */
function paletteItems(target) {
  const items = []
  const add = (group, label, run, extra = {}) => items.push({ group, label, run, ...extra })
  const st = S.state || {}
  if (target && target.kind === 'call') {
    const c = target.c, busy = c.notes_run && (c.notes_run.state === 'queued' || c.notes_run.state === 'running')
    const transcribed = !['recording', 'transcribing', 'failed', 'audio'].includes(c.status)
    for (const [id, label] of callTabs({ recording: c.status === 'recording', chat: (c.chat || []).length, frames: (c.frames || []).length })) {
      add('This call', `Open ${label}`, () => press({ tab: id }), { keywords: 'tab show' })
    }
    add('This call', c.notes ? 'Write minutes again' : 'Write minutes', () => press({ cmd: 'write-notes' }), { keywords: 'notes update claude', disabled: !transcribed ? 'after the transcript is ready' : busy ? 'minutes are being written' : null })
    add('This call', 'Ask Claude about this call', () => press({ cmd: 'ask-claude' }), { keywords: 'question terminal session' })
    add('This call', 'Add a note', () => { Layout.show('notes', panelViews()); layoutChanged(); setTimeout(() => { const el = $('#note-input'); if (el) el.focus() }, 50) }, { keywords: 'my notes comment' })
    add('This call', 'Show the call folder', () => press({ cmd: 'reveal' }), { keywords: 'finder files' })
    add('This call', 'Open minutes in browser', () => press({ cmd: 'open-minutes' }), { disabled: c.notes ? null : 'no minutes yet' })
    add('This call', 'Delete this call…', () => press({ cmd: 'ask-delete' }), { keywords: 'remove trash' })
  }
  if (target && target.kind === 'event') {
    const e = target.e, es = eventState(e, new Date()), kind = eventAction(e, es)
    if (kind === 'open-notes') add('This event', 'Open its notes', () => press({ call: e.recorded.name }))
    if (kind === 'scheduled') add('This event', "Don't record it", () => press({ unschedule: e.id }))
    if (kind === 'join') add('This event', 'Join and record', () => press({ join: e.id }))
    if (kind === 'join' || kind === 'record') add('This event', es.live ? 'Record it' : 'Record when it starts', () => press({ schedule: e.id }))
    if (kind === 'import') add('This event', 'Import Teams transcript', () => press({ calImport: e.id }), { keywords: 'missed unrecorded transcript teams', disabled: e.import && e.import.busy ? 'importing now' : null })
    if (!kind) add('This event', 'Record it', () => {}, { disabled: e.cancelled ? 'cancelled' : e.declined ? 'you declined it' : es.past ? 'it is over' : !e.online ? 'not a Teams meeting' : 'not possible' })
  }
  if (target && target.kind === 'event' && target.e.prep) {
    const p = target.e.prep
    if (p.ready) add('This event', 'Show its prep', () => press({ evPrep: p.id }), { keywords: 'brief prepare meeting' })
    if (!p.writing && !eventState(target.e, new Date()).past) add('This event', p.ready ? 'Prepare it again' : 'Prepare for this meeting', () => press({ prepWrite: p.id, prepOpen: '1' }), { keywords: 'prep brief before meeting' })
  }
  if (target && target.kind === 'upcoming') {
    const p = S.upPrep && !S.upPrep.none
    add('This meeting', p ? 'Write the prep again' : 'Prepare this meeting', () => press({ prepWrite: target.id }), { keywords: 'prep brief before', disabled: S.upPrep && S.upPrep.writing ? 'the prep is being written' : null })
  }
  if (target && target.kind === 'today') {
    const d = target.d, now = new Date()
    // Needs you now; meetings still to come: their prep; each task this week: close it, or ask Claude to change it.
    for (const x of (d.needs && d.needs.items) || []) {
      const name = `${x.from || ''} — ${x.line || ''}`
      if (x.source === 'mail') add('Needs you now', `Read and reply: ${name}`, () => press({ needsRead: x.id }), { keywords: 'mail answer' })
      else if (x.app_url || x.chat) add('Needs you now', `Reply in Teams: ${name}`, () => press({ needsOpen: x.app_url || '', chat: x.chat || '' }), { keywords: 'teams answer chat' })
      add('Needs you now', `Claude: ${name}`, () => press({ needsClaude: x.id }), { keywords: 'ask work on it yolo skill' })
      add('Needs you now', `I'll do it: ${name}`, () => press({ needsTodo: x.id }), { keywords: 'task todo' })
      add('Needs you now', `Ignore: ${name}`, () => press({ needsIgnore: x.id }), { keywords: 'dismiss hide skip' })
      add('Needs you now', `Won't do: ${name}`, () => press({ needsWont: x.id }), { keywords: 'not mine who for' })
    }
    // The meetings you go to (Your day also lists the ones not answered and the maybes).
    for (const x of (d.day || []).filter((x) => x.response !== 'notResponded' && x.response !== 'tentativelyAccepted')) {
      const end = x.end ? new Date(x.end) : x.start ? new Date(x.start) : null
      if (!x.prep || x.prep.writing || (end && end < now)) continue
      const name = x.subject || x.title || 'meeting'
      if (x.prep.ready) add('Today', `Show prep: ${name}`, () => press({ evPrep: x.prep.id, evCall: x.call || '' }), { keywords: 'prep brief meeting' })
      else add('Today', `Prepare: ${name}`, () => press({ prepWrite: x.prep.id, prepOpen: '1' }), { keywords: 'prep brief meeting before' })
    }
    const w = d.work || {}
    for (const t of ['mine', 'owed', 'waiting'].flatMap((k) => (w[k] && w[k].items) || [])) {
      const attrs = { project: t.project, title: t.title, node: t.id }
      add('My work', `Close: ${t.title}`, () => press({ todayClose: t.id, ...attrs }), { keywords: 'task done finish complete' })
      add('My work', `Claude: ${t.title}`, () => press({ claudeTask: t.id, today: '1', ...attrs }), { keywords: 'ask send change move due later work on it skill follow-up mail session' })
    }
    // The plugins' items (`ctx.work.provide`): open each in the browser, as its title does.
    for (const r of (w.plugins || []).filter((r) => /^https:\/\//.test(r.url || ''))) {
      add('My work', `Open: ${r.title}`, () => press({ openUrl: r.url }), { keywords: `task ${r.kind || ''} ${r.where || ''} ${r.source_name || ''}`, hint: [r.source_name, linkHost(r.url)].filter(Boolean).join(' · '), icon: workRowIcon(r) })
    }
  }
  // Echo's own
  if (st.recording && st.recording_memo) add('Echo', 'Stop the memo', () => press({ cmd: 'stop' }), { keywords: 'end memo', hint: MAC ? '⇧⌘R' : 'Ctrl+Shift+R' })
  else if (st.recording) add('Echo', 'Stop recording…', () => press({ cmd: 'stop-ask' }), { keywords: 'end', hint: MAC ? '⇧⌘R' : 'Ctrl+Shift+R' })
  else if (st.armed) add('Echo', 'Cancel recording the next call', () => press({ cmd: 'stop' }), { keywords: 'disarm' })
  else add('Echo', st.in_call ? 'Record this call' : 'Record the next call', () => press({ cmd: 'record' }), { keywords: 'start arm' })
  add('Echo', 'Record a memo', () => press({ cmd: 'record-memo' }), { keywords: 'memo dictate voice note myself idea', disabled: st.recording ? 'not while a recording is on' : null })
  add('Echo', 'Claude', () => claudePanel('claude-folder'), { keywords: 'terminal session ask echo folder' })
  claudeSessionActions(add)
  add('Echo', 'Vocabulary', () => press({ panelShow: 'vocabulary' }), { keywords: 'terms names words dictionary transcription learned glossary' })
  add('Echo', 'Show or hide Inbox', () => press({ panelFlip: 'needs' }), { keywords: 'needs you notifications attention decide review', hint: NY_HINT })
  for (const i of nyOpen().slice(0, 8)) add('Inbox', i.title, nyShow, { keywords: i.why })
  add('Echo', 'Show or hide My notes', () => press({ panelFlip: 'notes' }), { disabled: S.view === 'calls' && S.selected ? null : 'open a call first' })
  add('Echo', 'Show or hide the bottom panel', () => { Layout.toggle('bottom'); layoutChanged() })
  add('Echo', 'Expand or fold the sidebar', flipBar, { keywords: 'activity bar labels names drawer', hint: BAR_KEY })
  add('Echo', 'Check for updates', () => press({ act: 'check_updates' }))
  add('Go to', 'Yolo runs', () => press({ yoloRuns: '' }), { keywords: 'claude on its own what did claude do history log runs' })
  add('Echo', "What's new", () => press({ wnOpen: '' }), { keywords: 'release notes changes update version news' })
  add('Echo', 'Ask for permissions', () => press({ act: 'ask_permissions' }), { keywords: 'microphone screen accessibility health' })
  // Go to
  for (const a of shownActivities()) add('Go to', a.label, () => goActivity(a.id))
  for (const p of (S.pj.data && S.pj.data.projects) || []) {
    const st = ProjectArt.stage(p.stage)
    add('Threads', p.name, () => go(`project/${p.id}/brief`), { keywords: `thread project ${p.client_name || ''} ${st.label} ${st.group}`, hint: st.label })
  }
  for (const x of Plugins.palette) add(x.group || 'Plugins', x.label, () => Promise.resolve().then(() => x.run(target)).catch((err) => { S.notice = `${x.label}: ${(err && err.message) || err}`; render() }), { keywords: x.keywords || '' })
  for (const [id, label] of SETTINGS_SECTIONS) add('Go to', `Settings: ${label}`, () => go(`settings/${id}`), { keywords: 'preferences' })
  for (const c of S.calls || []) {
    const d = c.started_at ? new Date(c.started_at) : null
    add('Calls', c.title, () => go(`call/${c.name}/${S.tab}`), { keywords: `${(c.people || []).join(' ')} ${d ? dayLabel(d) : ''}`, hint: d ? dayLabel(d) : '' })
  }
  return items
}
function paletteOpen() {
  PAL.open = true; PAL.query = ''; PAL.sel = 0; PAL.target = paletteTarget()
  let el = $('#palette')
  if (!el) { el = document.createElement('div'); el.id = 'palette'; document.body.appendChild(el) }
  el.innerHTML = `<div class="pal-back" data-pal-close></div><div class="pal" role="dialog" aria-label="Actions">
    <div class="pal-head">${ICON('<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>')}<input id="pal-q" autocomplete="off" spellcheck="false" placeholder="Type an action, a call, a person…" aria-label="Action"><span id="pal-target"></span></div>
    <div class="pal-list" id="pal-list" role="listbox"></div>
    <div class="pal-foot"><span><kbd>↵</kbd> run</span><span><kbd>↑</kbd><kbd>↓</kbd> choose</span><span><kbd>esc</kbd> close</span>${PAL.target ? '<span><kbd>⌫</kbd> on an empty box: drop the target</span>' : ''}</div></div>`
  paletteDraw()
  $('#pal-q').focus()
}
function paletteClose() {
  PAL.open = false; const el = $('#palette'); if (el) el.remove()
  // Opened over What's new: the keyboard goes back into that dialog.
  if (S.wn) whatsNewFocus()
}
function paletteDraw() {
  const t = PAL.target
  $('#pal-target').innerHTML = t ? `<button class="pal-chip" data-pal-untarget title="Act on Echo itself instead">On: ${esc(t.label.length > 32 ? t.label.slice(0, 31) + '…' : t.label)} ×</button>` : ''
  const all = paletteItems(t)
  // Calls show only when something is typed (the list would be every call).
  PAL.items = Palette.grouped(Palette.rank(PAL.query.trim() ? all : all.filter((x) => x.group !== 'Calls'), PAL.query))
  if (PAL.sel >= PAL.items.length) PAL.sel = Math.max(0, PAL.items.length - 1)
  let group = null
  // Each group's items in a group named by its head.
  $('#pal-list').innerHTML = PAL.items.map((it, i) => {
    const head = it.group !== group ? `${group === null ? '' : '</div>'}<div role="group" aria-label="${esc(it.group)}"><div class="pal-group">${esc(it.group)}</div>` : ''
    group = it.group
    return `${head}<button class="pal-item${i === PAL.sel ? ' on' : ''}${it.disabled ? ' off' : ''}" role="option" aria-selected="${i === PAL.sel}" aria-disabled="${!!it.disabled}" data-pal-i="${i}"><span class="pal-label">${it.icon || ''}${esc(it.label)}</span><span class="pal-hint">${esc(it.disabled ? it.disabled : it.hint || '')}</span></button>`
  }).join('') + (group === null ? '' : '</div>') || '<div class="pal-empty">Nothing matches.</div>'
  const on = $('#pal-list .pal-item.on'); if (on) on.scrollIntoView({ block: 'nearest' })
}
function paletteRun(i) {
  const it = PAL.items[i]
  if (!it || it.disabled) return
  paletteClose()
  it.run()
}
document.addEventListener('click', (e) => { if (e.target.closest && e.target.closest('[data-pal-open]')) paletteOpen() })
document.addEventListener('input', (e) => { if (e.target.id === 'pal-q') { PAL.query = e.target.value; PAL.sel = 0; paletteDraw() } })
document.addEventListener('click', (e) => {
  if (!PAL.open) return
  const b = e.target.closest && e.target.closest('[data-pal-i],[data-pal-close],[data-pal-untarget]')
  if (!b) return
  e.stopPropagation()
  if (b.dataset.palClose !== undefined) return paletteClose()
  if (b.dataset.palUntarget !== undefined) { PAL.target = null; PAL.sel = 0; paletteDraw(); return $('#pal-q').focus() }
  paletteRun(Number(b.dataset.palI))
}, true)
document.addEventListener('keydown', (e) => {
  const mod = MAC ? e.metaKey : e.ctrlKey
  // ⌘K (Ctrl+K, Ctrl+Shift+P on Windows) — not while a terminal has the keys (Claude Code's own Ctrl+K).
  if (((mod && !e.shiftKey && e.key.toLowerCase() === 'k') || (!MAC && e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'p')) && !(e.target.closest && e.target.closest('.xterm'))) {
    e.preventDefault(); e.stopPropagation()
    return PAL.open ? paletteClose() : paletteOpen()
  }
  // ⇧⌘R / Ctrl+Shift+R: stop the recording (it asks first).
  if (mod && e.shiftKey && e.key.toLowerCase() === 'r' && S.state && S.state.recording && !(e.target.closest && e.target.closest('.xterm'))) {
    e.preventDefault(); e.stopPropagation(); return press({ cmd: 'stop-ask' })
  }
  // ⇧⌘I / Ctrl+Shift+U: show or hide Inbox (Ctrl+Shift+I is WebView2's DevTools on Windows).
  if (mod && e.shiftKey && e.key.toLowerCase() === NY_KEY.toLowerCase() && !(e.target.closest && e.target.closest('.xterm'))) {
    e.preventDefault(); e.stopPropagation(); return press({ panelFlip: 'needs' })
  }
  if (!PAL.open || e.target.id !== 'pal-q') return
  if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); return paletteClose() }
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); const n = PAL.items.length; if (n) PAL.sel = (PAL.sel + (e.key === 'ArrowDown' ? 1 : n - 1)) % n; return paletteDraw() }
  if (e.key === 'Enter') { e.preventDefault(); e.stopPropagation(); return paletteRun(PAL.sel) }
  if (e.key === 'Backspace' && !PAL.query && PAL.target) { PAL.target = null; PAL.sel = 0; return paletteDraw() }
}, true)

