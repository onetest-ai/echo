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
      const tag = e.recorded ? '' : e.transcript ? '<span class="tag tr" title="Teams has its transcript: click, then Import">Transcript</span>' : e.scheduled ? '<span class="tag will">Will record</span>' : e.no_notetakers ? '<span class="tag no">No notetakers</span>' : ''
      return `<button class="${cls}" style="${style}" data-ev="${esc(e.id || '')}" aria-selected="${S.cal.selected === e.id}"><b>${esc(e.subject)}</b><small>${esc(hm(st.s))}${tag ? ' ' + tag : ''}</small></button>`
    }).join('')
    return `<div class="col${sameDay(day, now) ? ' today' : ''}" style="height:${(h1 - h0) * HOUR}px">${lines}${nowLine}${items}</div>`
  }).join('')
  const empty = !blocks.some((b) => b.length) && !allday.some((a) => a.length)
    ? `<p style="position:absolute;top:40%;left:0;right:0;text-align:center;color:var(--text-3);pointer-events:none">${days === 1 ? 'Nothing on the calendar this day.' : 'Nothing on the calendar these days.'}</p>` : ''
  const grid = `<div class="grid-wrap" id="calgrid" data-hour="${HOUR}"><div class="grid" style="grid-template-columns: 56px repeat(${days}, minmax(0, 1fr))"><div class="hours" style="height:${(h1 - h0) * HOUR}px">${hourLabels}</div>${colHtml}</div>${empty}</div>`
  const sel = S.cal.selected && (d.events || []).find((e) => e.id === S.cal.selected)
  return `<div class="cal">${head}${note}<div style="display:grid;grid-template-rows:auto auto 1fr;min-height:0">${dayHead}${alldayRow}${grid}</div>${sel ? eventDetail(sel, now) : ''}</div>`
}

function eventDetail(e, now) {
  const st = eventState(e, now)
  const when = `${st.s.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })}, ${hm(st.s)} to ${hm(st.en)}`
  const ACTS = {
    'open-notes': () => `<button class="btn primary" data-call="${esc(e.recorded.name)}">Open notes</button>`,
    scheduled: () => `<button class="btn" data-unschedule="${esc(e.id)}">Don't record</button><p class="why">Echo arms itself a minute before it starts${e.scheduled_languages ? `, transcribed as ${esc(e.scheduled_languages.map(langName).join(', '))}` : ''}.</p>`,
    join: () => `${recordButton(`data-join="${esc(e.id)}"`, 'Join and record')}<button class="btn" data-schedule="${esc(e.id)}">Record it</button>`,
    record: () => recordButton(`data-schedule="${esc(e.id)}"`, st.live ? 'Record it' : 'Record when it starts'),
    import: () => e.import && e.import.busy ? '<button class="btn" disabled>Importing the Teams transcript…</button>'
      : `<button class="btn primary" data-cal-import="${esc(e.id)}">Import Teams transcript</button>`,
  }
  const kind = eventAction(e, st)
  const acts = kind ? ACTS[kind]() : ''
  const why = e.recorded ? '' : e.import && e.import.error ? e.import.error : kind === 'import' ? (e.transcript ? 'Not recorded, but Teams transcribed it' : e.transcript === false ? 'Not recorded, and Teams has no transcript of it (yet)' : 'Not recorded. If Teams transcribed it') + ' — Echo makes a call of its transcript and chat; minutes, threads and the Brain follow.' : e.no_notetakers ? 'The organizer asked for no notetakers.' : e.declined ? 'You declined this meeting.' : e.cancelled ? 'This meeting was cancelled.' : !e.online ? 'Not a Teams meeting.' : st.past ? 'This meeting is over and was not recorded.' : ''
  return `<aside class="detail">
    <button class="btn ghost close" data-calclose="1" aria-label="Close">Close</button>
    <h2 class="selectable">${esc(e.subject)}</h2><div class="when">${esc(when)}${eventTypeChip(e)}</div>
    <dl>${e.organizer ? `<dt>Organizer</dt><dd>${esc(e.organizer)}</dd>` : ''}${e.people ? `<dt>People</dt><dd>${e.people}</dd>` : ''}${e.location ? `<dt>Where</dt><dd>${esc(e.location)}</dd>` : ''}${e.recorded ? `<dt>${e.recorded.imported ? 'Call' : 'Recording'}</dt><dd>${e.recorded.imported ? 'from the Teams transcript' : STATUS[e.recorded.status] || esc(e.recorded.status)}${e.recorded.minutes ? `, ${e.recorded.minutes} min` : ''}</dd>` : ''}</dl>
    ${acts || slotItems('calendar.event', { kind: 'event', event: e }) ? `<div class="acts">${acts}${slotItems('calendar.event', { kind: 'event', event: e })}</div>` : ''}${why ? `<p class="why">${esc(why)}</p>` : ''}
    ${eventProjects(e)}
    ${eventPrep(e, st)}
    ${seriesEditor(e)}
    ${e.recap && e.recap.length ? `<div><b style="font-size:12px">Last time</b><ul>${e.recap.map((r) => `<li>${esc(r)}</li>`).join('')}</ul></div>` : ''}
  </aside>`
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

