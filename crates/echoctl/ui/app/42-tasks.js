// ── Tasks: every open project's open tasks by who has to move, and what to ask whom ────────
// Computed by the app (`tasks.rs`) from the ledgers; a card opens the task's details on the right.
const TASK_PILES = [['mine', 'I owe'], ['owed', 'Owed to me'], ['waiting', 'Waiting for others'], ['nobody', 'Nobody named']]
const TASK_SORTS = [['due', 'Due'], ['quiet', 'Quiet longest'], ['project', 'Thread']]
const TASK_SHOWN = 6
const projById = (id) => ((S.pj.data && S.pj.data.projects) || []).find((p) => p.id === id)
const dueText = (c) => c.due && c.due_guessed ? `<span class="tk-due guessed" title="Nobody said this date: never late">${esc(fmtDay(c.due))} · ${esc(dueFrom(c.due_from))}</span>`
  : c.due ? `<span class="tk-due${c.late ? ' late' : ''}" title="${c.late ? 'Overdue' : 'Due'}">${esc(fmtDay(c.due))}</span>` : '<span class="tk-due none">no date</span>'
/** Who a card is about (the user's rules, 2026-11-02): what you owe — its story, and what blocks
 *  it; owed — who does it (· client); waiting — who, and which of yours it stops; others — who. */
function taskWho(c, pile) {
  const stops = (c.blocks || []).length ? ` · stops ${c.blocks.map((b) => `<span title="${esc(b.title)}">${esc(b.id)}</span>`).join(', ')}` : ''
  if (pile === 'waiting') return c.wait ? `${esc(c.who || 'someone')}${stops}` : `${esc(c.owner || 'nobody named')}${c.client ? ' · client' : ''}${stops}`
  if (pile === 'owed' || pile === 'others') return `${esc(c.owner)}${c.client ? ' · client' : ''}${c.blocked ? ' · <span class="late">blocked</span>' : ''}`
  if (pile === 'mine' && c.blocked) return `${c.story ? esc(c.story.title) + ' · ' : ''}<span class="late">blocked</span>${c.waiting_for && c.waiting_for.who ? ` by ${esc(c.waiting_for.who)}` : c.after.length ? ` by ${c.after.map((a) => `<span title="${esc(a.title)}">${esc(a.id)}</span>`).join(', ')}` : ''}`
  return c.story ? esc(c.story.title) : ''
}
/** Others: other people's work in your threads that you neither handed over nor wait on — folded. */
function othersPile(all, key) {
  if (!all.length) return ''
  const open = S.tk.more[key]
  return `<section class="tk-others"><button class="tk-more" data-tk="more" data-pile="${key}">${CHEV(open)} Others · ${all.length} <span class="set-sub">— other people's work in your threads: not handed over by you, not stopping you</span></button>${open ? `<div class="tk-others-cards" role="region" aria-label="Others">${all.map((c) => taskCard(c, 'others')).join('')}</div>` : ''}</section>`
}
function taskCard(c, pile) {
  const p = projById(c.project) || { id: c.project, name: c.project }
  const quiet = c.quiet >= 5 && !c.due ? `<span class="tk-quiet" title="No word about it in ${c.quiet} days">quiet ${c.quiet} d</span>` : ''
  const on = S.pj.node === c.id && S.pj.sel === c.project
  return `<button class="tk-card${c.wait ? ' tk-wait' : ''}${on ? ' on' : ''}"${on ? ' aria-current="true"' : ''} data-tk="open" data-project="${esc(c.project)}" data-node="${esc(c.id)}">
    <span class="tk-t">${esc(c.title)}</span>${(() => { const w = taskWho(c, pile); return w ? `<span class="tk-who">${w}</span>` : '' })()}
    <span class="tk-m">${c.mine ? modeIcon(c.mode) : ''}<span class="tk-pj" title="${esc(p.name)}">${projIcon(p, 12)}<span class="tk-pjn">${esc(p.name)}</span></span>${(c.also_in || []).map((a) => { const q = projById(a.thread) || { id: a.thread, name: a.thread }; return `<span class="tk-pj" title="Also raised in ${esc(q.name)} (${esc(a.node)})">${projIcon(q, 12)}<span class="tk-pjn">${esc(q.name)}</span></span>` }).join('')}<span class="grow"></span>${quiet}${dueText(c)}</span>
  </button>`
}
/** Draws the Tasks view in the primary area (`renderNow`). */
function drawTasks(primary) {
  patch(primary, `<main class="main">${tasksView()}</main>`)
}
function tasksView() {
  const d = S.tk.data
  if (!d) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
  if (d.error) return `<div class="tk"><p class="error">${esc(d.error)}</p></div>`
  const projects = d.projects || []
  const pick = S.tk.project && projects.some((p) => p.id === S.tk.project) ? S.tk.project : ''
  const name = (id) => (projById(id) || { name: id }).name
  const sorts = {
    due: () => 0,
    quiet: (a, b) => (b.quiet || 0) - (a.quiet || 0),
    project: (a, b) => name(a.project).localeCompare(name(b.project)),
  }
  const projChip = `<label class="fchip${pick ? ' on' : ''}">${esc(pick ? name(pick) : 'All threads')} ▾<select data-tk-project aria-label="Thread"><option value="">All threads</option>${projects.map((p) => `<option value="${esc(p.id)}"${pick === p.id ? ' selected' : ''}>${esc(p.name)} · ${p.tasks}</option>`).join('')}</select></label>`
  const sortChip = `<label class="fchip">${esc((TASK_SORTS.find(([k]) => k === S.tk.sort) || TASK_SORTS[0])[1])} ▾<select data-tk-sort aria-label="Order">${TASK_SORTS.map(([k, l]) => `<option value="${k}"${S.tk.sort === k ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
  // Who does it (`taskmodes`): your tasks Claude can do, or the ones by hand.
  const MODES = [['', 'Anyone'], ['claude', 'Claude can do'], ['manual', 'By hand']]
  const modeChip = `<label class="fchip${S.tk.mode ? ' on' : ''}">${esc((MODES.find(([k]) => k === S.tk.mode) || MODES[0])[1])} ▾<select data-tk-mode aria-label="Who does it">${MODES.map(([k, l]) => `<option value="${k}"${S.tk.mode === k ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
  const head = `<header class="tk-head"><h2>Tasks</h2><span class="tk-sub">across ${projects.length} active project${projects.length === 1 ? '' : 's'}</span><span class="grow"></span>${projChip}${modeChip}${sortChip}</header>`
  if (!projects.length) return `<div class="tk">${head}<div class="wk-empty"><b>No active threads</b><p>Tasks come from your threads' calls: each thread's Work tab reads them into stories and tasks.</p><button class="btn primary" data-go="projects">Open Threads</button></div></div>`
  const total = TASK_PILES.reduce((n, [k]) => n + ((d.piles || {})[k] || []).length, 0) + ((d.piles || {}).others || []).length
  if (!total) return `<div class="tk">${head}<div class="wk-empty"><b>No open tasks</b><p>Once a thread's calls are synced, what was promised, owed and waited on shows here — from every thread at once.</p></div></div>`
  const cols = TASK_PILES.map(([k, label]) => {
    const all = ((d.piles || {})[k] || []).filter((c) => (!pick || c.project === pick) && (!S.tk.mode || (c.mode && c.mode.mode === S.tk.mode))).slice().sort(sorts[S.tk.sort] || sorts.due)
    const open = S.tk.more[k], shown = open ? all : all.slice(0, TASK_SHOWN)
    const more = all.length > TASK_SHOWN ? `<button class="tk-more" data-tk="more" data-pile="${k}">${open ? 'Show fewer' : `+ ${all.length - TASK_SHOWN}`}</button>` : ''
    return `<section class="tk-col" aria-label="${esc(label)}"><h3>${esc(label)} <span>${all.length}</span></h3>${shown.map((c) => taskCard(c, k)).join('') || '<p class="tk-none">Nothing here</p>'}${more}</section>`
  }).join('')
  const errs = (d.errors || []).length ? `<p class="set-sub tk-err">${d.errors.map((e) => `${esc(name(e.project))}: ${esc(e.error)}`).join(' · ')}</p>` : ''
  const others = ((d.piles || {}).others || []).filter((c) => (!pick || c.project === pick) && (!S.tk.mode || (c.mode && c.mode.mode === S.tk.mode))).slice().sort(sorts[S.tk.sort] || sorts.due)
  return `<div class="tk">${head}${errs}<div class="tk-cols">${cols}</div>${othersPile(others, 'others')}</div>`
}
/** What to ask whom, next time you meet (right panel): late, due-soon and quiet things they owe,
 *  and what we wait on them for. */
function asksPanel() {
  const d = S.tk.data
  const asks = ((d && d.asks) || []).map((a) => ({ ...a, items: a.items.filter((c) => !S.tk.project || c.project === S.tk.project) })).filter((a) => a.items.length)
  const when = (n) => {
    if (!n || !n.start) return 'no meeting this week'
    const s = new Date(n.start), t = new Date()
    const day = s.toDateString() === t.toDateString() ? 'today' : s.toLocaleDateString([], { weekday: 'short' })
    return `${day} · ${n.people <= 1 ? '1:1' : esc(n.subject || 'a meeting')}`
  }
  const why = (c) => c.why === 'late' ? `was due ${esc(fmtDay(c.due))}` : c.why === 'due' ? `due ${esc(fmtDay(c.due))}` : c.why === 'quiet' ? `quiet ${c.quiet} days` : `waiting${c.waiting_for && c.waiting_for.what ? ` · ${esc(c.waiting_for.what)}` : ''}`
  const rows = asks.map((a) => `<div class="ask" role="group" aria-label="${esc(a.who)}"><div class="ask-h"><b>${esc(a.who)}</b><span>${when(a.next)}</span></div>${a.items.map((c) => `<button class="ask-i" data-tk="open" data-project="${esc(c.project)}" data-node="${esc(c.id)}">${esc(c.title)} <span class="${c.why === 'late' ? 'late' : ''}">— ${why(c)}</span></button>`).join('')}</div>`).join('')
  return `<div class="asks"><div class="asks-h"><b>Asks · by person</b><span>next time you meet</span></div>${rows || '<p class="set-sub">Nothing to chase: nobody owes you anything late, due soon or gone quiet.</p>'}<p class="asks-foot">Computed from what went quiet, what's due and what's owed — never a model's guess.</p></div>`
}
PANEL_VIEWS.push({ id: 'asks', label: 'Asks', icon: ICON('<path d="M5 18.5V7a2.5 2.5 0 0 1 2.5-2.5h9A2.5 2.5 0 0 1 19 7v6a2.5 2.5 0 0 1-2.5 2.5H9z"/><path d="M10 9.2a2 2 0 1 1 2.6 1.9c-.4.2-.6.5-.6.9v.3M12 14v.01"/>'), area: 'right', when: () => S.view === 'tasks', render: asksPanel })
// A card that isn't a button (Today's tasks) opens with Enter or Space too.
document.addEventListener('keydown', (e) => {
  if ((e.key === 'Enter' || e.key === ' ') && e.target.matches && e.target.matches('[data-tk][role="button"]')) { e.preventDefault(); e.target.click() }
})
document.addEventListener('click', async (e) => {
  const b = e.target.closest && e.target.closest('[data-tk]')
  if (!b) return
  // A button or link inside a card that opens (Today's ✦ and Close) is its own.
  const inner = e.target.closest('button, a, input, select, textarea')
  if (inner && inner !== b && b.contains(inner)) return
  e.preventDefault()
  const d = b.dataset
  if (d.tk === 'more') { S.tk.more[d.pile] = !S.tk.more[d.pile]; return render() }
  if (d.tk === 'open') {
    if (S.pj.sel !== d.project) { S.pj.sel = d.project; S.pj.detail = null; S.pj.work = null; S.pj.tab = 'work' }
    S.pj.node = d.node
    S.pj.inline = S.view
    S.pj.editField = null
    const known = panelViews(), eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
    if (eff.narrow) S.overlay = true
    if (!S.pj.work) await loadWork()
    Layout.show('work-item', known)
    layoutChanged()
  }
})
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-cc-sync]')
  if (!b) return
  send({ cmd: 'project', op: 'sync', id: b.dataset.ccSync })
  b.disabled = true
  b.textContent = 'Reading…'
})
document.addEventListener('change', (e) => {
  const t = e.target
  if (t.matches && t.matches('[data-tk-project]')) { S.tk.project = t.value; render() }
  if (t.matches && t.matches('[data-tk-sort]')) { S.tk.sort = t.value; render() }
  if (t.matches && t.matches('[data-tk-mode]')) { S.tk.mode = t.value; render() }
})

/** Projects Echo would make from calls in none (`projects::found`): accept, merge, reject. */
function foundView(data) {
  const fd = S.pj.foundData
  const head = `<header class="pj-head"><div class="pj-title"><div style="min-width:0"><h2>Found for you</h2><div class="facts"><span>From your calls of the last 90 days that are in no thread — a recurring meeting, one organisation's calls, one Teams chat. Mail joins later.</span></div></div></div></header>`
  if (!fd) return `${head}<div class="pane pj-pane"><div class="skeleton">${'<i></i>'.repeat(6)}</div></div>`
  const clients = data.clients || [], projects = (data.projects || []).filter((p) => p.group !== 'closed')
  const cards = (fd.found || []).map((c) => {
    const calls = c.calls || []
    const titles = calls.slice(0, 4).map((x) => `<button class="pj-call" data-call="${esc(x.name)}"><span class="pj-call-d">${x.started_at ? esc(dayLabel(new Date(x.started_at))) : ''}</span><span class="t">${esc(x.title || x.name)}</span></button>`).join('')
    const more = calls.length > 4 ? `<div class="set-sub" style="padding:6px 14px">+ ${calls.length - 4} more</div>` : ''
    const nc = c.new_client
    const def = c.client || ''
    // The organisation it is with joins it as a counterpart (made from its domain when new) —
    // unless you make it the thread's client.
    const org = c.org ? `<div class="set-sub">With ${esc(c.org.name)}${c.org.new ? ` (${esc(c.org.domain)}) — a new organisation, not a client` : ''}: its counterpart</div>` : ''
    const clientSel = `<select class="found-client" aria-label="Client"><option value=""${def === '' ? ' selected' : ''}>No client</option>${nc ? `<option value="new"${def === 'new' ? ' selected' : ''}>New client: ${esc(nc.name)} (${esc(nc.domain)})</option>` : ''}${clients.map((x) => `<option value="${esc(x.id)}"${def === x.id ? ' selected' : ''}>${esc(x.name)}</option>`).join('')}</select>`
    const merge = projects.length ? `<select class="found-merge" aria-label="Add to a thread"><option value="">Add to a thread…</option>${projects.map((p) => `<option value="${esc(p.id)}">${esc(p.name)}</option>`).join('')}</select>` : ''
    return `<section class="set-card found-card" role="group" aria-label="${esc(c.name)}" data-key="${esc(c.key)}">
      <div class="set-row"><div class="found-top">${projIcon({ id: c.key, name: c.name, color: PJ_COLORS[ProjectArt.hash(c.key) % PJ_COLORS.length] }, 28)}<input class="found-name" value="${esc(c.name)}" aria-label="Thread name"></div>
        <div class="set-ctl"><button class="btn primary" data-pj="found-accept">Accept</button><button class="btn ghost" data-pj="found-reject" title="Not a thread — never suggested again">Reject</button></div></div>
      <div class="set-row"><div><div class="set-sub">${esc(c.why)}</div>${org}</div><div class="set-ctl">${clientSel}${merge}</div></div>
      <div class="pj-calls">${titles}${more}</div></section>`
  }).join('')
  const empty = `<p class="set-sub" style="margin:4px 2px">Nothing found right now. Echo suggests a thread when two or more calls that are in none share a meeting series, one outside organisation, or a Teams chat.</p>`
  const newRow = `<div class="pj-new"><span aria-hidden="true">+</span><input id="pj-new" placeholder="New thread — name it, Echo finds its calls" aria-label="New thread name" autocomplete="off"><span class="set-sub">Client</span><select id="pj-new-client" aria-label="Client"><option value="">None</option>${clients.map((x) => `<option value="${esc(x.id)}">${esc(x.name)}</option>`).join('')}</select><button class="btn" data-pj="create">Create</button></div>`
  return `${head}<div class="pane pj-pane">${newRow}${fd.error ? `<p class="error">${esc(fd.error)}</p>` : ''}${cards || empty}</div>`
}

function projectBrief(p) {
  if (S.pj.briefEdit != null) {
    return `<section class="set-sec"><div class="set-card"><div class="set-row"><div><b>What it is</b><div class="set-sub">Yours: goals, scope, who matters. Markdown. The minutes and Claude read it.</div></div><div class="set-ctl"><button class="btn ghost" data-pj="brief-cancel">Cancel</button><button class="btn primary" data-pj="brief-save">Save</button></div></div>
      <div class="me-edit"><textarea id="pj-brief" spellcheck="true" aria-label="The thread's brief (Markdown)">${esc(S.pj.briefEdit)}</textarea><div class="set-sub">⌘S / Ctrl+S saves, Esc cancels.</div></div></div></section>`
  }
  const body = p.brief && p.brief.trim() ? `<div class="doc me-doc" role="region" aria-label="The thread's brief">${p.brief_html}</div>` : '<div class="me-doc set-sub">Nothing yet — say what this thread is for, so the minutes and Claude know.</div>'
  const recent = (p.calls || []).slice(0, 5).map((c) => `<button class="pj-call" data-call="${esc(c.name)}"><span class="pj-call-d">${c.started_at ? esc(dayLabel(new Date(c.started_at))) : ''}</span><span class="t">${esc(c.title || c.name)}</span>${c.minutes ? '' : '<span class="set-sub">no minutes yet</span>'}</button>`).join('')
  return `${insightsBlock('thread', p.id, p.name)}<section class="set-sec"><div class="set-card"><div class="set-row"><div><b>What it is</b></div><div class="set-ctl"><button class="btn" data-pj="brief-edit">Edit</button></div></div>${body}</div></section>${pjLinked(p)}
    <section class="set-sec"><h3>Latest calls</h3><div class="set-card pj-calls">${recent || '<div class="set-row"><div class="set-sub">No calls yet: add a rule on Profile, or put a call in from its header.</div></div>'}</div></section>`
}

const WHY = { series: 'its meeting series', people: "the client's people", chat: 'its Teams chat', subject: 'its subject', pinned: 'added by hand' }
function projectCalls(p) {
  const rows = (p.calls || []).map((c) => {
    const d = c.started_at ? new Date(c.started_at) : null
    const ctl = c.why === 'pinned'
      ? `<button class="btn ghost" data-pj="member" data-call-name="${esc(c.name)}" data-how="rules" title="Leave it to the rules">Unpin</button>`
      : `<button class="btn ghost" data-pj="member" data-call-name="${esc(c.name)}" data-how="exclude" title="Take it out (the rules won't put it back)">Take out</button>`
    return `<div class="set-row" role="listitem"><div><button class="link pj-call-t" data-call="${esc(c.name)}">${esc(c.title || c.name)}</button><div class="set-sub">${d ? `${esc(dayLabel(d))}, ${esc(hm(d))} · ` : ''}${esc(WHY[c.why] || c.why)}${c.minutes ? '' : ' · no minutes yet'}</div></div><div class="set-ctl">${ctl}</div></div>`
  }).join('')
  const out = p.exclusions && p.exclusions.length ? `<p class="set-foot">${p.exclusions.length} call${p.exclusions.length === 1 ? '' : 's'} taken out by hand.</p>` : ''
  return `<section class="set-sec"><div class="set-card"${rows ? ' role="list" aria-label="Calls"' : ''}>${rows || '<div class="set-row"><div class="set-sub">No calls match yet. Add a rule on Profile — a meeting series, the client\'s people, a Teams chat — or open a call and add it from its header.</div></div>'}</div>${out}</section>`
}

/** A token (× removes it) and the dashed "+ Add" that opens an inline box in its place. */
const tok = (html, remove, title = '') => `<span class="tok"${title ? ` title="${esc(title)}"` : ''}>${html}${remove ? `<button data-pj="${remove.op}" ${Object.entries(remove).filter(([k]) => k !== 'op').map(([k, v]) => `data-${k}="${esc(v)}"`).join(' ')} aria-label="${remove.value != null ? esc(`Remove ${remove.value}`) : 'Remove'}">×</button>` : ''}</span>`
function addTok(what, placeholder, extra = '') {
  if (S.pj.adding !== what) return `<button class="tok add" data-pj="adding" data-what="${what}">+ Add</button>`
  return `<span class="tok-in">${extra}<input id="pj-add-${what}" data-add="${what}" placeholder="${esc(placeholder)}" aria-label="${esc(placeholder)}" autofocus></span>`
}
const RULE_TOKEN = (r, clientName) => r.kind === 'series' ? `series “${esc(r.subject)}”` : r.kind === 'people' ? (r.domains && r.domains.length ? `people from ${esc(r.domains.join(', '))} ≥ half` : `${esc(clientName || 'client')} people ≥ half`) : r.kind === 'chat' ? `from ${esc(r.name || 'a Teams chat')}` : `subject “${esc(r.contains)}”`
const shortDay = (iso) => new Date(iso).toLocaleDateString([], { day: 'numeric', month: 'short' })

/** A thread's sources for the Brain: Teams channels and mail (read, never copied: each section links
 *  back to the message), how each was last read, and the pickers. Folders come next. */
const m365 = { teams: null, channels: {}, state: null, stateAt: 0 }
function loadSourceState(force) {
  if (!force && Date.now() - m365.stateAt < 15000) return
  m365.stateAt = Date.now()
  get('api/brain/sources').then((d) => { m365.state = d; render() }).catch(() => {})
  get('api/brain/files').then((d) => { m365.files = d && !d.error ? d : {}; render() }).catch(() => {})
}
/** A folder's files in a few words: read, waiting for the page reader, not read. */
function folderSummary(root) {
  const fs = ((m365.files || {})[root]) || { n: 0 }
  if (!fs.n) return { line: 'Not read yet', n: 0 }
  const by = (st) => (fs.by_status || {})[st] || 0
  const read = by('indexed') + by('pending'), pending = fs.pending_pages || 0
  const not = fs.n - read, err = by('error')
  return { line: [`${read} file${read === 1 ? '' : 's'} read`, pending ? `${pending} page${pending === 1 ? '' : 's'} to read from their images` : '', not ? `${not} not read (${[by('twin') ? `${by('twin')} twin${by('twin') === 1 ? '' : 's'}` : '', by('skipped') ? `${by('skipped')} skipped` : '', by('unsupported') ? `${by('unsupported')} other formats` : '', err ? `${err} couldn't be read` : ''].filter(Boolean).join(', ')})` : ''].filter(Boolean).join(' · '), n: fs.n, err }
}
function sourceLabel(x) {
  if (x.kind === 'mail') {
    const who = x.id === 'any:*' ? 'any mail' : x.id.startsWith('person:') ? `mail with ${x.id.slice(7)}` : `mail with ${x.id.replace(/^domain:/, '')}`
    return (x.subject || []).length ? `${who} · subject: ${x.subject.join(', ')}` : who
  }
  return x.name || x.id
}
/** A thread's meeting chats, read into its work between calls (`ledger::chat_sync`), with the chats
 *  of its calls it doesn't read yet offered — whether or not the Brain is on. */
function chatSources(p) {
  const cs = p.chat_state || {}
  const chips = (p.sources || []).filter((x) => x.kind === 'teams_chat').map((x) => {
    const s = cs[x.id] || {}
    const info = s.error ? `Couldn't read it: ${s.error}` : s.read_at ? `Read ${relDay(s.read_at)} · ${s.days || 0} day${s.days === 1 ? '' : 's'} of messages between calls go into the work` : 'Read into the work between calls — at the next sync'
    return tok(`💬 ${esc(x.name || 'Meeting chat')}${s.error ? ' <span class="src-err">⚠</span>' : ''}`, { op: 'source-remove', kind: 'teams_chat', source: x.id }, info)
  }).join('')
  const offer = (p.chat_suggestions || []).map((c) => `<button class="tok add" data-pj="source-add" data-kind="teams_chat" data-source="${esc(c.id)}" data-name="${esc(c.name)}" title="The chat of ${c.calls} of its call${c.calls === 1 ? '' : 's'}: what is said there between calls goes into the work">+ 💬 ${esc(c.name)} chat</button>`).join('')
  return chips + offer
}
function threadSources(p, cl) {
  if (!brainOn()) {
    const kept = (p.sources || []).filter((x) => x.kind !== 'teams_chat').map((x) => tok(`${{ teams_channel: '#', mail: '✉', folder: '▣' }[x.kind] || '·'} ${esc(x.kind === 'mail' ? sourceLabel(x) : x.name || x.id)}`, null)).join('')
    return chatSources(p) + kept + `<div class="pf-sub">Recall is off, so channels, mail and folders aren't read into it — turn it on in Settings → Knowledge. Meeting chats, and mail already added here, are read into the work either way.</div>`
  }
  loadSourceState()
  const st = (m365.state && m365.state.state) || {}
  const icon = { teams_channel: '#', mail: '✉', teams_chat: '💬', folder: '▣' }
  const chip = (x) => {
    if (x.kind === 'folder') {
      const f = folderSummary(x.id)
      const ex = (x.exclude || []).length ? ` · not reading ${x.exclude.join(', ')}` : ''
      return tok(`▣ ${esc(x.name || x.id.split(/[\\/]/).pop())}${f.err ? ' <span class="src-err">⚠</span>' : ''}`, { op: 'source-remove', kind: 'folder', source: x.id }, `${x.id}\n${f.line}${ex}`)
        + (S.pj.adding === `exclude:${x.id}` ? `<span class="tok-in"><input id="pj-add-exclude" data-add="exclude" data-root="${esc(x.id)}" value="${esc((x.exclude || []).join(', '))}" placeholder="Don't read — e.g. Archive/, *draft*" aria-label="What not to read" autofocus></span>` : `<button class="tok add" data-pj="adding" data-what="exclude:${esc(x.id)}" title="Folders or names in it not to read">Don't read…</button>`)
    }
    const s = st[`${x.kind}:${x.id}`] || {}
    let info = s.error ? `Couldn't read it: ${s.error}` : s.read_at ? `Read ${relDay(s.read_at)} · ${s.found || 0} new` : 'Not read yet'
    // Mail also goes into the work (`ledger::mail_sync`): trusted senders' mails, one item each.
    const w = x.kind === 'mail' ? (p.mail_state || {})[x.id] : null
    if (w) info += w.error ? `\nInto the work: couldn't read it — ${w.error}` : w.read_at ? `\nInto the work: read ${relDay(w.read_at)} · ${w.found || 0} from trusted senders` : ''
    return tok(`${icon[x.kind] || '·'} ${esc(sourceLabel(x))}${s.error || (w && w.error) ? ' <span class="src-err">⚠</span>' : ''}`, { op: 'source-remove', kind: x.kind, source: x.id }, info)
  }
  const have = new Set((p.sources || []).map((x) => `${x.kind}:${x.id}`))
  const signedIn = !m365.state || m365.state.signed_in
  const folderBtn = `<button class="tok add" data-pj="source-folder" title="A folder of this thread's documents">▣ Folder</button>`
  const others = (p.sources || []).filter((x) => x.kind !== 'teams_chat')
  if (!signedIn) return chatSources(p) + others.map(chip).join('') + folderBtn + `<div class="pf-sub">Channels, mail and meeting chats need Microsoft 365 — sign in under Settings → Calendar & chat.</div>`
  const suggest = ((cl && cl.domains) || []).filter((d) => !have.has(`mail:domain:${d}`)).map((d) => `<button class="tok add" data-pj="source-add" data-kind="mail" data-source="${esc(d)}" title="Mail with anyone at ${esc(d)}">+ mail with ${esc(d)}</button>`).join('')
  let channel = `<button class="tok add" data-pj="adding" data-what="channel"># Channel</button>`
  if (S.pj.adding === 'channel') {
    if (!m365.teams) { m365.teams = []; get('api/m365/teams').then((d) => { m365.teams = Array.isArray(d) ? d : { error: d.error }; render() }).catch(() => {}) }
    const team = S.pj.srcTeam
    const chans = team ? m365.channels[team] : null
    if (team && !chans) { m365.channels[team] = []; get('api/m365/channels?team=' + encodeURIComponent(team)).then((d) => { m365.channels[team] = Array.isArray(d) ? d : []; render() }).catch(() => {}) }
    const teams = Array.isArray(m365.teams) ? m365.teams : []
    channel = `<span class="tok-in"><select data-src-team aria-label="Team" autofocus><option value="">${teams.length ? 'Team…' : m365.teams && m365.teams.error ? 'Teams unavailable' : 'Loading teams…'}</option>${teams.map((t) => `<option value="${esc(t.id)}"${t.id === team ? ' selected' : ''}>${esc(t.name)}</option>`).join('')}</select>${team ? `<select data-src-channel aria-label="Channel"><option value="">${chans && chans.length ? 'Channel…' : 'Loading…'}</option>${(chans || []).filter((c) => !have.has(`teams_channel:${team}|${c.id}`)).map((c) => `<option value="${esc(c.id)}" data-name="${esc(teams.find((t) => t.id === team).name + ' › ' + c.name)}">${esc(c.name)}${c.membership === 'private' ? ' (private)' : ''}</option>`).join('')}</select>` : ''}</span>`
  }
  const mail = S.pj.adding === 'mail' ? `<span class="tok-in"><input id="pj-add-mail" data-add="mail" placeholder="A domain, an address or * — acme.example, jane@acme.example" aria-label="Mail with" autofocus><input id="pj-add-mail-subject" data-add="mail" placeholder="Only subjects with — Acme Platform, APX (needed with *)" aria-label="Only subjects with"></span>` : `<button class="tok add" data-pj="adding" data-what="mail">✉ Mail</button>`
  return chatSources(p) + others.map(chip).join('') + suggest + channel + mail + folderBtn
    + `<div class="pf-sub">A meeting chat goes into the work between calls (what was done, moved or blocked). Channels, mail and folders are read into Recall, never copied: each section links to the post, the mail or the file's page. Mail is matched by its people's addresses, and by its subject when the source names words; a folder's pages that their text misses (diagrams, timelines) are read from their images.</div>`
}
document.addEventListener('change', (e) => {
  const t = e.target
  if (t.matches && t.matches('[data-src-team]')) { S.pj.srcTeam = t.value || null; return render() }
  if (t.matches && t.matches('[data-src-channel]') && t.value && S.pj.srcTeam) {
    const name = t.selectedOptions[0].dataset.name
    projectSend('source_add', { kind: 'teams_channel', source: `${S.pj.srcTeam}|${t.value}`, name })
    S.pj.adding = null; S.pj.srcTeam = null
    setTimeout(() => loadSourceState(true), 1500)
    return render()
  }
})

function projectProfile(p, data) {
  const id = esc(p.id)
  const row = (label, html) => `<div class="pf-row" role="group" aria-label="${esc(label)}"><span class="pf-l">${esc(label)}</span><div class="pf-v">${html}</div></div>`
  // Stage: the three groups side by side; its history as one line.
  const at = (st) => { for (let i = (p.stages || []).length - 1; i >= 0; i--) if (p.stages[i].stage === st) return i; return -1 }
  const passed = new Set((p.stages || []).map((c) => c.stage))
  const groups = ProjectArt.STAGES.map(([, glabel, xs]) => `<div class="pf-sg" role="group" aria-label="${esc(glabel)}"><span>${esc(glabel)}</span><div class="seg">${xs.map(([sid, label]) => `<button data-pj="stage" data-stage="${sid}" aria-pressed="${p.stage === sid}" class="${p.stage !== sid && passed.has(sid) ? 'passed' : ''}">${esc(label)}</button>`).join('')}</div></div>`).join('')
  const st = p.stages || []
  const hist = st.length ? [`${ProjectArt.stage(p.stage).label} since ${shortDay(st[st.length - 1].at)}`, ...st.slice(0, -1).reverse().slice(0, 3).map((c, i, arr) => { const j = st.length - 2 - i; return `${ProjectArt.stage(c.stage).label} ${shortDay(c.at)} → ${shortDay(st[j + 1].at)}` })].join(' · ') : ''
  const reason = st.length && st[st.length - 1].reason ? ` (${esc(st[st.length - 1].reason)})` : ''
  // Client: its chip (opens it) and domains; changed through the chip's menu.
  const clients = data.clients || []
  const cl = projClient(p.client)
  const clientPick = `<label class="fchip">${cl ? 'Change' : 'Choose'} ▾<select data-pj-field="client" data-id="${id}" aria-label="Client"><option value=""${!p.client ? ' selected' : ''}>No client</option>${clients.map((c) => `<option value="${esc(c.id)}"${p.client === c.id ? ' selected' : ''}>${esc(c.name)}</option>`).join('')}<option value="__new">+ New client…</option></select></label>`
  const newClient = S.pj.newClient ? `<div class="pj-inline"><input id="pj-client-name" placeholder="Client name" aria-label="Client name" autofocus><input id="pj-client-domains" placeholder="Mail domains, e.g. northwind.example" aria-label="Mail domains"><button class="btn primary" data-pj="client-add">Add</button><button class="btn ghost" data-pj="client-cancel">Cancel</button></div>` : ''
  // Counterparts: organisations the thread is with besides its client (their people, the people rule).
  const allOrgs = data.orgs || clients
  const mine = (p.orgs || []).map((o) => allOrgs.find((c) => c.id === o.client) || { id: o.client, name: o.client, domains: [] })
  const orgPick = S.pj.adding === 'org' ? `<span class="tok-in"><select id="pj-org-pick" aria-label="Organisation"><option value="">Organisation…</option>${allOrgs.filter((c) => c.id !== p.client && !c.internal && !mine.some((m) => m.id === c.id)).map((c) => `<option value="${esc(c.id)}">${esc(c.name)}${c.domains.length ? ` · ${esc(c.domains[0])}` : ''}</option>`).join('')}<option value="__new">+ New organisation…</option></select></span>`
    : '<button class="tok add" data-pj="adding" data-what="org">+ Organisation</button>'
  const newOrg = S.pj.newOrg ? `<div class="pj-inline"><input id="pj-org-name" placeholder="Organisation, e.g. Globex" aria-label="Organisation name" autofocus><input id="pj-org-domains" placeholder="Mail domains, e.g. globex.example" aria-label="Mail domains"><button class="btn primary" data-pj="org-new">Add</button><button class="btn ghost" data-pj="org-new-cancel">Cancel</button></div>` : ''
  const orgsRow = mine.map((c) => tok(esc(c.name), { op: 'org-unlink', org: c.id }, (c.domains || []).join(', '))).join('') + orgPick + newOrg
    + '<div class="pf-sub">Who this thread is with besides its client: their people are theirs in People, and the people rule counts their domains</div>'
  const clientRow = (cl ? `<button class="tok link" data-pj="client" data-id="${esc(cl.id)}" title="Open ${esc(cl.name)}">${clientIcon(cl, 14)}${esc(cl.name)}</button><span class="pf-sub pf-inl">${cl.domains.length ? `its domains: ${esc(cl.domains.join(', '))}` : 'no mail domains yet — the people rule needs them'}</span>` : '<span class="pf-sub pf-inl">No client — internal work, or not set</span>') + clientPick + newClient
  // Since: text; click to change.
  // Documents, posts and mail dated before this are history in the Brain: kept, not searched unless asked.
  const history = S.pj.pfEdit === 'history' ? `<input type="date" class="pf-in" data-pj-field="history_before" data-id="${id}" value="${esc(p.history_before || '')}" aria-label="History before" autofocus>`
    : `<button class="pf-text" data-pj="pf-edit" data-what="history">${p.history_before ? `Before ${esc(new Date(p.history_before + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' }))}` : 'None'}</button><span class="pf-sub pf-inl">· documents, posts and mail dated before it are history: kept, searched only when history is asked for</span>`
  const since = S.pj.pfEdit === 'since' ? `<input type="date" class="pf-in" data-pj-field="since" data-id="${id}" value="${esc(p.since || '')}" aria-label="Since" autofocus>`
    : `<button class="pf-text" data-pj="pf-edit" data-what="since">${p.since ? esc(new Date(p.since + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' })) : 'From its first call'}</button><span class="pf-sub pf-inl">· nothing older is read</span>`
  // Calls: the rules as tokens; how many match.
  const pins = (p.pins || []).length, outs = (p.exclusions || []).length
  const ruleKind = `<select id="pj-add-kind" aria-label="Kind of rule"><option value="series">Meeting series</option><option value="people">The client's people</option><option value="subject">Subject contains</option><option value="chat">Teams chat id</option></select>`
  const calls = (p.rules || []).map((r, i) => tok(RULE_TOKEN(r, p.client_name), { op: 'rule-remove', index: i })).join('') + addTok('rule', "a meeting's subject (empty for the client's people)", ruleKind)
    + `<div class="pf-sub">${p.call_count} call${p.call_count === 1 ? '' : 's'} match${pins || outs ? ` · ${[pins ? `${pins} pinned` : '', outs ? `${outs} taken out by hand` : ''].filter(Boolean).join(', ')}` : ''}</div>`
  const sources = threadSources(p, cl)
  // The plugins' rows (`thread.profile`), under Sources; drawn again after the page is in (`drawProjects`).
  const plugRows = threadSlotRows(Plugins.slots['thread.profile'], slotThread(p), (m) => console.warn(`thread.profile: ${m}`))
    .map((r) => row(r.label, `<div class="pf-plug" data-thread-slot="${esc(r.key)}">${r.html}</div>`)).join('')
  const notes = S.pj.pfEdit === 'notes' ? `<textarea class="pf-in pj-notes" id="pj-f-notes" data-pj-field="notes" data-id="${id}" rows="3" aria-label="Notes for Claude" autofocus>${esc(p.notes || '')}</textarea>`
    : `<button class="pf-text${p.notes ? '' : ' empty'}" data-pj="pf-edit" data-what="notes">${p.notes ? esc(p.notes) : 'What the minutes and Claude should know — what people call things, what to leave out'}</button>`
  // Look and AI: not on the board; the same row style.
  const colors = PJ_COLORS.map((c) => `<button class="pj-swatch${p.color === c ? ' on' : ''}" style="background:${c}" data-pj="color" data-color="${c}" aria-label="Colour ${c}" aria-pressed="${p.color === c}"></button>`).join('')
  // Its look: a pattern made from it (in its colour), or its own logo — the symbols and the client's logo were dropped
  // (the user, 2026-11-05); a thread that picked one still shows it.
  const look = `<div class="pf-look">${p.has_logo ? '' : `<span title="Made from the thread, in its colour">${projIcon({ ...p, icon: null }, 22)}</span>`}<span class="pj-swatches">${colors}</span><span class="pj-logo-drop" data-drop-op="logo" data-id="${id}" title="Drop an image here">${p.has_logo ? projIcon(p, 22) : '<span class="pj-logo-empty">logo</span>'}</span>${logoPicker('logo', p.id, p.has_logo ? 'Change logo…' : 'Use a logo…')}${p.has_logo ? '<button class="link-btn" data-pj="logo-remove">Remove</button>' : ''}${p.icon && !p.has_logo ? '<button class="link-btn" data-pj="icon" data-icon="">Use the pattern</button>' : ''}</div>`
  const profiles = ai().profiles || []
  const aiRow = `<label class="fchip pf-start">${esc(p.ai_profile ? profileName(p.ai_profile) : `The default · ${profileName(ai().default) || 'built in'}`)} ▾<select data-pj-field="ai_profile" data-id="${id}" aria-label="AI profile"><option value="">The default (${esc(profileName(ai().default) || 'built in')})</option>${profiles.map((x) => `<option value="${esc(x.id)}"${p.ai_profile === x.id ? ' selected' : ''}>${esc(x.name)}</option>`).join('')}</select></label>`
    + (S.pj.pfEdit === 'model' ? `<input class="pf-in" data-pj-field="model" data-id="${id}" value="${esc(p.model || '')}" placeholder="sonnet" aria-label="Model" autofocus>` : `<button class="tok" data-pj="pf-edit" data-what="model">model: ${esc(p.model || "the profile's minutes model")}</button>`)
    + `<div class="pf-sub">What Claude runs on for this thread's sync and questions, whichever profile a call was recorded on.</div>`
  return `<div class="pf-card">
    ${row('Stage', `<div class="pf-stages">${groups}</div><div class="pf-sub">${esc(hist)}${reason}</div>`)}
    ${row('Client', clientRow)}
    ${row('Organisations', orgsRow)}
    ${row('Since', since)}
    ${brainOn() ? row('History', history) : ''}
    ${row('Calls', calls)}
    ${row('Meetings', seriesBlock(p))}
    ${row('Sources', sources)}
    ${plugRows}
    ${row('Notes for Claude', notes)}
    ${row('Look', look)}
    ${row('AI', aiRow)}
  </div>
  <p class="pf-foot">Saved as you change it · <code>projects/${id}/</code> · Delete in ⋯</p>`
}

/** What a series' rule does, in a few words (no rule: not recorded). */
function seriesSummary(r) {
  r = r || {}
  const parts = []
  parts.push(r.record === 'always' ? 'recorded' : r.record === 'ask' ? 'asks to record' : 'not recorded')
  if (r.languages && r.languages.length) parts.push(r.languages.map(langName).join(', '))
  return parts.join(' · ')
}

/** A thread's meeting series (Profile → Meetings): what Echo does with each, read only — set from a meeting in the
 *  calendar —; × takes one out of the thread (`series_unlink`). */
function seriesBlock(p) {
  const d = S.pj.series && S.pj.series.id === p.id ? S.pj.series : null
  if (!d) return '<span class="pf-sub">Reading its meetings…</span>'
  if (d.error) return `<span class="pf-sub">${esc(d.error)}</span>`
  const list = d.series || []
  if (!list.length) return '<span class="pf-sub">No meeting series yet. A meeting that recurs in this thread shows here.</span>'
  const when = (x) => x.next ? `next ${esc(new Date(x.next).toLocaleString([], { weekday: 'short', hour: '2-digit', minute: '2-digit' }))}` : x.calls ? `${x.calls} call${x.calls === 1 ? '' : 's'}` : ''
  return list.map((x) => `<div class="se-row"><span class="se-t">${esc(x.subject || 'A meeting')}</span><span class="pf-sub se-when">${when(x)}<button class="pj-x" data-series-unlink="${esc(x.key)}" title="Not this thread's: its meetings stop joining it" aria-label="Take ${esc(x.subject || 'this series')} out of the thread">×</button></span><span class="pf-sub se-what">${esc(seriesSummary(x.rule))}</span></div>`).join('')
    + '<div class="pf-sub">Set what Echo does with a series on one of its meetings in the calendar.</div>'
}

/** Any meeting in the calendar: Prepare it (a thread's meetings get one each morning by themselves); once written,
 *  Show prep (Calls → Coming up today, or the recorded call's Prep tab) and Prepare again. The card's action strip
 *  takes the button (`btn`) beside its main one and the words (`line`) into its line. */
function eventPrep(e, st) {
  const p = e.prep
  if (!p) return { btn: '', line: '' }
  if (p.writing) return { btn: '', line: 'Writing the prep…' }
  if (p.ready) return { btn: `<button class="btn" data-ev-prep="${esc(p.id)}">Show prep</button>`, line: st.past ? '' : `<button class="link-btn" data-prep-write="${esc(p.id)}" data-prep-open="1">Prepare again</button>` }
  if (st.past) return { btn: '', line: '' }
  return { btn: `<button class="btn" data-prep-write="${esc(p.id)}" data-prep-open="1">Prepare</button>`, line: 'Prepare writes last time, what is open, what is new and who is coming.' }
}

/** A recurring meeting in the calendar: record every meeting of its series, ask at each ("Record?"), or not (the
 *  default), and in which language — that's all. A choice on the Record button for one meeting still wins. */
function seriesEditor(e) {
  const x = e.series
  if (!x || !x.key) return ''
  // A Zoom or Google Meet meeting: no choice to make yet (#228).
  if (otherApp(e)) return `<div class="se-cal"><div class="ec-cap">Every meeting of this series</div><div class="set-sub">${notYet(e)}</div></div>`
  const r = x.rule || {}
  // Not recorded unless set: Don't record is the default (no rule, or never); Ask offers "Record?" at each.
  const rec = r.record === 'always' ? 'always' : r.record === 'ask' ? 'ask' : null
  const btn = (on, v, l) => `<button data-series-set="record" data-val="${esc(JSON.stringify(v))}" aria-pressed="${on}">${esc(l)}</button>`
  const opts = [['', 'Default language'], ...LANG_LIST.map((l) => [l, langName(l)])], cur = (r.languages || [])[0] || ''
  const lang = `<label class="fchip">${esc((opts.find(([v]) => v === cur) || opts[0])[1])} ▾<select data-series-field="languages" aria-label="Language">${opts.map(([v, l]) => `<option value="${esc(v)}"${v === cur ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
  return `<div class="se-cal"><div class="ec-cap">Every meeting of this series</div>
    <div class="se-line" data-series-key="${esc(x.key)}" data-series-subject="${esc(e.subject || '')}"><div class="seg" role="group" aria-label="Record every meeting">${btn(rec === 'always', 'always', 'Record')}${btn(rec === 'ask', 'ask', 'Ask')}${btn(!rec, null, "Don't record")}</div>${lang}</div></div>`
}

// ── client tabs: a client page's tabs, each with its count (no DOM: `client-tabs.test.mjs`) ─────────────
/** `calls`: the client's calls (null until loaded); `tasks`: its open tasks; `people`: its people. A zero shows
 *  no number, on every tab alike (#173). */
function clientTabs(calls, tasks, people) {
  const n = (k) => k ? `<span class="n">${k}</span>` : ''
  return [['overview', 'Overview'], ['calls', `Calls${n(calls && calls.length)}`], ['tasks', `Tasks${n(tasks)}`], ['people', `People${n(people)}`], ['profile', 'Profile']]
}
// end of the client tabs block
/** A client's page (`#client/<id>`): its names, domains, whether it is the user's own company, its logo and projects. */
function clientView(data) {
  const c = (data.clients || []).find((x) => x.id === S.pj.client)
  if (!c) return `<div class="empty"><div><h3>This client is gone</h3><p><a href="#projects">Back to the threads</a></p></div></div>`
  const id = esc(c.id)
  const row = (label, html) => `<div class="pf-row"${label ? ` role="group" aria-label="${esc(label)}"` : ''}><span class="pf-l">${esc(label)}</span><div class="pf-v">${html}</div></div>`
  const ps = (data.projects || []).filter((p) => p.client === c.id)
  const name = S.pj.pfEdit === 'cname' ? `<input class="pf-in" id="pj-cname" data-client-field="name" data-id="${id}" value="${esc(c.name)}" aria-label="Name" autofocus>` : `<button class="pf-text" data-pj="pf-edit" data-what="cname">${esc(c.name)}</button>`
  const aliases = (c.aliases || []).map((a) => tok(esc(a), { op: 'client-list-remove', list: 'aliases', value: a })).join('') + addTok('alias', 'Another name — e.g. NW, Northwind Group')
  const domains = (c.domains || []).map((d) => tok(esc(d), { op: 'client-list-remove', list: 'domains', value: d })).join('') + addTok('domain', 'A mail domain — e.g. northwind.example')
    + `<div class="pf-sub">${c.internal ? 'Its people are your colleagues: they never count as a client\'s people.' : 'A call is this client\'s when half of its outside people come from these domains.'}</div>`
  const internal = `<button class="sw${c.internal ? ' on' : ''}" role="switch" aria-checked="${!!c.internal}" aria-label="Your own company" data-pj="client-internal" data-on="${!c.internal}"></button><span class="pf-sub">${c.internal ? 'Your own company (internal work)' : 'A client, partner or vendor'}</span>`
  const logo = `<div class="pf-look"><span class="pj-logo-drop" data-drop-op="client_logo" data-id="${id}" title="Drop an image here">${clientIcon(c, 28) || '<span class="pj-logo-empty">logo</span>'}</span>${logoPicker('client_logo', c.id, c.logo_v ? 'Change logo…' : 'Add a logo…')}${c.logo_v ? `<button class="link-btn" data-pj="client-logo-remove" data-id="${id}">Remove</button>` : ''}</div><div class="pf-sub">PNG, JPEG or WebP, up to 512 KB — shown beside its name and on its threads that wear it.</div>`
  const projects = ps.map((p) => `<button class="pf-proj" data-pj="open" data-id="${esc(p.id)}">${projIcon(p, 18)}<span class="t">${esc(p.name)}</span>${stageTag(p.stage)}<span class="pf-sub">${p.call_count} call${p.call_count === 1 ? '' : 's'}</span></button>`).join('')
    + (S.pj.adding === 'cproject' ? `<span class="tok-in"><input id="pj-add-cproject" data-add="cproject" placeholder="New thread for ${esc(c.name)}" aria-label="New thread name" autofocus></span>` : `<button class="tok add" data-pj="adding" data-what="cproject">+ New thread</button>`)
  const del = S.pj.confirmDelete ? `<span class="pf-sub">Remove ${esc(c.name)}?</span><button class="btn ghost" data-pj="delete-cancel">Cancel</button><button class="btn danger" data-pj="client-remove" data-id="${id}">Remove</button>`
    : ps.length ? `<span class="pf-sub">A client with threads stays: move or delete them first.</span>` : `<button class="btn ghost danger" data-pj="delete-ask">Remove this client…</button>`
  const cd = S.pj.cdata && S.pj.cdata.client && S.pj.cdata.client.id === c.id ? S.pj.cdata : null
  const openTasks = cd && cd.work ? TASK_PILES.reduce((n, [k]) => n + ((cd.work.piles || {})[k] || []).length, 0) : 0
  const tabs = clientTabs(cd && cd.calls, openTasks, peopleCount())
  const tab = tabs.some(([t]) => t === S.pj.ctab) ? S.pj.ctab : 'overview'
  const head = `<header class="pj-head"><div class="pj-title">${clientIcon(c, 34) || `<span class="pj-ic pf-cic" style="--pj:#8a8a95;width:34px;height:34px">${esc(c.name.slice(0, 1).toUpperCase())}</span>`}<div style="min-width:0"><div class="pj-name"><h2>${esc(c.name)}</h2><span class="tag">${c.internal ? 'your company' : 'client'}</span></div><div class="facts"><span>${ps.length} thread${ps.length === 1 ? '' : 's'}${c.domains.length ? ` · ${esc(c.domains.join(', '))}` : ''}</span></div></div><span class="grow"></span><button class="btn" data-claude-about title="Claude, with Recall, about ${esc(c.name)}">Claude about ${esc(c.name)}</button></div>
    <div class="tabs" role="tablist">${tabs.map(([t, label]) => `<button class="tab" role="tab" data-pj="ctab" data-tab-id="${t}" aria-selected="${tab === t}">${label}</button>`).join('')}</div></header>`
  if (tab !== 'profile') {
    if (!cd) return `${head}<div class="pane pj-pane"><div class="skeleton">${'<i></i>'.repeat(6)}</div></div>`
    if (cd.error) return `${head}<div class="pane pj-pane"><p class="error">${esc(cd.error)}</p></div>`
    return `${head}<div class="pane pj-pane${tab === 'tasks' || tab === 'people' ? ' wide' : ''}">${tab === 'calls' ? clientCalls(cd) : tab === 'tasks' ? clientTasks(cd) : tab === 'people' ? peopleView() : clientOverview(cd, c)}</div>`
  }
  return `${head}
  <div class="pane pj-pane"><div class="pf-card">
    ${row('Name', name)}
    ${row('Other names', aliases)}
    ${row('Mail domains', domains)}
    ${row('Kind', internal)}
    ${row('Logo', logo)}
    ${row('Threads', projects)}
    ${row('', del)}
  </div><p class="pf-foot">Saved as you change it · <code>projects/clients.json</code></p></div>`
}

/** A client's Overview: its description (the user's, edited in place as a thread's brief) and a card
 *  per thread — stage, calls, stories done, open tasks by who has to move, the latest decisions,
 *  what's open (risks, questions). */
function clientOverview(cd, c) {
  const desc = S.pj.briefEdit != null
    ? `<section class="set-sec"><div class="set-card"><div class="set-row"><div><b>About ${esc(c.name)}</b><div class="set-sub">Yours: who they are, what you do for them, who matters. Markdown.</div></div><div class="set-ctl"><button class="btn ghost" data-pj="brief-cancel">Cancel</button><button class="btn primary" data-pj="brief-save">Save</button></div></div>
      <div class="me-edit"><textarea id="pj-brief" spellcheck="true" aria-label="About the client (Markdown)">${esc(S.pj.briefEdit)}</textarea><div class="set-sub">⌘S / Ctrl+S saves, Esc cancels.</div></div></div></section>`
    : `<section class="set-sec"><div class="set-card"><div class="set-row"><div><b>About ${esc(c.name)}</b></div><div class="set-ctl"><button class="btn" data-pj="brief-edit">Edit</button></div></div>${cd.brief && cd.brief.trim() ? `<div class="doc me-doc" role="region" aria-label="${esc(`About ${c.name}`)}">${cd.brief_html}</div>` : '<div class="me-doc set-sub">Nothing yet — who they are, what you do for them, who matters.</div>'}</div></section>`
  const PILE_SHORT = { mine: 'I owe', owed: 'owed to me', waiting: 'waiting', nobody: 'nobody named', others: 'others' }
  const card = (p) => {
    const s = p.summary || {}, pl = s.piles || {}
    const tasks = TASK_PILES.filter(([k]) => pl[k]).map(([k]) => `<span><b>${pl[k]}</b> ${PILE_SHORT[k]}</span>`).join('')
    // Each with what it holds: the decision and who took it; a risk's response — none stands out.
    const recLine = (x, cls) => {
      const sub = cls === 'risk' ? (x.what ? `↳ ${x.what}` : 'no response yet') : x.what ? `${x.what}${x.by ? ` — ${x.by}` : ''}` : ''
      const lvl = cls === 'risk' && (x.likelihood || x.impact) ? ` <span class="set-sub">${[x.likelihood && `likelihood ${LEVEL_LABEL[x.likelihood]}`, x.impact && `impact ${LEVEL_LABEL[x.impact]}`].filter(Boolean).join(' · ')}</span>` : ''
      return `<button class="cl-rec${cls === 'risk' && !x.what ? ' bare' : ''}" data-pj="goto-node" data-id="${esc(p.id)}" data-node="${esc(x.id)}">${esc(x.title)}${lvl}${sub ? `<span class="cl-rec-sub">${esc(sub)}</span>` : ''}</button>`
    }
    const recs = (label, xs, cls) => xs && xs.length ? `<div class="cl-recs"><span class="cl-rl ${cls}">${label}</span><div>${xs.slice(0, 3).map((x) => recLine(x, cls)).join('')}${xs.length > 3 ? `<span class="set-sub">+ ${xs.length - 3} more</span>` : ''}</div></div>` : ''
    const prog = s.stories ? `<div class="cl-prog"><span class="bd-prog"><span style="width:${Math.round(100 * s.stories_done / s.stories)}%"></span></span><span class="set-sub">${s.stories_done} of ${s.stories} stor${s.stories === 1 ? 'y' : 'ies'} done</span></div>` : ''
    return `<div class="cl-card${p.group === 'closed' ? ' closed' : ''}" role="group" aria-label="${esc(p.name)}">
      <button class="cl-top" data-pj="open" data-id="${esc(p.id)}">${projIcon(p, 22)}<b>${esc(p.name)}</b>${stageTag(p.stage)}</button>
      <div class="set-sub">${p.call_count} call${p.call_count === 1 ? '' : 's'}${p.last_call ? ` · last ${esc(dayLabel(new Date(p.last_call)).toLowerCase())}` : ''}</div>
      ${prog}
      ${tasks || s.late ? `<div class="cl-tasks">${tasks}${s.late ? `<span class="late">${s.late} late</span>` : ''}</div>` : '<div class="cl-tasks none">No open tasks</div>'}
      ${recs('Decided', s.decisions, '')}${recs('Risks', s.risks, 'risk')}${recs('Open questions', s.questions, '')}
    </div>`
  }
  const open = cd.threads.filter((p) => p.group !== 'closed'), closed = cd.threads.filter((p) => p.group === 'closed')
  return `${insightsBlock('client', c.id, c.name)}${desc}<section class="set-sec"><h3>Threads</h3>${cd.threads.length ? `<div class="cl-grid">${open.map(card).join('')}${closed.map(card).join('')}</div>` : '<div class="set-card"><div class="set-row"><div class="set-sub">No threads for this client yet — add one on Profile.</div></div></div>'}</section>`
}
/** Every call of the client's threads, newest first, with the threads each is in. */
function clientCalls(cd) {
  const rows = cd.calls.map((c) => {
    const d = c.started_at ? new Date(c.started_at) : null
    const chips = (c.threads || []).map((id) => projById(id)).filter(Boolean).map((p) => `<span class="tk-pj" title="${esc(p.name)}">${projIcon(p, 12)}<span class="tk-pjn">${esc(p.name)}</span></span>`).join('')
    return `<div class="set-row" role="listitem"><div><button class="link pj-call-t" data-call="${esc(c.name)}">${esc(c.title || c.name)}</button><div class="set-sub">${d ? `${esc(dayLabel(d))}, ${esc(hm(d))}` : ''}${c.minutes ? '' : ' · no minutes yet'}</div></div><div class="set-ctl cl-chips">${chips}</div></div>`
  }).join('')
  return `<section class="set-sec"><div class="set-card"${rows ? ' role="list" aria-label="Calls"' : ''}>${rows || '<div class="set-row"><div class="set-sub">No calls in this client\'s threads yet.</div></div>'}</div></section>`
}
/** The Tasks activity's piles for this client's open threads. */
function clientTasks(cd) {
  const cols = TASK_PILES.map(([k, label]) => {
    const all = (cd.work.piles || {})[k] || []
    return `<section class="tk-col" aria-label="${esc(label)}"><h3>${esc(label)} <span>${all.length}</span></h3>${all.map((c) => taskCard(c, k)).join('') || '<p class="tk-none">Nothing here</p>'}</section>`
  }).join('')
  return `<div class="tk-cols">${cols}</div>${othersPile((cd.work.piles || {}).others || [], 'client-others')}`
}

