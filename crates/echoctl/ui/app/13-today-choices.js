// ── The day's choices (`core:calendar.choices`, #315; design: Workbench `WidgetsCalendar`): meetings you accepted at
// once — Echo suggests one and says why — and invites still to answer — accept when the time is free, decline when you
// go elsewhere then. You decide: Accept, Maybe or Decline is sent to the organiser through Echo's Microsoft 365 sign-in
// (`event_respond`, window/today.rs → m365::event_respond), only on your click; "Keep" one of an overlap declines the
// others after a second click that names them.

// ── day choices: what to decide today (no DOM: `today-choices.test.mjs`) ─────────────────────────────────────────
/** Today's choices still to make, in time order: `{kind: 'overlap', members, pick, why, at}` for two or more you accepted
 *  at once (not over) — `pick` null when Echo has no reason to suggest one (your call) — and `{kind: 'invite', item,
 *  suggest: 'accept'|'decline', why, at}` for an invite still to answer. `ctx`: `due` — the threads with a promise of
 *  yours due today or late; `clients` — client id → name. */
function dayChoices(day, now, ctx = {}) {
  const t = (s) => new Date(s)
  const yes = (x) => !['notResponded', 'tentativelyAccepted', 'declined'].includes(x.response)
  const len = (x) => t(x.end || x.start) - t(x.start)
  const out = []
  // Meetings you accepted, not over, in clusters of ones at once (a chain counts: A with C, C with B) — the same rule
  // as widgets::has_choices, so the widget shows whenever the CLI says there is a choice.
  const open = (day || []).filter((x) => x.kind === 'meeting' && x.start && yes(x) && t(x.end || x.start) > now).sort((a, b) => t(a.start) - t(b.start) || len(b) - len(a))
  const clusters = []
  let cur = [], curEnd = 0
  for (const x of open) {
    if (cur.length && t(x.start) >= curEnd) { clusters.push(cur); cur = [] }
    if (!cur.length) curEnd = 0
    cur.push(x)
    curEnd = Math.max(curEnd, +t(x.end || x.start))
  }
  if (cur.length) clusters.push(cur)
  for (const members of clusters) {
    if (members.length < 2) continue
    // Echo suggests one only for a reason the design names, met by exactly one of them (the user's decision,
    // 2026-11-11): you organise it, else you're a required attendee (#402), else a promise of yours in its thread is
    // due, else it is a client's. No reason: no suggestion — your call.
    const due = ctx.due || new Set(), clients = ctx.clients || {}
    const dueIn = (x) => (x.threads || []).find((p) => due.has(p.id))
    const clientOf = (x) => (x.threads || []).find((p) => p.client && clients[p.client])
    const rules = [
      [(x) => x.response === 'organizer', () => 'you organise it'],
      [(x) => x.invited_as === 'required', () => "you're a required attendee"],
      [(x) => !!dueIn(x), (x) => `a promise in ${dueIn(x).name} is due`],
      [(x) => !!clientOf(x), (x) => `${clients[clientOf(x).client]}'s`],
    ]
    let pick = null, why = ''
    for (const [has, say] of rules) {
      const hits = members.filter(has)
      if (hits.length === 1) { pick = hits[0]; why = say(pick); break }
    }
    out.push({ kind: 'overlap', members, pick, why, at: t(members[0].start) })
  }
  const going = (day || []).filter((x) => x.start && (x.kind === 'call' || (x.kind === 'meeting' && yes(x))) && t(x.end || x.start) > now)
  for (const x of day || []) {
    if (x.kind !== 'meeting' || x.response !== 'notResponded' || !x.start || t(x.end || x.start) <= now) continue
    const at = going.filter((g) => t(g.start) < t(x.end || x.start) && t(x.start) < t(g.end || g.start)).sort((a, b) => t(a.start) - t(b.start))
    // Where you go then: the one Echo suggests keeping, when it is among them, else the first.
    const overlaps = out.filter((c) => c.kind === 'overlap')
    const picked = at.find((g) => overlaps.some((c) => c.pick === g))
    // At a time you go elsewhere: that meeting — the one Echo suggests keeping, or your only one; a choice not made
    // yet says only that you accepted others then.
    const open = !picked && at.find((g) => overlaps.some((c) => !c.pick && c.members.includes(g)))
    const why = picked ? `you go to ${picked.subject || 'another meeting'} then`
      : open ? `you accepted ${overlaps.find((c) => c.members.includes(open)).members.length} others then`
      : at.length ? `you go to ${at[0].subject || 'another meeting'} then` : 'your day is free then'
    out.push({ kind: 'invite', item: x, suggest: at.length ? 'decline' : 'accept', why, at: t(x.start) })
  }
  return out.sort((a, b) => a.at - b.at)
}
// end of the day choices block

/** The widget: a line a choice; nothing to decide, nothing drawn (temporal). */
function dayChoicesView(d, now) {
  // Its reasons: the threads where a promise of yours is due today or late (My work), and the clients' names.
  const mine = (((d.work || {}).mine || {}).items || []).filter((t) => t.late || (t.due && new Date(t.due + 'T12:00').toDateString() === now.toDateString()))
  const clients = Object.fromEntries(((S.pj.data && S.pj.data.clients) || []).filter((c) => c.account !== false).map((c) => [c.id, c.name]))
  const cs = dayChoices(d.day, now, { due: new Set(mine.map((t) => t.project)), clients })
  if (!cs.length) return ''
  const busy = S.tdResp || {}
  const btn = (x, how, label, primary) => {
    const sending = busy[x.event] === how
    return `<button class="btn sm${primary ? ' primary' : ''}" data-td-respond="${esc(how)}" data-event="${esc(x.event)}" ${busy[x.event] ? 'disabled' : ''}>${sending ? 'Sending…' : esc(label)}</button>`
  }
  const lines = cs.map((c) => {
    if (c.kind === 'invite') {
      const x = c.item
      return `<div class="ch-row" role="listitem" data-td-ev="${esc(x.event || '')}" aria-label="${esc(`Invite: ${x.subject || 'A meeting'}`)}"><span class="ag-t mono">${esc(hm(c.at))}</span><div class="ch-b"><div><b>${esc(x.subject || 'A meeting')}</b> <span class="set-sub">not answered</span></div><div class="set-sub">Echo suggests: <b>${c.suggest === 'accept' ? 'Accept' : 'Decline'}</b> — ${esc(c.why)}</div><div class="ch-acts">${btn(x, 'accept', 'Accept', c.suggest === 'accept')}${btn(x, 'tentative', 'Maybe')}${btn(x, 'decline', 'Decline', c.suggest === 'decline')}</div></div></div>`
    }
    const key = c.members.map((x) => x.event).join('|')
    // Two of the same name at once: told apart by their order ("Vendor demo (2)").
    const name = (x) => {
      const same = c.members.filter((m) => m.subject === x.subject)
      return (x.subject || 'A meeting') + (same.length > 1 ? ` (${same.indexOf(x) + 1})` : '')
    }
    const confirm = S.tdKeep && S.tdKeep.key === key ? c.members.find((x) => x.event === S.tdKeep.event) : null
    // Keep declines the others — not one you organise (Graph can't decline it: cancel it in Outlook).
    const others = (keep) => c.members.filter((x) => x !== keep && x.response !== 'organizer')
    const yours = (keep) => c.members.filter((x) => x !== keep && x.response === 'organizer')
    const acts = confirm
      ? `<span class="set-sub">${others(confirm).length ? `Decline ${esc(others(confirm).map(name).join(', '))}? The organiser${others(confirm).length === 1 ? ' is' : 's are'} told.` : 'Nothing to decline.'}${yours(confirm).length ? ` Yours — ${esc(yours(confirm).map(name).join(', '))} — stays: cancel it in Outlook.` : ''}</span>${others(confirm).length ? '<button' : '<span hidden'} class="btn sm primary" data-td-keep-yes="1" data-keep="${esc(confirm.event)}" data-decline="${esc(others(confirm).map((x) => x.event).join('|'))}">Decline ${others(confirm).length === 1 ? 'it' : 'them'}${others(confirm).length ? '</button>' : '</span>'}<button class="btn sm ghost" data-td-keep-no="1">Cancel</button>`
      : c.members.map((x) => `<button class="btn sm${x === c.pick ? ' primary' : ''}" data-td-keep="${esc(x.event)}" data-key="${esc(key)}" ${c.members.some((m) => busy[m.event]) ? 'disabled' : ''}>Keep ${esc(name(x))}</button>`).join('')
    return `<div class="ch-row" role="listitem" aria-label="${esc(`At once: ${c.members.map(name).join(' or ')}`)}"><span class="ag-t mono choice">${esc(hm(c.at))}</span><div class="ch-b"><div>${c.members.map((x) => `<b class="ch-m" data-td-ev="${esc(x.event || '')}">${esc(name(x))}</b>`).join(' <span class="set-sub">or</span> ')}</div><div class="set-sub">You accepted ${c.members.length} at once. ${c.pick ? `Echo suggests <b>${esc(name(c.pick))}</b> — ${esc(c.why)}` : 'Echo has no reason to pick one — your call.'}</div><div class="ch-acts">${acts}</div></div></div>`
  }).join('')
  return `<section class="td-sec ch-sec" aria-label="The day's choices"><h2>The day's choices <span class="set-sub">${cs.length}</span></h2><div class="ch-list" role="list">${lines}</div></section>`
}

/** Sends one answer; the reply refreshes Today (the agenda says the new answer). */
function todayRespond(event, how) {
  S.tdResp = { ...(S.tdResp || {}), [event]: how }
  render()
  send({ cmd: 'event_respond', event, how })
  // The buttons stay off until Today is read again after the reply (its refresh): sent, the line has gone; refused, the
  // notice said why and they come back.
  const was = S.today
  const back = () => { if (S.today === was) return setTimeout(back, 500); if (S.tdResp) { delete S.tdResp[event]; render() } }
  setTimeout(back, 500)
}

document.addEventListener('click', (e) => {
  if (S.view !== 'today' || !e.target.closest) return
  const r = e.target.closest('[data-td-respond]')
  if (r) { e.stopPropagation(); return todayRespond(r.dataset.event, r.dataset.tdRespond) }
  const k = e.target.closest('[data-td-keep]')
  // The keyboard stays in the line: on "Decline them" (or Cancel) after Keep, back on that Keep after Cancel.
  const refocus = (sel) => setTimeout(() => { const el = document.querySelector(sel); if (el) el.focus() })
  if (k) { e.stopPropagation(); S.tdKeep = { key: k.dataset.key, event: k.dataset.tdKeep }; render(); return refocus('button[data-td-keep-yes], [data-td-keep-no]') }
  if (e.target.closest('[data-td-keep-no]')) {
    e.stopPropagation()
    const was = S.tdKeep
    S.tdKeep = null
    render()
    return was && refocus(`[data-td-keep="${CSS.escape(was.event)}"][data-key="${CSS.escape(was.key)}"]`)
  }
  const y = e.target.closest('[data-td-keep-yes]')
  if (y) {
    e.stopPropagation()
    S.tdKeep = null
    for (const ev of y.dataset.decline.split('|').filter(Boolean)) todayRespond(ev, 'decline')
  }
}, true)
