function eventTypeChip(e) {
  const t = e.meeting_type
  if (!t || !t.key) return ''
  const types = ((TPL.data && TPL.data.meeting_types) || []).map((x) => [x.id, x.name])
  if (!types.length) { if (!TPL.data) loadTemplates().then(render); return ` <span class="mt-chip${t.by === 'guess' ? ' guess' : ''}">${esc(t.label)}</span>` }
  const hint = t.by === 'you' ? 'You set this — for every meeting of the series' : t.by === 'invite' ? 'From its invite — change it for every meeting of the series' : 'Guessed from who is invited'
  return ` <label class="mt-chip${t.by === 'guess' ? ' guess' : ''}" title="${esc(hint)}">${esc(t.label)} ▾<select data-event-type="${esc(t.key)}" aria-label="Kind of meeting">${types.map(([k, l]) => `<option value="${esc(k)}"${t.type === k ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
}
document.addEventListener('change', (e) => {
  if (!e.target.matches || !e.target.matches('[data-event-type]')) return
  const key = e.target.dataset.eventType, kind = e.target.value
  send({ cmd: 'set_event_type', key, kind })
  const name = (((TPL.data && TPL.data.meeting_types) || []).find((x) => x.id === kind) || {}).name || kind
  for (const ev of (S.cal.data && S.cal.data.events) || []) if (ev.meeting_type && ev.meeting_type.key === key) ev.meeting_type = { ...ev.meeting_type, type: kind, label: name, by: 'you' }
  render()
})
/** What kind of meeting it was — Client, Partner, 1:1, Internal, Industry & Analyst, Other — guessed from who was
 *  invited, settled by the minutes, changed here by the user (kept from then on). Not on a memo. */
function meetingTypeChip(c) {
  const t = c.meeting_type
  if (!t || c.memo) return ''
  const hint = t.by === 'you' ? 'You set this' : t.by === 'echo' ? 'Settled by the minutes — change it if it is wrong' : t.by === 'invite' ? 'From the meeting\'s invite — the minutes may settle it' : 'Guessed from who was invited — the minutes settle it'
  return `<label class="mt-chip${t.by === 'guess' ? ' guess' : ''}" title="${esc(hint)}">${esc(t.label)} ▾<select data-meeting-type aria-label="Kind of meeting">${(c.meeting_types || []).map(([k, l]) => `<option value="${esc(k)}"${t.type === k ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
}
document.addEventListener('change', (e) => {
  if (!e.target.matches || !e.target.matches('[data-meeting-type]') || !S.call) return
  const kind = e.target.value
  send({ cmd: 'set_meeting_type', call: S.selected, kind })
  S.call.meeting_type = { type: kind, label: ((S.call.meeting_types || []).find(([k]) => k === kind) || [, kind])[1], by: 'you' }
  render()
})

/** The call's one set of actions: a ⋯ menu (the folder, the minutes page, delete). */
function callMenu(c) {
  const item = (cmd, label, cls = '') => `<button class="menu-item${cls}" role="menuitem" data-cmd="${cmd}">${esc(label)}</button>`
  const items = [
    item('reveal', 'Show folder'),
    c.notes ? item('open-minutes', 'Open minutes in browser') : '',
    c.notes ? item('reveal-minutes', `${SHOW_FILE.replace('Show in', 'Show minutes in')}`) : '',
    c.notes ? item('write-notes', 'Write minutes again') : '',
    c.notes && c.template && c.template.why !== 'memo' ? writeAgainWith(c) : '',
    item('ask-claude', 'Ask Claude about this call'),
    slotItems('call.menu', { kind: 'call', call: c }, 'menu'),
    '<div class="menu-sep" role="separator"></div>',
    item('ask-delete', 'Delete…', ' danger'),
  ].join('')
  const eff = Layout.effective(window.innerWidth, window.innerHeight, panelViews(), S.overlay)
  const a = Layout.area('notes', panelViews())
  const on = !!(a && eff[a].shown && eff[a].active === 'notes')
  const notes = `<button class="icon-btn${on ? ' on' : ''}" data-panel-flip="notes" aria-pressed="${on}" title="My notes" aria-label="My notes">${ICON('<path d="M5 4h10l4 4v12H5z"/><path d="M15 4v4h4M8 12h8M8 16h6"/>')}</button>`
  // What moved, beside it: only for a call that is in a thread; its count once a thread has read it.
  const ma = (c.changes || []).length && Layout.area('moved', panelViews())
  const mOn = !!(ma && eff[ma].shown && eff[ma].active === 'moved'), mn = movedCount(c)
  const moved = ma ? `<button class="icon-btn${mOn ? ' on' : ''}${mn ? ' has-n' : ''}" data-panel-flip="moved" aria-pressed="${mOn}" title="What moved" aria-label="What moved${mn ? ` · ${mn} change${mn === 1 ? '' : 's'}` : ''}">${MOVED_ICON}${mn ? `<span class="mv-n">${mn}</span>` : ''}</button>` : ''
  return notes + moved + `<button class="icon-btn" data-cmd="call-menu" aria-haspopup="menu" aria-expanded="${S.menuOpen}" aria-label="Call actions" title="Call actions">${ICON('<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>')}</button>${S.menuOpen ? `<div class="menu" role="menu">${items}</div>` : ''}`
}

/** Messages for a moment float over the window (`toasts`, drawn once in #r-over): nothing in the
 *  page moves when one comes or goes. */
function toasts() {
  const n = S.notice ? `<div class="toast"><span class="grow">${esc(S.notice).replace(/`([^`]+)`/g, '<code>$1</code>')}</span><button class="btn ghost" data-cmd="notice-ok">OK</button></div>` : ''
  const c = S.closeError ? `<div class="toast error"><span class="grow">Your notes couldn't be saved: ${esc(S.closeError)}</span><button class="btn" data-cmd="close-retry">Try again</button><button class="btn ghost danger" data-cmd="close-now">Close without saving</button></div>` : ''
  return n + c
}

/** A release's one-time announcement, until answered (`announce.rs`): a strip across the window under the top bar, its
 *  buttons the first the main one. */
function announcement() {
  const a = S.state && S.state.announcement
  if (!a) return ''
  return `<div class="announce" role="status"><span class="grow"><b>${esc(a.title)}</b> ${esc(a.text)}</span>${a.buttons.map((b, i) => `<button class="btn${i ? ' ghost' : ' primary'}" data-announce="${esc(b.action)}" data-arg="${esc(b.arg)}">${esc(b.label)}</button>`).join('')}</div>`
}

function confirmBox(c) {
  return `<div class="confirm"><span>Delete "${esc(c.title)}"? It goes to the trash in the data folder for 30 days.</span>
    <button class="btn" data-cmd="cancel-delete">Keep</button><button class="btn primary" style="background:var(--danger);border-color:var(--danger)" data-cmd="delete">Delete</button></div>`
}

/** Transcript turns: consecutive lines of one speaker joined; `q` highlights, `jump` (seconds)
 *  marks the turn to scroll to. */
/** A turn's longest stretch, in seconds, before the same speaker's next line starts a new one. */
const TURN_MAX = 60
// ── transcript jump
/** The turn a moment of the call opens on. Moments are whole seconds (minutes chips, notes, links):
 *  the first turn starting in that second, else the turn under way then, or the next one when the
 *  moment falls in the silence after it. */
function jumpTurn(turns, t) {
  const at = turns.findIndex((x) => x.start >= t && x.start < t + 1)
  if (at >= 0) return at
  let i = -1
  for (let k = 0; k < turns.length; k++) if (turns[k].start < t) i = k
  if (i < 0) return 0
  return turns[i].end < t && i + 1 < turns.length ? i + 1 : i
}
// end of the transcript jump block
/** Each transcript's turns, by its place (`live`, `tab`): the page holds an empty `.transcript` there, and
 *  `fillTranscripts` puts in only the turns that are new or changed, so a growing transcript isn't drawn again whole
 *  every few seconds while a call records. */
const TRANSCRIPTS = {}

function turnsHtml(segs, q, jump, marks = [], key = 'tab') {
  // One speaker's lines join a turn while they follow each other (< 8 s apart), each line its own
  // paragraph; a turn runs at most TURN_MAX seconds, then the next starts with its own time — during a
  // call every other voice is one "Remote" track, which would otherwise be one block of the whole call.
  const turns = []
  for (const s of segs) {
    const last = turns[turns.length - 1]
    if (last && last.who === speaker(s) && s.start - last.end < 8 && s.start - last.start < TURN_MAX) { last.text += ' ' + s.text; last.parts.push(s.text); last.end = s.end }
    else turns.push({ who: speaker(s), me: speaker(s) === 'Me', spk: s.speaker_id, start: s.start, end: s.end, lang: s.lang, text: s.text, parts: [s.text] })
  }
  const jumpIdx = jump == null ? -1 : jumpTurn(turns, jump)
  TRANSCRIPTS[key] = turns.map((t, i) => {
    const hit = (q && t.text.toLowerCase().includes(q)) || i === jumpIdx
    // Detected remote speakers each get a color of their own (speakers.rs numbers them).
    const spk = !t.me && t.spk ? ` spk${((t.spk - 1) % 6) + 1}` : ''
    // My notes at this turn's moment: a dot each in the margin (the note's text on hover).
    // On the turn its time opens, as a click on it would.
    const mine = marks.filter((m) => jumpTurn(turns, m.t) === i)
    const dots = mine.length ? `<span class="nt-marks">${mine.map((m) => `<span class="nt-dot k-${m.kind}" title="${esc((NOTE_LOOK[m.kind] || NOTE_LOOK.note)[1] + (m.text ? ': ' + m.text : ''))}"></span>`).join('')}</span>` : ''
    return `<div class="turn ${t.me ? 'me' : 'remote'}${spk}${hit ? ' hit' : ''}" data-t="${t.start}" role="group" aria-label="${clock(Math.floor(t.start))} ${esc(t.who)}" ${i === jumpIdx ? 'id="jump"' : ''}>
      <time>${dots}${clock(Math.floor(t.start))}</time>
      <div><div class="who">${esc(t.who)}${t.lang ? `<span class="lang">${esc(t.lang)}</span>` : ''}</div>${t.parts.map((x) => `<p>${esc(x)}</p>`).join('')}</div></div>`
  })
  return `<div class="transcript" data-turns="${key}"></div>`
}

/** Bring every `.transcript[data-turns]` on the page up to its turns: a changed turn is replaced, new ones added at
 *  the end, gone ones removed; turns that didn't change are left as they are. */
function fillTranscripts() {
  for (const box of document.querySelectorAll('.transcript[data-turns]')) {
    const turns = TRANSCRIPTS[box.dataset.turns] || []
    turns.forEach((html, i) => {
      const old = box.children[i]
      if (old && old._html === html) return
      const tmp = document.createElement('div')
      tmp.innerHTML = html
      const el = tmp.firstElementChild
      el._html = html
      if (old) old.replaceWith(el)
      else box.appendChild(el)
    })
    while (box.children.length > turns.length) box.lastElementChild.remove()
  }
}

function tabBody(c, segs, chat, frames) {
  if (S.tab === 'prep') return prepView(c.prep, false)
  if (S.tab === 'transcript') {
    // A recording with live notes: the live transcript at full width, kept at its end (the notes are in My notes).
    if (hasLive(c) && c.status === 'recording') return liveTranscript(segs)
    if (!segs.length) return c.status === 'recording'
      ? `<p style="color:var(--text-3)">Listening. The live transcript appears here as people speak.</p>`
      : `<p style="color:var(--text-3)">${c.status === 'transcribing' ? 'Transcribing on this computer. This takes about a quarter of the call length.' : 'No transcript for this call.'}</p>`
    return `<div class="from-bar"><input class="search" id="find" placeholder="Find in transcript" value="${esc(S.find)}" style="max-width:280px" aria-label="Find in transcript"></div>
      ${turnsHtml(segs, S.find.trim().toLowerCase(), S.jump != null ? S.jump : S.jumpShown && S.jumpShown.call === c.name && !S.find.trim() ? S.jumpShown.t : null, noteMarks(c))}`
  }
  if (S.tab === 'chat') {
    return `<div class="chat">${chat.map((m) => `<article class="msg"><header><b>${esc(m.from)}</b><time>${m.t == null ? '' : m.t < 0 ? 'before the call' : clock(m.t)}</time></header>
      <p>${esc(m.text)}</p>${m.files && m.files.length ? `<div class="files">${esc(m.files.join(', '))}</div>` : ''}</article>`).join('')}</div>`
  }
  if (S.tab === 'screens') {
    return `<div class="frames">${frames.map((f) => `<button class="frame" data-frame="${esc(f.url)}"><img src="${esc(f.url)}" alt="Shared screen at ${clock(f.t || 0)}" loading="lazy"><span>${clock(f.t || 0)}</span></button>`).join('')}</div>`
  }
  return minutesTab(c)
}

/** What this call changed in its threads' work (`ledger::call_changes`): one line per change, in
 *  the order said — what happened, to which item, the words, the verifier's mark; changes the verifier called
 *  wrong, none. */
const CC_KIND = { story: 'New story', task: 'New task', decision: 'Decision', risk: 'Risk', question: 'Question' }
function ccWhat(x) {
  const n = x.node
  if (x.op === 'create') return [n.kind === 'task' && n.type === 'session' ? 'New session' : n.kind === 'task' && n.type === 'dependency' ? 'New dependency' : CC_KIND[n.kind] || 'New', n.kind]
  if (x.effect) {
    const e = x.effect
    return [e === 'due' ? `Due ${x.due ? fmtDay(x.due) : x.due_text || ''}` : e === 'held' ? 'Held' : e === 'done' ? 'Done' : e === 'progress' ? 'Progress' : e === 'blocked' ? 'Blocked'
      : e === 'cancelled' ? 'Cancelled' : e === 'reopen' ? 'Reopened' : e === 'delayed' ? 'Slipped' : e === 'reassign' ? 'New owner' : e, e === 'done' || e === 'held' ? 'done' : e === 'blocked' || e === 'delayed' ? 'warn' : 'move']
  }
  if (x.op === 'link') return ['Now waits', 'link']
  if (x.op === 'waiting') return [x.what ? 'Waiting' : 'Not waiting', 'warn']
  if (x.op === 'ac_met') return ['Criterion met', 'done']
  if (x.op === 'revise') return ['Revised', 'move']
  if (x.op === 'answer') return [n.kind === 'risk' ? 'Response' : 'Answered', 'done']
  return [x.op, 'move']
}
const TASK_DONE = new Set(['done', 'cancelled', 'merged', 'deleted'])
// Claude Code's own mark (thesvg.org/icon/claude-code, mono), filled — the other icons are strokes.
const CLAUDE_MARK = '<path fill="currentColor" stroke="none" fill-rule="evenodd" d="M20.998 10.949H24v3.102h-3v3.028h-1.487V20H18v-2.921h-1.487V20H15v-2.921H9V20H7.488v-2.921H6V20H4.487v-2.921H3V14.05H0V10.95h3V5h17.998v5.949zM6 10.949h1.488V8.102H6v2.847zm10.51 0H18V8.102h-1.49v2.847z"/>'
const HAND_MARK = '<path d="M8 13V6.5a1.5 1.5 0 0 1 3 0V12M11 11V5a1.5 1.5 0 0 1 3 0v6M14 11V6.5a1.5 1.5 0 0 1 3 0V14c0 3.5-2.5 6-6 6h-.5a6 6 0 0 1-4.6-2.2L4.5 15.6a1.5 1.5 0 0 1 2.3-1.9L8 15"/>'
/** Who does one of the user's tasks (`taskmodes`), as an icon: Claude's mark or a hand — Echo's guess faded. Its
 *  how, and the switch, are in the task's details (`modeRows`). */
function modeIcon(m) {
  if (!m || !m.mode) return ''
  const claude = m.mode === 'claude', guess = m.by === 'guess'
  const tip = `${claude ? 'Claude can do it' : 'You do it by hand'}${guess ? " — Echo's guess" : ''}${!claude && m.how ? `: ${m.how}` : ''}`
  return `<span class="md-ico${claude ? ' claude' : ''}${guess ? ' guess' : ''}" title="${esc(tip)}" aria-label="${esc(tip)}">${ICON(claude ? CLAUDE_MARK : HAND_MARK)}</span>`
}
/** A Claude one's ✦ carries its skill into the Claude dialog, and its prompt: the user's own as their note; Echo's
 *  guess — model output from others' words — only as a hint beside it, never typed as theirs (Claude reads it with
 *  the task, as data). */
const modeData = (n) => {
  const m = n.mode
  if (!m || m.mode !== 'claude') return ''
  const p = m.prompt || ''
  return ` data-skill="${esc(m.skill || '')}"${m.prompt_by === 'you' ? ` data-prompt="${esc(p)}"` : ` data-hint="${esc(p)}"`}`
}
/** The task's details: who does it (a switch of two icons, named in their label and tooltip; a click keeps the user's choice on the task), and its how — or what
 *  Claude hands back — written by Echo's guess or by hand (`edit`: the details' own field editor). */
function modeRows(project, n, val) {
  const m = n.who_does || {}, guess = m.by === 'guess'
  const seg = (k, mark, label) => `<button class="md-seg${m.mode === k ? ' on' : ''}${m.mode === k && guess ? ' guess' : ''}" data-mode-set="${k}" data-project="${esc(project)}" data-node="${esc(n.id)}" aria-pressed="${m.mode === k}" aria-label="${label}" title="${label}">${ICON(mark)}</button>`
  const rows = [['Who does it', `<span class="md-segs" role="group" aria-label="Who does it">${seg('claude', CLAUDE_MARK, 'Claude can')}${seg('manual', HAND_MARK, 'By hand')}</span>${guess ? ' <span class="set-sub">Echo\'s guess</span>' : ''}`]]
  if (m.mode === 'manual') rows.push(['How', val('how', m.how, 'not said', `<input class="wp-in" data-ledger-set="how" value="${esc(m.how || '')}" maxlength="200" aria-label="How" autofocus>`)])
  if (m.mode === 'claude') rows.push(['Claude hands back', val('prompt', m.prompt, 'not said', `<textarea class="wp-in" data-ledger-set="prompt" rows="3" maxlength="600" aria-label="What Claude hands back" autofocus>${esc(m.prompt || '')}</textarea>`)])
  return rows
}
/** What the call changed in its threads (the What moved panel view): each thread's changes, the user's own open
 *  tasks first with who does each (`modeIcon`; a click opens the task's details). */
const movedRows = (g) => g.changes.filter((x) => x.verdict !== 'wrong')
/** How many changes the call made in its threads that have read it (the What moved tab's count). */
const movedCount = (c) => ((c && c.changes) || []).reduce((n, g) => n + (g.read ? movedRows(g).length : 0), 0)
function callChanges(c) {
  const blocks = (c.changes || []).map((g) => {
    const p = projById(g.project) || { id: g.project, name: g.project }
    const rows = movedRows(g)
      .sort((a, b) => (b.node.mine && !TASK_DONE.has(b.node.state) ? 1 : 0) - (a.node.mine && !TASK_DONE.has(a.node.state) ? 1 : 0))
    const head = `<div class="cc-th" role="group" aria-label="${esc(p.name)}">${projIcon(p, 14)}<b>${esc(p.name)}</b>`
    if (!g.read) {
      // Not read yet: being read now, due at a time (15 min after the call, then the next look), or
      // nothing to read (no transcript).
      const at = g.reads_at && new Date(g.reads_at)
      const why = g.syncing ? 'is reading this call now…'
        : g.queued ? 'hasn\'t read this call yet'
        : at ? (at > new Date() ? `reads this call after ${esc(hm(at))} — 15 minutes after it ended` : 'reads this call within a few minutes')
        : 'has nothing to read in this call (no transcript)'
      const canSync = !g.syncing && (g.queued || at)
      return `${head}<span class="set-sub">${why}</span><span class="grow"></span>${canSync ? `<button class="link-btn" data-cc-sync="${esc(g.project)}">Sync now</button>` : ''}</div>`
    }
    if (!rows.length) return `${head}<span class="set-sub">nothing changed</span></div>`
    return `${head}<span class="set-sub">${rows.length} change${rows.length === 1 ? '' : 's'}</span></div><div class="cc-list">${rows.map((x) => {
      const [what, cls] = ccWhat(x)
      const mark = x.applied === false && x.verdict === 'supports' ? '<span class="wk-v weak" title="You set this yourself; the call\'s words are kept in its history">kept yours</span>'
        : x.verdict === 'weak' ? '<span class="wk-v weak">weak · to review</span>'
        : x.op !== 'create' && x.verdict == null && x.applied === false ? '<span class="wk-v">to check</span>' : ''
      const ts = x.ts ? hms(x.ts) : null
      return `<div class="cc-row" role="group" aria-label="${esc(`${what} ${x.node.id} ${x.node.title}`)}"><span class="cc-what ${esc(cls)}">${esc(what)}</span>
        <div class="cc-main"><button class="cc-node" data-tk="open" data-project="${esc(g.project)}" data-node="${esc(x.node.id)}"><span class="wk-id">${esc(x.node.id)}</span>${x.node.mine && !TASK_DONE.has(x.node.state) ? ` ${modeIcon(x.node.mode)}` : ''} ${esc(x.node.title)}</button>
        ${x.op === 'link' ? `<div class="cc-dep">for <button class="cc-node inline" data-tk="open" data-project="${esc(g.project)}" data-node="${esc(x.depends_on)}"><span class="wk-id">${esc(x.depends_on)}</span> ${esc(x.depends_on_title || '')}</button></div>`
          : x.op === 'waiting' && x.what ? `<div class="cc-dep">on ${esc(x.who || 'someone')} — ${esc(x.what)}</div>` : ''}
        ${x.quote ? `<div class="cc-q">“${esc(x.quote)}”${x.speaker ? ` <span>— ${esc(x.speaker)}</span>` : ''}</div>` : ''}
</div>
        ${mark}${ts != null ? `<button class="btn ghost" data-at="${ts}">${clock(ts)}</button>` : '<span></span>'}</div>`
    }).join('')}</div>`
  })
  return blocks.length ? `<section class="cc">${blocks.join('')}</section>` : ''
}
const hms = (s) => String(s).split(':').reduce((a, x) => a * 60 + Number(x), 0)

