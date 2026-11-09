// ── calendar: today, or a rolling week (yesterday, today, 5 days ahead) ───────────────────
// Days and hours by the local calendar and clock, never by adding 24 h: a day the clocks change
// isn't 24 hours long (a week across one showed a day twice and drew the later days' events at
// midnight).
const ymd = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
const midnight = (d) => { const x = new Date(d); x.setHours(0, 0, 0, 0); return x }
const addDays = (d, n) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n)
const sameDay = (a, b) => midnight(a).getTime() === midnight(b).getTime()
/** Where time `t` (ms) is on `day`'s clock, in hours: 0 before the day, 24 after it. */
const clockHours = (t, day) => {
  const d = new Date(t)
  return sameDay(d, day) ? d.getHours() + d.getMinutes() / 60 + d.getSeconds() / 3600 : t < day.getTime() ? 0 : 24
}

/** Side-by-side lanes for overlapping blocks in one day column, by their drawn hours `y0`..`y1`. */
function lanes(items) {
  items.sort((a, b) => a.y0 - b.y0 || b.y1 - a.y1)
  const cols = []
  let group = [], groupEnd = 0
  const flush = () => { const n = Math.max(1, ...group.map((x) => x.lane + 1)); group.forEach((x) => (x.lanes = n)); group = [] }
  for (const it of items) {
    if (group.length && it.y0 >= groupEnd) { flush(); cols.length = 0 }
    let lane = cols.findIndex((end) => end <= it.y0)
    if (lane < 0) { lane = cols.length; cols.push(0) }
    cols[lane] = it.y1
    it.lane = lane
    group.push(it)
    groupEnd = Math.max(groupEnd, it.y1)
  }
  if (group.length) flush()
  return items
}
/** One day column's blocks (`s`, `real`: start and end, ms) where they are drawn: `y0`..`y1` in
 *  clock hours on `day`, at least 20 min so a short meeting is still a readable block, in lanes by
 *  those hours (the hour a day the clocks go back repeats would put two meetings an hour apart in
 *  one place). Counted in whole seconds, so a block ending when the next starts never overlaps it. */
const layoutDay = (items, day) => lanes(items.map((x) => {
  const sec = (t) => Math.round(clockHours(t, day) * 3600)
  const s0 = sec(x.s)
  return Object.assign(x, { y0: s0 / 3600, y1: Math.max(sec(x.real), s0 + 1200) / 3600 })
}))
// end of the calendar's helpers (calendar-dates.test.mjs runs app.js from `const ymd` to here)
function calRange() {
  const today = midnight(new Date())
  if (S.cal.mode === 'day') return { start: addDays(today, S.cal.offset), days: 1 }
  return { start: addDays(today, S.cal.offset * 7 - 1), days: 7 }
}
async function loadCalendar(fresh) {
  const { start, days } = calRange()
  S.cal.loading = true
  try {
    S.cal.data = await get(`api/calendar?start=${ymd(start)}&days=${days}${fresh ? '&fresh=1' : ''}`)
  } catch (e) { S.cal.data = { error: String(e), events: [], calls: [] } }
  S.cal.loading = false
}

function eventState(e, now) {
  const s = new Date(e.start), en = new Date(e.end)
  const past = en < now, live = s <= now && en > now
  const recordable = e.online && !e.declined && !e.cancelled && !e.no_notetakers && !past
  return { s, en, past, live, recordable }
}

function calendarView() {
  const d = S.cal.data
  const { start, days } = calRange()
  const now = new Date()
  const dayList = Array.from({ length: days }, (_, i) => addDays(start, i))
  const title = days === 1
    ? start.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })
    : `${dayList[0].toLocaleDateString([], { day: 'numeric', month: 'short' })} to ${dayList[days - 1].toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' })}`
  const head = `<div class="cal-head"><h1>${esc(title)}</h1>
    <div class="seg" role="group" aria-label="View"><button data-calmode="day" aria-pressed="${S.cal.mode === 'day'}">Day</button><button data-calmode="week" aria-pressed="${S.cal.mode === 'week'}">Week</button></div>
    <button class="btn ghost" data-calnav="-1" aria-label="Earlier">‹</button><button class="btn" data-calnav="0">Today</button><button class="btn ghost" data-calnav="1" aria-label="Later">›</button></div>`
  let note = ''
  if (d && d.error) note = `<div class="cal-note error">Can't read the calendar: ${esc(d.error)}</div>`
  else if (d && !d.signed_in) note = `<div class="cal-note">Showing today's meetings only. Connect Microsoft 365 from the menu-bar octopus (Calendar) to see your week.</div>`
  if (!d) return `<div class="cal">${head}${note}<div style="padding:20px"><div class="skeleton">${'<i></i>'.repeat(8)}</div></div></div>`

  // Blocks per day.
  const blocks = dayList.map(() => [])
  const allday = dayList.map(() => [])
  for (const e of d.events || []) {
    const st = eventState(e, now)
    const di = dayList.findIndex((x) => sameDay(x, st.s))
    if (di < 0) continue
    if (e.all_day) { allday[di].push(e); continue }
    blocks[di].push({ kind: 'event', ev: e, st, s: st.s.getTime(), real: st.en.getTime() })
  }
  for (const c of d.calls || []) {
    const s = new Date(c.start), en = c.end ? new Date(c.end) : new Date(s.getTime() + 30 * 60000)
    const di = dayList.findIndex((x) => sameDay(x, s))
    if (di >= 0) blocks[di].push({ kind: 'call', c, s: s.getTime(), real: en.getTime() })
  }
  blocks.forEach((b, di) => layoutDay(b, dayList[di]))

  // The whole day, scrolling; an hour is sized so the working day (08:00–19:00) fills the
  // window (min 40 px).
  const h0 = 0, h1 = 24
  const reserved = 112 + (allday.some((a) => a.length) ? 30 : 0) + (note ? 34 : 0)
  const HOUR = Math.max(40, Math.floor((window.innerHeight - reserved) / 11))
  const cols = `grid-template-columns: repeat(${days}, minmax(0, 1fr))`
  const dayHead = `<div class="days" style="${cols}">${dayList.map((x) => `<div class="${sameDay(x, now) ? 'today' : ''}">${esc(x.toLocaleDateString([], { weekday: 'short' }))}<b>${x.getDate()}</b></div>`).join('')}</div>`
  const alldayRow = allday.some((a) => a.length) ? `<div class="allday" style="${cols}">${allday.map((a) => `<div>${a.map((e) => `<span title="${esc(e.subject)}">${esc(e.subject)}</span>`).join('')}</div>`).join('')}</div>` : ''
  const hourLabels = Array.from({ length: h1 - h0 }, (_, i) => `<span style="top:${i * HOUR}px">${i === 0 ? '' : String(h0 + i).padStart(2, '0') + ':00'}</span>`).join('')
  const top = (hours) => (hours - h0) * HOUR
  const colHtml = dayList.map((day, di) => {
    const lines = Array.from({ length: h1 - h0 }, (_, i) => `<div class="hourline" style="top:${i * HOUR}px"></div>`).join('')
    const nowLine = sameDay(day, now) ? `<div class="now" style="top:${top(clockHours(now.getTime(), day))}px"></div>` : ''
    const items = blocks[di].map((x) => {
      const y = top(x.y0), h = Math.max(22, top(x.y1) - y - 2)
      const w = 100 / x.lanes, left = x.lane * w
      const style = `top:${y}px;height:${h}px;left:calc(${left}% + 2px);width:calc(${w}% - 4px)`
      if (x.kind === 'call') {
        return `<button class="ev call" style="${style}" data-call="${esc(x.c.name)}"><b>${esc(x.c.title)}</b><small>${esc(hm(new Date(x.s)))}</small></button>`
      }
      const { ev: e, st } = x
      const cls = ['ev', e.online ? 'online' : '', e.recorded ? 'recorded' : '', e.scheduled ? 'scheduled' : '', st.past ? 'past' : '', !e.recorded && e.transcript ? 'transcript' : '', e.declined ? 'declined' : '', e.cancelled ? 'cancelled' : ''].filter(Boolean).join(' ')
      // Green already says recorded: only what the colour doesn't.
      const tag = [appTag(e), e.recorded ? '' : e.transcript ? '<span class="tag tr" title="Teams has its transcript: click, then Import">Transcript</span>' : e.scheduled ? '<span class="tag will">Will record</span>' : e.no_notetakers ? '<span class="tag no">No notetakers</span>' : ''].filter(Boolean).join(' ')
      return `<button class="${cls}" style="${style}" data-ev="${esc(e.id || '')}" aria-selected="${S.cal.selected === e.id}"><b>${esc(e.subject)}</b><small>${esc(hm(st.s))}${tag ? ' ' + tag : ''}</small></button>`
    }).join('')
    return `<div class="col${sameDay(day, now) ? ' today' : ''}" style="height:${(h1 - h0) * HOUR}px">${lines}${nowLine}${items}</div>`
  }).join('')
  const empty = !blocks.some((b) => b.length) && !allday.some((a) => a.length)
    ? `<p style="position:absolute;top:40%;left:0;right:0;text-align:center;color:var(--text-3);pointer-events:none">${days === 1 ? 'Nothing on the calendar this day.' : 'Nothing on the calendar these days.'}</p>` : ''
  const grid = `<div class="grid-wrap" id="calgrid" data-hour="${HOUR}"><div class="grid" style="grid-template-columns: 56px repeat(${days}, minmax(0, 1fr))"><div class="hours" style="height:${(h1 - h0) * HOUR}px">${hourLabels}</div>${colHtml}</div>${empty}</div>`
  const sel = S.cal.selected && (d.events || []).find((e) => e.id === S.cal.selected)
  return `<div class="cal">${head}${note}<div style="display:grid;grid-template-rows:auto auto 1fr;min-height:0">${dayHead}${alldayRow}${grid}</div>${sel ? eventDetail(sel, now) : ''}${eventMenu(d, now)}</div>`
}

/** A calendar event's card: its head (title, × close, when and how far off, its kind and threads as chips), one action
 *  strip (the main button for its state, its prep beside it, a line saying what happens), then People, the series rule
 *  and Last time under small headings — each only when there is something to show. */
function eventDetail(e, now) {
  const st = eventState(e, now)
  const kind = eventAction(e, st)
  const ACTS = {
    'open-notes': () => `<button class="btn primary" data-call="${esc(e.recorded.name)}">Open notes</button>`,
    'other-app': () => '',
    scheduled: () => `<button class="btn" data-unschedule="${esc(e.id)}">Don't record</button>`,
    join: () => `${recordButton(`data-join="${esc(e.id)}"`, 'Join & record')}<button class="btn" data-schedule="${esc(e.id)}" title="Record without joining — you're in it already">Record</button>`,
    record: () => recordButton(`data-schedule="${esc(e.id)}"`, st.live ? 'Record it' : 'Record when it starts'),
    import: () => e.import && e.import.busy ? '<button class="btn" disabled>Importing the Teams transcript…</button>'
      : `<button class="btn primary" data-cal-import="${esc(e.id)}">Import Teams transcript</button>`,
  }
  const prep = eventPrep(e, st)
  const slots = slotItems('calendar.event', { kind: 'event', event: e })
  const acts = `${kind ? ACTS[kind]() : ''}${prep.btn}${slots}`
  // One line: what happens, or why nothing can.
  const rec = e.recorded
  const line = rec ? `${rec.imported ? 'A call made of its Teams transcript' : STATUS[rec.status] || esc(rec.status)}${rec.minutes ? `, ${rec.minutes} min` : ''}`
    : e.import && e.import.error ? esc(e.import.error)
    : kind === 'import' ? `${e.transcript ? 'Not recorded, but Teams transcribed it' : e.transcript === false ? 'Not recorded, and Teams has no transcript of it (yet)' : 'Not recorded. If Teams transcribed it'}: Echo makes a call of its transcript and chat; minutes, threads and the Brain follow.`
    : kind === 'other-app' ? notYet(e)
    : kind === 'scheduled' ? `Echo starts recording a minute before it begins${e.scheduled_languages ? `, transcribed as ${esc(e.scheduled_languages.map(langName).join(', '))}` : ''}.`
    : kind === 'join' ? 'Joins it in Teams and records both sides.'
    : kind === 'record' ? (st.live ? 'Records it now, both sides.' : 'Echo starts recording a minute before it begins.')
    : e.no_notetakers ? 'The organizer asked for no notetakers: Echo won\'t record it.' : e.declined ? 'You declined this meeting.' : e.cancelled ? 'This meeting was cancelled.'
    : !e.online ? 'Not a Teams meeting: nothing to record.' : st.past ? 'This meeting is over and was not recorded.' : ''
  const lines = [line, prep.line].filter(Boolean).join(' ')
  const main = acts || lines ? `<div class="ec-main">${acts ? `<div class="ec-acts">${acts}</div>` : ''}${lines ? `<p class="why">${lines}</p>` : ''}</div>` : ''
  // Who's coming: the organizer first, then three names and how many more.
  // The names come at most 50: how many more is counted from the invite's own number.
  const names = e.names || [], others = names.filter((n) => n !== e.organizer)
  const total = Math.max(others.length, (e.people || 0) - (names.includes(e.organizer) ? 1 : 0)), more = total - Math.min(3, others.length)
  const invited = others.length ? `<div class="ec-names" title="${esc(others.join(', '))}">${esc(others.slice(0, 3).join(', '))}${more > 0 ? ` <span class="set-sub">+${more} more</span>` : ''}</div>`
    : total > 0 ? `<div class="ec-names set-sub">${total} invited</div>` : ''
  const people = e.organizer || invited ? `<section class="ec-sec"><div class="ec-cap">People</div>
      ${e.organizer ? `<div class="ec-org"><span class="ec-av" aria-hidden="true">${esc([...e.organizer][0] || '')}</span><b>${esc(e.organizer)}</b><span class="set-sub">organizer</span></div>` : ''}
      ${invited}</section>` : ''
  const series = seriesEditor(e)
  const recap = e.recap && e.recap.length ? `<section class="ec-sec"><div class="ec-cap">Last time</div><ul>${e.recap.map((r) => `<li>${esc(r)}</li>`).join('')}</ul></section>` : ''
  const where = e.location ? `<span class="ec-where">${esc(e.location)}</span>` : ''
  const chips = `${appTag(e)}${eventTypeChip(e)}${eventProjects(e)}`
  return `<aside class="detail" aria-labelledby="ec-h">
    <div class="ec-head"><div class="ec-title"><h2 id="ec-h" class="selectable">${esc(e.subject)}</h2><button class="icon-btn ec-x" data-calclose="1" aria-label="Close" title="Close">${ICON('<path d="M6 6l12 12M18 6L6 18"/>')}</button></div>
      <div class="when">${esc(evDay(st.s, now))} · ${esc(hm(st.s))}–${esc(hm(st.en))}${evRel(e, st, now)}${where}</div>
      ${chips.trim() ? `<div class="ec-chips">${chips}</div>` : ''}</div>
    ${main}${people}${series ? `<section class="ec-sec">${series}</section>` : ''}${recap}
  </aside>`
}
/** The app of a meeting Echo can't record yet (Zoom, Google Meet), as a small label; nothing for Teams. */
function appTag(e) {
  return otherApp(e) ? `<span class="tag app">${esc(meetingApp(e).name)}</span>` : ''
}
/** Why a Zoom or Google Meet meeting has no Record (#228). */
function notYet(e) {
  return `Recording isn't available for ${esc(meetingApp(e).name)} meetings yet.`
}
/** A day as the card says it: Today, Tomorrow, Yesterday, else its weekday and date. */
function evDay(d, now) {
  const a = new Date(d), b = new Date(now); a.setHours(0, 0, 0, 0); b.setHours(0, 0, 0, 0)
  const n = Math.round((a - b) / 864e5)
  return n === 0 ? 'Today' : n === 1 ? 'Tomorrow' : n === -1 ? 'Yesterday' : d.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })
}
/** How far off it is, coloured by state: in 12 min / now · 8 min left / recorded / over. */
function evRel(e, st, now) {
  const span = (cls, t) => ` <span class="ec-rel ${cls}">· ${esc(t)}</span>`
  if (e.recorded) return span('done', 'recorded')
  if (e.cancelled) return span('muted', 'cancelled')
  if (st.live) return span('live', `now · ${Math.max(1, Math.round((st.en - now) / 60000))} min left`)
  if (st.past) return span('muted', 'over')
  const m = Math.round((st.s - now) / 60000)
  return span('soon', m < 60 ? `in ${Math.max(1, m)} min` : m < 24 * 60 ? `in ${Math.round(m / 60)} h` : `in ${Math.round(m / 1440)} day${Math.round(m / 1440) === 1 ? '' : 's'}`)
}

/** A calendar event's projects on its card: chips (× on one added by hand to this event), and
 *  + Project — add it to a project (its call will belong), always its meeting series, or a new
 *  project from it. A recorded meeting's projects are its call's. */
function eventProjects(e) {
  if (!e.id) return ''
  const mine = e.projects || []
  const ids = new Set(mine.map((p) => p.id))
  const WHYS = { series: 'its meeting series', people: "the client's people", chat: 'its Teams chat', subject: 'its subject', pinned: 'added by hand' }
  const chips = mine.map((p) => `<span class="pj-chip" title="${esc(WHYS[p.why] || p.why)}">${projIcon(p, 14)}<button class="link-btn" data-pj="goto" data-id="${esc(p.id)}">${esc(p.name)}</button>${p.why === 'pinned' && !e.recorded ? `<button class="pj-x" data-pj="event-member" data-id="${esc(p.id)}" data-event="${esc(e.id)}" data-how="remove" aria-label="Take it out of ${esc(p.name)}">×</button>` : ''}</span>`).join('')
  const all = ((S.pj.data && S.pj.data.projects) || []).filter((p) => p.group !== 'closed' && !ids.has(p.id))
  const picker = S.pj.evPicker === e.id ? `<div class="menu pj-menu" role="menu">${all.map((p) => `<button class="menu-item" role="menuitem" data-pj="event-member" data-id="${esc(p.id)}" data-event="${esc(e.id)}" data-how="include">${projIcon(p, 14)} ${esc(p.name)}</button>`).join('') || '<div class="menu-item set-sub">No other threads</div>'}<div class="menu-sep" role="separator"></div>${all.length ? `<label class="menu-item pj-series" title="Every occurrence of this meeting joins the thread">Every occurrence → <select data-ev-series="${esc(e.subject || '')}"><option value=""></option>${all.concat(mine.map((m) => ((S.pj.data && S.pj.data.projects) || []).find((x) => x.id === m.id)).filter(Boolean)).map((p) => `<option value="${esc(p.id)}">${esc(p.name)}</option>`).join('')}</select></label>` : ''}<button class="menu-item" role="menuitem" data-pj="event-new" data-event="${esc(e.id)}" data-subject="${esc(e.subject || '')}">New thread from this meeting…</button></div>` : ''
  if (e.recorded && !mine.length) return ''
  return `<div class="pj-chips ev-pj">${chips}${e.recorded ? '' : `<span class="pj-add-wrap"><button class="pj-chip add" data-pj="ev-picker" data-event="${esc(e.id)}" aria-haspopup="menu" aria-expanded="${S.pj.evPicker === e.id}">+ Thread</button>${picker}</span>`}</div>`
}


/** Right-click a meeting in the calendar: what its card offers, in a menu at the pointer — its main action, its prep,
 *  a thread, the series rule, its Teams link, and the card itself. Each item is the card's own button (same handlers). */
function eventMenu(d, now) {
  const m = S.cal.ctx
  const e = m && ((d && d.events) || []).find((x) => x.id === m.id)
  if (!e) return ''
  const st = eventState(e, now), kind = eventAction(e, st)
  const item = (attrs, label, extra = '') => `<button class="menu-item" role="menuitem" ${attrs}${extra}>${esc(label)}</button>`
  const sep = '<div class="menu-sep" role="separator"></div>'
  const main = {
    'open-notes': () => item(`data-call="${esc(e.recorded.name)}"`, 'Open notes'),
    'other-app': () => '',
    scheduled: () => item(`data-unschedule="${esc(e.id)}"`, "Don't record"),
    join: () => item(`data-join="${esc(e.id)}"`, 'Join & record') + item(`data-schedule="${esc(e.id)}"`, 'Record (without joining)'),
    record: () => item(`data-schedule="${esc(e.id)}"`, st.live ? 'Record it' : 'Record when it starts'),
    import: () => item(`data-cal-import="${esc(e.id)}"`, 'Import Teams transcript', e.import && e.import.busy ? ' disabled' : ''),
  }
  const p = e.prep
  const prep = !p || p.writing ? '' : p.ready ? item(`data-ev-prep="${esc(p.id)}"`, 'Show prep') + (st.past ? '' : item(`data-prep-write="${esc(p.id)}" data-prep-open="1"`, 'Prepare again'))
    : st.past ? '' : item(`data-prep-write="${esc(p.id)}" data-prep-open="1"`, 'Prepare')
  const thread = e.recorded ? '' : item(`data-ev-ctx="thread" data-id="${esc(e.id)}"`, 'Add to a thread…')
  const x = e.series
  const rec = x && x.key && x.rule && x.rule.record === 'always'
  // A Zoom or Google Meet meeting: Echo can't record its series either (#228).
  const series = x && x.key && !otherApp(e) ? `<div data-series-key="${esc(x.key)}" data-series-subject="${esc(e.subject || '')}" style="display:contents">${item(`data-series-set="record" data-val="${esc(JSON.stringify(rec ? null : 'always'))}"`, rec ? "Stop recording every meeting of this series" : 'Record every meeting of this series')}</div>` : ''
  const link = e.join_url ? item(`data-copy="${esc(e.join_url)}"`, `Copy the ${otherApp(e) ? meetingApp(e).name : 'Teams'} link`) : ''
  const groups = [(kind ? main[kind]() : '') + prep, thread + series, link, item(`data-ev-ctx="open" data-id="${esc(e.id)}"`, 'Show details')].filter(Boolean)
  // At the pointer, kept inside the window (its size is about known: 240 px wide, 32 px an item).
  const n = (groups.join('').match(/menu-item/g) || []).length
  const x0 = Math.min(m.x, window.innerWidth - 252), y0 = Math.min(m.y, window.innerHeight - (n * 32 + groups.length * 9 + 40))
  return `<div class="menu ev-ctx" role="menu" aria-label="${esc(e.subject)}" style="left:${Math.max(8, x0)}px;top:${Math.max(8, y0)}px"><div class="menu-label">${esc(e.subject)}</div>${groups.join(sep)}</div>`
}
document.addEventListener('contextmenu', (ev) => {
  const b = ev.target.closest && ev.target.closest('.cal .ev[data-ev]')
  if (!b || !b.dataset.ev) return
  ev.preventDefault()
  S.cal.ctx = { id: b.dataset.ev, x: ev.clientX, y: ev.clientY }
  render()
  const first = document.querySelector('.ev-ctx .menu-item:not([disabled])')
  if (first) first.focus()
})
// Any click closes the menu (the item's own handler still sees its button); Escape too.
document.addEventListener('click', (ev) => {
  if (!S.cal.ctx) return
  const inMenu = ev.target.closest && ev.target.closest('.ev-ctx')
  const c = inMenu && ev.target.closest('[data-ev-ctx]')
  if (c) {
    S.cal.selected = c.dataset.id
    if (c.dataset.evCtx === 'thread') S.pj.evPicker = c.dataset.id
  }
  // The copy button is redrawn away: the notice says it was copied.
  if (inMenu && ev.target.closest('[data-copy]')) S.notice = 'The Teams link is copied.'
  closeEventMenu(!inMenu)
})
/** The menu goes; its meeting's block gets the focus back (unless something else was clicked). */
function closeEventMenu(refocus) {
  const id = S.cal.ctx && S.cal.ctx.id
  S.cal.ctx = null
  render()
  if (refocus && id) { const b = document.querySelector(`.cal .ev[data-ev="${CSS.escape(id)}"]`); if (b) b.focus() }
}
// Keys in the menu: arrows, Home and End move between its items; Escape closes it.
document.addEventListener('keydown', (ev) => {
  if (!S.cal.ctx) return
  if (ev.key === 'Escape') { ev.preventDefault(); ev.stopImmediatePropagation(); return closeEventMenu(true) }
  const items = [...document.querySelectorAll('.ev-ctx .menu-item:not([disabled])')]
  if (!items.length || !['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(ev.key)) return
  ev.preventDefault()
  const i = items.indexOf(document.activeElement)
  const n = ev.key === 'Home' ? 0 : ev.key === 'End' ? items.length - 1 : ev.key === 'ArrowDown' ? (i + 1) % items.length : (i - 1 + items.length) % items.length
  items[n].focus()
})
// The user scrolling (a wheel, not the grid's own scroll a render restores) or resizing the window closes it too.
for (const t of ['wheel', 'resize']) window.addEventListener(t, () => { if (S.cal.ctx) closeEventMenu(false) }, { passive: true })
