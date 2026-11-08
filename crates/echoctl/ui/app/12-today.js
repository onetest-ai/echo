function todayView() {
  const d = S.today
  if (!d) return `<div class="skeleton" style="padding:22px">${'<i></i>'.repeat(8)}</div>`
  const now = new Date()
  const date = now.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })
  const meetings = (d.day || []).filter((x) => x.kind === 'meeting').length
  const mine = (d.work && d.work.mine) || { items: [], total: 0 }
  // How many of yours are late, beside the meetings and tasks (from the Meeting Notebook plugin's Today).
  const late = (mine.items || []).filter((t) => t.late).length
  // Hello by the time of day, by the user's first name (About me).
  const hr = now.getHours(), first = (d.me || '').trim().split(/\s+/)[0]
  const hello = `${hr < 5 ? 'Good night' : hr < 12 ? 'Good morning' : hr < 18 ? 'Good afternoon' : 'Good evening'}${first ? `, ${first}` : ''}`
  const head = `<header class="td-head"><h1>${esc(hello)}</h1><span class="set-sub">${esc(date)}${meetings ? ` · ${meetings} meeting${meetings === 1 ? '' : 's'}` : ''}${mine.total ? ` · ${mine.total} task${mine.total === 1 ? '' : 's'} on you` : ''}${late ? ` · <span class="warn">${late} late</span>` : ''}</span></header>`
  const cards = (d.cards || []).map((c) => `<section class="td-card"><div class="td-card-h"><b>${esc(c.title)}</b><span class="set-sub">${esc([c.about, c.at ? hm(new Date(c.at)) : ''].filter(Boolean).join(' · '))}</span><button class="btn ghost" data-today-seen="${esc(c.key)}">Got it</button></div><div class="td-md">${c.html}</div>${c.notice ? `<p class="set-sub">${esc(c.notice)}</p>` : ''}</section>`).join('')
  // Your day: a line a meeting or call, the line "now" between past and to come.
  let nowDrawn = false
  const rows = (d.day || []).map((x) => {
    const st = x.start ? new Date(x.start) : null, en = x.end ? new Date(x.end) : null
    const past = en ? en < now : st ? st < now : false
    const line = !nowDrawn && st && st > now ? (nowDrawn = true, `<div class="td-now"><span>${esc(hm(now))}</span></div>`) : ''
    const chips = (x.threads || []).map((p) => `<span class="pj-chip"><button class="link-btn" data-pj="goto" data-id="${esc(p.id)}">${projIcon(p, 14)}${esc(p.name)}</button></span>`).join('')
    const acts = []
    // Its minutes (from the Meeting Notebook plugin's Today): writing, coming at a time, written, or failed — Try again.
    const mn = x.minutes || {}
    // New minutes (not opened yet) stand out, with what they hold — what to read after a busy day.
    const ct = mn.counts || {}
    const held = [ct.decisions ? `${ct.decisions} decision${ct.decisions === 1 ? '' : 's'}` : '', ct.actions ? `${ct.actions} action${ct.actions === 1 ? '' : 's'}` : '', ct.questions ? `${ct.questions} question${ct.questions === 1 ? '' : 's'}` : ''].filter(Boolean).join(' · ')
    if (x.call && mn.state === 'ready') acts.push(`<button class="btn${mn.unread ? ' primary' : ' ghost'}" data-call="${esc(x.call)}" data-call-tab="minutes">${mn.unread ? 'New minutes' : 'Minutes'}</button>${held ? `<span class="set-sub">${esc(held)}</span>` : ''}`)
    else if (x.call) acts.push(`<button class="btn ghost" data-call="${esc(x.call)}">Open the call</button>`)
    if (mn.state === 'writing') acts.push('<span class="set-sub">Writing the minutes…</span>')
    else if (mn.state === 'transcribing') acts.push('<span class="set-sub">Transcribing…</span>')
    else if (mn.state === 'at' && mn.at) acts.push(`<span class="set-sub">Minutes at ${esc(hm(new Date(mn.at)))}</span>`)
    else if (mn.state === 'failed') acts.push(`<span class="warn" title="${esc(mn.why || '')}">Minutes failed</span><button class="btn" data-today-retry="${esc(x.call)}">Try again</button>`)
    if (x.prep && x.prep.ready) acts.push(`<button class="btn${past ? ' ghost' : ' primary'}" data-ev-prep="${esc(x.prep.id)}" data-ev-call="${esc(x.call || '')}">Show prep</button>`)
    else if (x.prep && x.prep.writing) acts.push('<span class="set-sub">Writing the prep…</span>')
    else if (x.prep && !past) acts.push(`<button class="btn" data-prep-write="${esc(x.prep.id)}" data-prep-open="1">Prepare</button>`)
    // Over and not recorded: its Teams transcript, as the calendar offers it.
    if (x.importable && x.transcript !== false) {
      if (x.import && x.import.busy) acts.push('<span class="set-sub">Importing the Teams transcript…</span>')
      else acts.push(`<button class="btn" data-today-import="${esc(x.event)}">Import Teams transcript</button>${x.import && x.import.error ? `<span class="warn">${esc(x.import.error)}</span>` : x.transcript === true ? '' : '<span class="set-sub">if Teams transcribed it</span>'}`)
    }
    if (x.kind === 'meeting' && !past && !x.call) acts.push(x.scheduled ? `<button class="btn ghost" data-unschedule="${esc(x.event)}">Don't record</button>` : `<button class="btn ghost" data-schedule="${esc(x.event)}">Record when it starts</button>`)
    return `${line}<div class="td-row${past ? ' past' : ''}"><span class="td-t mono">${st ? esc(hm(st)) : ''}</span><div class="td-b"><div class="td-s"><b>${esc(x.subject || 'A call')}</b>${chips}</div><div class="td-a">${acts.join('')}</div></div></div>`
  }).join('') + (!nowDrawn && (d.day || []).length ? `<div class="td-now"><span>${esc(hm(now))}</span></div>` : '')
  const day = (d.day || []).length ? `<section class="td-sec"><h2>Your day</h2><div class="td-rows">${rows}</div></section>` : ''
  // My work: a card a task due this week or late — what you owe, what is owed to you, what you wait for.
  const all = (S.pj.data && S.pj.data.projects) || []
  const today0 = new Date(); today0.setHours(0, 0, 0, 0)
  // The date as an icon (words don't fit a narrow card): late, today, later this week; the date in its hint.
  const CAL = '<rect x="4" y="5.5" width="16" height="14" rx="2.5"/><path d="M4 10h16M9 3.5v4M15 3.5v4"/>'
  const dueChip = (t) => {
    const d = t.due ? new Date(t.due + 'T12:00') : null
    const when = d ? d.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'short' }) : ''
    if (t.late) return `<span class="td-due late" title="Late${when ? ` — was due ${esc(when)}` : ''}" aria-label="Late">${ICON('<path d="M12 4l9 16H3z"/><path d="M12 10v4M12 17h.01"/>')}</span>`
    if (!d) return ''
    const today = d.toDateString() === today0.toDateString()
    return `<span class="td-due${today ? ' today' : ''}" title="Due ${today ? 'today' : esc(when)}" aria-label="Due ${today ? 'today' : esc(when)}">${ICON(today ? CAL + '<circle cx="12" cy="14.5" r="1.6" fill="currentColor"/>' : CAL)}</span>`
  }
  const card = (t, kind, who) => {
    const pj = all.find((x) => x.id === t.project)
    const where = [kind, who, pj ? pj.name : ''].filter(Boolean).map(esc).join(' · ')
    // Close… asks first in a dialog (`todayCloseDialog`); the title opens the task.
    const attrs = `data-project="${esc(t.project)}" data-title="${esc(t.title)}" data-node="${esc(t.id)}"`
    const close = `<button class="icon-btn td-ico" data-today-close="${esc(t.id)}" ${attrs} title="Close this task…" aria-label="Close “${esc(t.title)}”…">${ICON('<circle cx="12" cy="12" r="8.5"/><path d="M8.5 12.2l2.4 2.4 4.6-4.9"/>')}</button>`
    // Claude: one dialog — work on it in the panel, or Yolo (Claude does it on its own), with a skill.
    const work = `<button class="icon-btn td-ico" data-claude-task="${esc(t.id)}" data-today="1"${modeData(t)} ${attrs} title="Claude — work on this task, or Yolo" aria-label="Claude: “${esc(t.title)}”…">${ICON('<path d="M12 3.5l1.9 5 5 1.9-5 1.9-1.9 5-1.9-5-5-1.9 5-1.9z"/><path d="M18.5 15.5l.8 2 2 .8-2 .8-.8 2-.8-2-2-.8 2-.8z"/>')}</button>`
    // The date's icon before the title's words.
    // The card opens the task's details in the right panel, Today staying put (its icons do their own thing).
    return `<div class="td-task${S.pj.inline === 'today' && S.pj.node === t.id && S.pj.sel === t.project ? ' on' : ''}" data-tk="open" data-project="${esc(t.project)}" data-node="${esc(t.id)}" role="button" tabindex="0" title="Open its details"><span class="td-task-t">${dueChip(t)}<span class="td-task-title">${esc(t.title)}</span></span><span class="td-task-m">${pj ? projIcon(pj, 12) : ''}${where}<span class="td-icos">${kind === 'I owe' ? modeIcon(t.mode) : ''}${work}${close}</span></span></div>`
  }
  const w = d.work || {}
  // One list by date: late first (the oldest first), then by due date; whose it is stays on each card.
  const tasks = [...((w.mine && w.mine.items) || []).map((t) => [t, 'I owe', '']),
    ...((w.owed && w.owed.items) || []).map((t) => [t, 'owed to me', t.owner]),
    ...((w.waiting && w.waiting.items) || []).map((t) => [t, 'waiting for', t.waiting_for && t.waiting_for.who])]
  tasks.sort(([a], [b]) => (b.late ? 1 : 0) - (a.late ? 1 : 0) || String(a.due || '9999').localeCompare(String(b.due || '9999')))
  const cards2 = tasks.map(([t, kind, who]) => card(t, kind, who))
  const work = cards2.length || S.todayAskId ? `<section class="td-sec"><h2>My work · this week <button class="link-btn" data-go="tasks">Tasks</button></h2>${todayAskBar()}<div class="td-tasks">${cards2.join('')}</div></section>` : ''
  const empty = !cards && !day && !work ? '<div class="empty"><div><h3>A quiet day</h3><p>No meetings today and nothing on you in Tasks.</p></div></div>' : ''
  return `<div class="td">${head}${needsView(d.needs)}${cards}<div class="td-grid">${day}${work}</div>${empty}</div>${todayCloseDialog()}${needsDialog()}`
}

/** Today → Needs you now: tagged in the last day, or burning where you watch (`needs.rs`) — hot first. Reply in Teams
 *  (the app) or Read and reply (a mail, here); I'll do it (a task, in a thread) or Won't do. Empty: not drawn. */
function needsView(n) {
  const items = (n && n.items) || []
  if (!items.length) return ''
  const ago = (t) => { const m = Math.round((Date.now() - new Date(t)) / 60000); return m < 60 ? `${Math.max(m, 1)} min` : `${Math.round(m / 60)} h` }
  const rows = items.map((x) => {
    const reply = x.source === 'mail' ? `<button class="btn" data-needs-read="${esc(x.id)}">Read and reply</button>`
      : x.app_url || x.chat ? `<button class="btn" data-needs-open="${esc(x.app_url || '')}" data-chat="${esc(x.chat || '')}">Reply in Teams</button>` : ''
    return `<div class="nd-row"><span class="nd-dot ${x.kind === 'hot' ? 'hot' : ''}" aria-label="${x.kind === 'hot' ? 'Burning' : 'For you'}"></span>
      <div class="nd-b"><div><b>${esc(x.where || '')}</b> <span class="set-sub">— ${esc(x.line || '')}</span></div><div class="mono set-sub">${esc(x.from || '')} · ${esc(ago(x.at))} ago${x.quote ? ` · “${esc(x.quote)}”` : ''}</div></div>
      <div class="nd-a">${reply}<button class="btn primary" data-needs-todo="${esc(x.id)}">I'll do it</button><button class="btn ghost" data-needs-wont="${esc(x.id)}">Won't do</button></div></div>`
  }).join('')
  return `<section class="td-card nd" aria-labelledby="nd-h"><div class="td-card-h"><b id="nd-h">Needs you now</b><span class="set-sub">the last 24 h · where you watch · goes away when you answer</span></div>${rows}</section>`
}
/** The dialogs of Needs you now: I'll do it (a task's title and its thread), Read and reply (the mail, your answer). */
function needsDialog() {
  const c = S.needsDlg
  if (!c) return ''
  const item = (((S.today && S.today.needs) || {}).items || []).find((x) => x.id === c.id) || {}
  if (c.kind === 'todo') {
    const threads = ((S.pj.data && S.pj.data.projects) || []).filter((p) => p.group !== 'closed')
    // Its thread when Echo found one (its chat, its sender among the thread's people); else the user picks.
    const pick = `<select id="nd-thread" class="set-in">${item.thread ? '' : '<option value="" selected disabled>Choose a thread…</option>'}${threads.map((p) => `<option value="${esc(p.id)}"${p.id === item.thread ? ' selected' : ''}>${esc(p.name)}</option>`).join('')}</select>`
    return `<div class="td-modal" data-needs-off="1"><div class="td-dialog" role="dialog" aria-modal="true" aria-labelledby="nd-dlg">
      <h3 id="nd-dlg">A task of yours</h3><label for="nd-title" class="set-sub">What to do</label><textarea id="nd-title" rows="2">${esc(item.line || '')}</textarea>
      <label for="nd-thread" class="set-sub">In which thread</label>${pick}
      <div class="td-dialog-a"><button class="btn ghost" data-needs-off="1">Cancel</button><button class="btn primary" data-needs-todo-ok="${esc(c.id)}">Add to Tasks</button></div></div></div>`
  }
  const m = c.mail
  const body = !m ? '<p class="set-sub">Reading the mail…</p>' : m.error ? `<p class="warn">${esc(m.error)}</p>` : `<p><b>${esc(m.subject || '')}</b><br><span class="set-sub">${esc((m.from && (m.from.name || m.from.email)) || '')}</span></p><div class="nd-mail">${esc(m.text || '')}</div>`
  return `<div class="td-modal" data-needs-off="1"><div class="td-dialog wide" role="dialog" aria-modal="true" aria-labelledby="nd-dlg">
    <h3 id="nd-dlg">Read and reply</h3>${body}
    <label for="nd-reply" class="set-sub">Your answer — sent when you click Reply</label><textarea id="nd-reply" rows="4"></textarea>
    <div class="td-dialog-a">${m && m.url ? `<button class="link-btn" data-ext href="${esc(m.url)}" data-needs-outlook="${esc(m.url)}">Open in Outlook</button><span class="grow"></span>` : ''}<button class="btn ghost" data-needs-off="1">Cancel</button><button class="btn" data-needs-send="${esc(c.id)}" data-all="1">Reply all</button><button class="btn primary" data-needs-send="${esc(c.id)}">Reply</button></div></div></div>`
}

/** Today → the Claude dialog's Yolo: the task, its skill and the user's words go to Echo (IPC `task_yolo`), which has
 *  Claude Code do it on its own in the background (the thread's profile); Today reads the outcome (`asks`) until it is
 *  there — a run may take minutes. */
function todayYolo(c, skill, text) {
  const id = `ask-${Date.now()}`
  S.todayAskId = id
  send({ cmd: 'task_yolo', ask: id, project: c.project, node: c.node, skill, text })
  let n = 0
  const poll = () => loadToday().then(() => { render(); const a = ((S.today && S.today.asks) || []).find((x) => x.id === id); if ((!a || a.state === 'working') && ++n < 300) setTimeout(poll, 5000) })
  setTimeout(poll, 1000)
}
/** The last ask's outcome above My work: what Claude changed (Undo), why it didn't, or that it is working. */
function todayAskBar() {
  const a = S.todayAskId && ((S.today && S.today.asks) || []).find((x) => x.id === S.todayAskId)
  if (!S.todayAskId) return ''
  if (!a || a.state === 'working') return '<div class="td-askbar"><span class="set-sub">Yolo: Claude is on it…</span></div>'
  return `<div class="td-askbar${a.state === 'failed' ? ' failed' : ''}"><span class="grow">${esc(a.say || '')}</span><button class="icon-btn" data-today-askoff="1" aria-label="Dismiss">×</button></div>`
}

/** The Claude dialog for a task, two actions on the same skill and words: Work on it — into the Claude panel's live
 *  session, typed there as if by the user (`/my-skills:<id>`, the user's words); the conversation goes on there.
 *  Yolo (from Today) — `todayYolo`: Claude does it on its own, sending included. */
// ── skill fit: how well each skill fits a task (no DOM: `skill-fit.test.mjs`) ────────────────────────
/** The task's words (its title, its thread) in each skill's name and description — words of four letters or more,
 *  any alphabet, cut to a short stem (follow-up ~ follow, reports ~ report), the common ones left out — then how
 *  often the user chose it before. Best first, each with its `fit`. */
const SKILL_STOP = new Set(['send', 'what', 'when', 'task', 'user', 'next', 'step', 'with', 'that', 'this', 'from', 'have', 'will', 'your', 'after', 'before', 'then', 'them', 'they', 'into', 'about', 'make', 'need', 'once', 'agre', 'each', 'some', 'more', 'than', 'only', 'also', 'here', 'there', 'their', 'which', 'what', 'were', 'been', 'being', 'does', 'done', 'should', 'would', 'could', 'other', 'others', 'very', 'just', 'like'])
function rankSkills(skills, text, used) {
  const stem = (w) => w.replace(/(ing|ed|es|s)$/, '').slice(0, 7)
  const words = (t) => new Set(String(t || '').toLowerCase().split(/[^\p{L}\p{N}]+/u).filter((w) => w.length >= 4 && !SKILL_STOP.has(w)).map(stem).filter((w) => !SKILL_STOP.has(w)))
  const want = words(text)
  return skills
    .map((x) => {
      const hay = words(`${x.id.replace(/-/g, ' ')} ${x.description || ''}`)
      let fit = 0
      for (const w of want) if (hay.has(w)) fit++
      return { x, fit, used: (used && used[x.id]) || 0 }
    })
    .sort((a, b) => b.fit - a.fit || b.used - a.used || a.x.id.localeCompare(b.x.id))
}
// end of the skill fit block
const skillUse = () => { try { return JSON.parse(localStorage.getItem('echo.skillUse') || '{}') } catch { return {} } }
/** One more use of a skill, for the ranking's tie-break. */
const bumpSkillUse = (id) => { if (!id) return; const u = skillUse(); u[id] = (u[id] || 0) + 1; try { localStorage.setItem('echo.skillUse', JSON.stringify(u)) } catch { /* private mode */ } }
function sendClaudeDialog() {
  const c = S.claudeSend
  if (!c) return ''
  if (!TPL.data) loadTemplates().then(render)
  // Skills Claude can use (on and valid), found by name or description: many fit in one field.
  const skills = ((TPL.data && TPL.data.skills) || []).filter((x) => x.on && !(x.problems || []).length)
  const thread = ((S.pj.data && S.pj.data.projects) || []).find((p) => p.id === c.project)
  const ranked = rankSkills(skills, `${c.title} ${thread ? thread.name : ''}`, skillUse())
  // A clear best fit is chosen when the dialog opens (the cross takes it back to Just the task).
  if (c.skill === undefined && TPL.data) c.skill = ranked[0] && ranked[0].fit > 0 && (!ranked[1] || ranked[0].fit > ranked[1].fit) ? ranked[0].x.id : ''
  const best = ranked[0] && ranked[0].fit > 0 ? ranked[0].x.id : null
  const chosen = skills.find((x) => x.id === c.skill)
  const q = (c.q || '').trim().toLowerCase()
  const found = ranked.map((r) => r.x).filter((x) => !q || tplName(x.id).toLowerCase().includes(q) || (x.description || '').toLowerCase().includes(q)).slice(0, 6)
  // Yolo: from Today only (its outcome shows there).
  const yolo = c.today ? `<button class="btn" data-sc-yolo title="Claude does it on its own, in the background — sending included">${ICON('<path d="M12 3.5l1.9 5 5 1.9-5 1.9-1.9 5-1.9-5-5-1.9 5-1.9z"/><path d="M18.5 15.5l.8 2 2 .8-2 .8-.8 2-.8-2-2-.8 2-.8z"/>')}Yolo</button>` : ''
  const picker = chosen
    ? `<div class="sc-chosen"><b>${esc(tplName(chosen.id))}</b><span title="${esc(chosen.description)}">${esc(chosen.description)}</span><button class="icon-btn" data-sc-skill="" aria-label="No skill — just the task" title="No skill — just the task">${ICON('<path d="M6 6l12 12M18 6L6 18"/>')}</button></div>`
    : `<input id="sc-q" class="sc-q" placeholder="Just the task — or type to find a skill (${skills.length})" value="${esc(c.q || '')}" autocomplete="off" aria-label="Skill">
       ${c.open ? `<div class="sc-sugg" role="listbox">${found.map((x) => `<button class="sc-opt" role="option" data-sc-skill="${esc(x.id)}" title="${esc(x.description)}"><b>${esc(tplName(x.id))}${x.id === best ? ' <em class="sc-best">fits best</em>' : ''}</b><span>${esc(x.description)}</span></button>`).join('') || '<div class="set-sub" style="padding:6px 10px">No skill has that in its name or description.</div>'}</div>` : ''}`
  return `<div class="td-modal" data-sc-off="1"><div class="td-dialog sc-dialog" role="dialog" aria-modal="true" aria-labelledby="sc-h">
    <div class="sc-head"><h3 id="sc-h"><span class="md-ico claude">${ICON(CLAUDE_MARK)}</span>Claude</h3><div class="sc-task">${thread ? `${projIcon(thread, 14)}<span class="set-sub">${esc(thread.name)}</span> · ` : ''}<span>${esc(c.title)}</span></div></div>
    <div class="sc-pick">${picker}</div>
    ${c.hint ? `<div class="sc-hint set-sub" title="Echo's guess — Claude reads it with the task">Echo suggests: ${esc(c.hint)}</div>` : ''}
    <textarea id="sc-words" class="sc-words" rows="2" placeholder="Anything to add — optional (Keep it short; Dana is away until the 12th)">${esc(c.words || '')}</textarea>
    <div class="sc-foot"><button class="btn primary" data-sc-send title="In the Claude panel, with you">${ICON(CLAUDE_MARK)}Work on it</button>${yolo}<button class="btn ghost" data-sc-off="1">Cancel</button><span class="grow"></span><span class="set-sub">${c.today ? 'Yolo: Claude does it on its own.' : ''}</span></div></div></div>`
}
/** Type a line into the Claude panel's session: started (or resumed) first when it isn't running, and typed once it
 *  has drawn and gone quiet — earlier keys would land in its start-up. */
function sendToClaude(line, profile) {
  const key = 'claude:folder'
  // The task's text stays with its thread's AI profile: the panel starts on it, or — running on another — says so.
  const want = profile || ai().default || ''
  const old = termEntry(key), running = !!(old && old.id != null && !old.exited)
  if (running && (S.folderProfile || ai().default || '') !== want) {
    claudePanel('claude-folder')
    S.notice = `This thread works with ${profileName(want) || 'another AI profile'}: switch the Claude panel's profile to it, then send the task again.`
    return render()
  }
  if (!running) S.folderProfile = want === ai().default ? null : want
  claudePanel('claude-folder')
  const e = termEntry(key)
  if (!e) { S.notice = "The Claude panel can't be shown."; return render() }
  // Claude asking something (a permission, a choice) would take the keys as its answer: then nothing is typed.
  const asking = () => {
    // Around the cursor, where a prompt is drawn — the rows under it can be blank.
    const b = e.term.buffer.active, at = b.baseY + b.cursorY, lines = []
    for (let y = Math.max(0, at - 16); y <= Math.min(b.length - 1, at + 2); y++) lines.push((b.getLine(y) && b.getLine(y).translateToString(true)) || '')
    // Claude Code's own choice prompt (its ❯ cursor on option 1, "Esc to cancel"), not words in Claude's answer.
    return /❯\s*1\.|Esc to cancel|trust the files/.test(lines.join('\n'))
  }
  // The line, then Enter on its own a moment later: in one burst a terminal app takes it all for a paste.
  const type = () => {
    if (asking()) { S.notice = 'Claude is waiting for your answer in the panel — answer it, then send the task again.'; return render() }
    send({ cmd: 'pty_input', id: e.id, data: line.replace(/[\u0000-\u001f\u007f]+/g, ' ') })
    setTimeout(() => send({ cmd: 'pty_input', id: e.id, data: '\r' }), 150)
  }
  if (e.id != null && !e.exited && !e.starting) return type()
  // Not running: started (or resumed) — once; then typed when it has drawn something new and gone quiet.
  const since = Date.now()
  if (!e.starting) { e.lastData = 0; termStart('claude-folder', key, { profile: S.folderProfile || ai().default || '' }) }
  const wait = setInterval(() => {
    const quiet = e.lastData > since && Date.now() - e.lastData > 1500
    if (e.id != null && !e.exited && !e.starting && quiet) { clearInterval(wait); type() }
    else if (e.exited || Date.now() - since > 30000) { clearInterval(wait); S.notice = 'Claude didn\'t start — send it again once the panel shows Claude.'; render() }
  }, 300)
}
/** Today → Close…: a dialog — closing a task by hand asks first, with an optional comment (kept in its history).
 *  A thread's sync closes tasks by itself when its calls and mail say so; this is for what they don't. */
function todayCloseDialog() {
  const c = S.todayClose
  if (!c) return ''
  return `<div class="td-modal" data-today-keep="1"><div class="td-dialog" role="dialog" aria-modal="true" aria-labelledby="td-dlg-h">
    <h3 id="td-dlg-h">Close this task?</h3>
    <p>${esc(c.title)}</p>
    <label for="td-close-note" class="set-sub">A comment, if you like — why it's done, or where</label>
    <textarea id="td-close-note" rows="3" placeholder="Sent the diagram to Ravi on Tuesday"></textarea>
    <div class="td-dialog-a"><button class="btn ghost" data-today-keep="1">Cancel</button><button class="btn primary" data-today-done="${esc(c.node)}" data-project="${esc(c.project)}">Close it</button></div>
  </div></div>`
}

/** A meeting still to come (Calls → Coming up today, `echo://prep`): its header and the Prep tab; Transcript and
 *  Minutes come once it is recorded. */
function upcomingDoc() {
  const u = ((S.up && S.up.meetings) || []).find((x) => x.id === S.upSel)
  const p = S.upPrep && !S.upPrep.error ? S.upPrep : null
  const title = (u && u.subject) || (p && p.subject) || 'A meeting'
  const start = (u && u.start) || (p && p.start), end = (u && u.end) || (p && p.end)
  const when = start ? `${new Date(start).toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })}, ${hm(new Date(start))}` : ''
  // As a call's header: the thread's icon and name, opening it.
  const all = (S.pj.data && S.pj.data.projects) || []
  // A call's header, before the call: its day and time, length, people, thread chips (+ Thread, as in the calendar).
  const mins = start && end ? Math.round((new Date(end) - new Date(start)) / 60000) : 0
  const names = (u && u.names) || (p && p.people ? p.people.map((x) => x.name) : [])
  const people = names.length ? `<span class="people" title="${esc(names.join(', '))}">${esc(names.slice(0, 6).join(', '))}${names.length > 6 ? ` and ${names.length - 6} more` : ''}</span>` : ''
  const ev = u ? { id: u.event, subject: u.subject, projects: u.projects || [] } : null
  const head = `${listToggle()}<div class="head-row"><div style="min-width:0"><h2 class="selectable">${esc(title)}</h2>
      <div class="facts">${when ? `<span>${esc(when)}</span>` : ''}${mins ? `<span>${mins} min</span>` : ''}${people}</div>${ev ? eventProjects(ev) : ''}</div>
      <div class="doc-actions">${p ? `<button class="btn ghost" data-copy="${esc(p.link)}" title="Its echo:// link">Copy link</button>` : ''}</div></div>
    <div class="tabs" role="tablist"><button class="tab" role="tab" aria-selected="true">Prep</button><button class="tab" role="tab" aria-selected="false" disabled title="Once it is recorded">Transcript</button><button class="tab" role="tab" aria-selected="false" disabled title="Once it is recorded">Minutes</button></div>`
  // Being written (from the calendar, any day): read it again until it is there.
  const writing = (u && u.writing) || (S.upPrep && S.upPrep.writing)
  if (writing && !S.upPoll) S.upPoll = setTimeout(() => { S.upPoll = null; loadUpcoming().then(render) }, 2000)
  if (!u && !p) return { head, pane: writing ? '<div class="empty"><div><h3>Writing the prep…</h3><p>From last time, what is open in its threads, what is new and who is coming.</p></div></div>' : S.upPrep ? '<p class="set-sub">This meeting isn\'t in today\'s calendar any more.</p>' : `<div class="skeleton">${'<i></i>'.repeat(6)}</div>` }
  if (!p) {
    const busy = writing
    return { head, pane: `<div class="empty"><div><h3>${busy ? 'Writing the prep…' : 'No prep yet'}</h3><p>${busy ? 'From last time, what is open in its threads, what is new and who is coming.' : u && u.threads.length ? 'Echo writes it this morning for every meeting of a thread.' : 'This meeting is in no thread: Echo prepares it when you ask, from its people\'s mail and Teams.'}</p>${busy ? '' : `<button class="btn primary" data-prep-write="${esc(S.upSel)}">Prepare now</button>`}</div></div>` }
  }
  return { head, pane: prepView(p, !!u) }
}

/** A prep: In short, then Last time, Open with these people, New since then; Who's coming beside. `again`: it can
 *  be written again (the meeting is still to come). Its text is the user's own records, escaped. */
function prepView(p, again) {
  if (!p) return ''
  const day = (d) => d ? new Date(d.length > 10 ? d : d + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' }) : ''
  const sec = (title, items) => items ? `<section class="pr-sec"><h3>${title}</h3>${items}</section>` : ''
  const l = p.last_time
  const last = l ? sec(`Last time · <a href="${esc(l.link)}">${esc(day(l.date))}</a>`, `<ul>${(l.highlights || []).map((h) => `<li>${esc(h)}</li>`).join('')}${(l.open_actions || []).map((a) => `<li><b>Still to do</b> ${a.link ? `<a href="${esc(a.link)}">${esc(a.task)}</a>` : esc(a.task)}${a.owner ? ` · ${esc(a.owner)}` : ''}</li>`).join('')}</ul>`) : ''
  const KIND = { task: 'Task', risk: 'Risk', question: 'Question' }
  // What waits on the user: their tasks, what they still owe from last time, mail to them — or that nothing does.
  const exp = (p.expected || []).map((x) => {
    const title = x.kind === 'reply' ? (x.url && /^https:\/\//.test(x.url) ? `<a data-ext href="${esc(x.url)}">${esc(x.title)}</a>` : esc(x.title)) : x.link ? `<a href="${esc(x.link)}">${esc(x.title)}</a>` : esc(x.title)
    const what = x.kind === 'reply' ? `<b>Reply</b> to ${esc(x.from || '')}: ` : x.kind === 'action' ? '<b>From last time</b> ' : ''
    return `<li>${what}${title}${x.late ? ' · <span class="warn">late</span>' : x.due ? ` · ${esc(day(x.due))}` : ''}</li>`
  }).join('')
  const expected = p.expected ? sec('Expected from you', exp ? `<ul>${exp}</ul>` : '<p class="set-sub" style="margin:0">Nothing waits on you.</p>') : ''
  const open = (p.open || []).length ? sec('Open with these people', `<ul>${p.open.map((o) => `<li><b>${KIND[o.kind] || esc(o.kind)}</b> <a href="${esc(o.link)}">${esc(o.title)}</a>${o.owner ? ` · ${esc(o.owner)}` : ''}${o.late ? ' · <span class="warn">late</span>' : o.due ? ` · ${esc(day(o.due))}` : ''}</li>`).join('')}</ul>`) : ''
  const fresh = (p.new_since || []).length ? sec('New since then', `<ul>${p.new_since.map((n) => `<li>${esc(day(n.date))} · ${n.kind === 'mail' ? 'Mail' : 'Chat'}${n.from ? ` from ${esc(n.from)}` : ''}: ${n.url && /^https:\/\//.test(n.url) ? `<a data-ext href="${esc(n.url)}">${esc(n.title || '')}</a>` : esc(n.title || '')}${n.snippet ? ` <span class="set-sub">“${esc(n.snippet)}”</span>` : ''}</li>`).join('')}</ul>`) : ''
  // The new faces first, then by calls together; ten at most.
  const ppl = [...(p.people || [])].sort((a, b) => (b.new - a.new) || (b.calls - a.calls)), more = ppl.length - 10
  const who = ppl.slice(0, 10).map((x) => `<div class="pr-who"><span>${x.link ? `<a href="${esc(x.link)}">${esc(x.name)}</a>` : esc(x.name)}${x.role ? ` <span class="set-sub">· ${esc(x.role)}</span>` : ''}</span>${x.new ? '<span class="chip warn-chip">new</span>' : (x.calls ? `<span class="set-sub pr-n">${x.calls} call${x.calls === 1 ? '' : 's'}</span>` : '')}</div>`).join('') + (more > 0 ? `<div class="set-sub">and ${more} more</div>` : '')
  const short = p.summary ? sec('In short', `<p class="pr-short">${esc(p.summary)}</p>`) : p.error ? `<p class="set-sub">${esc(p.error)}</p>` : ''
  const foot = `<div class="pr-foot"><span class="set-sub">Written ${esc(p.written_at ? hm(new Date(p.written_at)) : '')}${p.model ? ` · ${esc(p.model)}` : ''}</span>${again ? `<button class="btn ghost" data-prep-write="${esc(p.id)}">Write it again</button>` : ''}</div>`
  const empty = !short && !last && !open && !fresh ? '<p class="set-sub">Nothing to go on yet: no earlier meeting with minutes, nothing open in its threads.</p>' : ''
  return `<div class="pr"><article class="pr-main">${short}${expected}${last}${open}${fresh}${empty}</article><aside class="pr-side">${who ? sec("Who's coming", who) : ''}${foot}</aside></div>`
}

/** The ⋯ menu's Write again with…: each minutes template but the one used now (the call's from then on), and the
 *  way to Settings → Templates. Not while minutes are being written. */
function writeAgainWith(c) {
  if (!TPL.data) { loadTemplates().then(render); return '' }
  const run = c.notes_run, busy = run && (run.state === 'queued' || run.state === 'running')
  const list = (TPL.data.templates || []).filter((x) => !x.memo && x.id !== c.template.id)
  const items = list.map((x) => `<button class="menu-item" role="menuitem" data-tpl-again="${esc(x.id)}"${busy ? ' disabled title="The minutes are being written now"' : ''}>${esc(tplName(x.id))}</button>`).join('')
  return `<div class="menu-sep" role="separator"></div><div class="menu-label">Write again with</div>${items}<button class="menu-item" role="menuitem" data-tpl-edit>Edit templates…</button><div class="menu-sep" role="separator"></div>`
}

/** A calendar meeting's type, from its invite (or a guess while it isn't typed yet); set here for its whole series. */
