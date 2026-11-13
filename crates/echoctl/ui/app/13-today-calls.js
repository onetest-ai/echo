// ── Recorded this week (`core:calls.week`, #315): the calls recorded since Monday — memos counted, as the Calls list
// shows them — the time recorded and how many have minutes; at M a bar a day, Monday to Sunday, today marked. A click
// opens Calls. Data: `calls_week::view` (api/today's `calls_week`, read only when it is on Today).

/** The week's three numbers; at M the calls of each day as bars. No data (not on Today): nothing (the Add a widget
 *  panel then says what it is). */
function callsWeekView(w, size) {
  if (!w) return ''
  const n = (x) => Number(x) || 0
  const calls = n(w.calls), memos = n(w.memos), min = n(w.recorded_min), mins = n(w.with_minutes)
  // Minutes under an hour, else hours to one decimal.
  const h = Math.round(min / 6) / 10
  const time = min < 60 ? [min, `minute${min === 1 ? '' : 's'} recorded`] : [h, `hour${h === 1 ? '' : 's'} recorded`]
  const stat = (v, label) => `<div class="cw-stat"><b>${esc(String(v))}</b><span class="set-sub">${esc(label)}</span></div>`
  const callsLabel = `call${calls === 1 ? '' : 's'}${memos ? `, ${memos === calls ? (memos === 1 ? 'a memo' : 'all memos') : `${memos} of them memo${memos === 1 ? '' : 's'}`}` : ''}`
  const stats = `<div class="cw-stats">${stat(calls, callsLabel)}${stat(time[0], time[1])}${stat(mins, 'with minutes')}</div>`
  const none = calls ? '' : '<p class="set-sub cw-none">Nothing recorded this week.</p>'
  let bars = ''
  if (size === 'M') {
    const days = (w.days || []).slice(0, 7), max = Math.max(1, ...days.map((d) => n(d.calls)))
    const day = (d) => new Date(`${d.date}T12:00:00`)
    bars = `<div class="cw-days" role="list" aria-label="Calls a day, Monday to Sunday">${days.map((d) => {
      const c = n(d.calls), on = d.date === w.today
      const name = `${day(d).toLocaleDateString([], { weekday: 'long' })}: ${c} call${c === 1 ? '' : 's'}${on ? ' (today)' : ''}`
      return `<div class="cw-day${on ? ' on' : ''}" role="listitem" aria-label="${esc(name)}" title="${esc(name)}"><span class="cw-bar"><i style="height:${Math.round((c / max) * 100)}%"></i></span><span class="cw-d" aria-hidden="true">${esc(day(d).toLocaleDateString([], { weekday: 'short' }))}</span></div>`
    }).join('')}</div>`
  }
  return `<section class="td-sec cw-sec" aria-label="Recorded this week" data-go="calls" title="Open Calls"><h2>Recorded this week${w.scope ? ` · ${esc(w.scope)}` : ''} <button class="link-btn" data-go="calls">Calls</button></h2>${stats}${none}${bars}</section>`
}
