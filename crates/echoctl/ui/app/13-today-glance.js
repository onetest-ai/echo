// ── Threads at a glance and Done this week (`core:tasks.glance`, `core:tasks.done`, #315; design: Workbench
// `WidgetsTasks`): a traffic light a thread — done this week (green), to do (amber), waiting (red) — and its total; the
// tasks done since Monday. Data: `glance::view` (api/today's `glance`, read only when one of them is on Today).

/** A bar a thread, busiest first; a click on its name opens its Work. M five threads, L ten. */
function glanceView(g, size) {
  const threads = (g && g.threads) || []
  if (!threads.length) return ''
  const max = size === 'L' ? 10 : 5, shown = threads.slice(0, max)
  const when = g.period === 'month' ? 'this month' : 'this week'
  const seg = (n, cls, total, say) => (n ? `<span class="gl-${cls}" style="flex:${n}" title="${n} ${say}"></span>` : '')
  const rows = shown.map((t) => `<div class="gl-row" role="listitem" aria-label="${esc(`${t.name}: ${t.done} done ${when}, ${t.todo} to do, ${t.waiting} waiting`)}"><a class="link-btn gl-name" href="#project/${encodeURIComponent(t.id)}/work">${esc(t.name)}</a><span class="gl-bar">${seg(t.done, 'done', t.total, `done ${when}`)}${seg(t.todo, 'todo', t.total, 'to do')}${seg(t.waiting, 'wait', t.total, 'waiting')}</span><span class="gl-n mono">${t.total}</span></div>`).join('')
  const more = threads.length > max ? `<button class="link-btn gl-more" data-go="projects">${threads.length - max} more in Threads</button>` : ''
  return `<section class="td-sec gl-sec" aria-label="Threads at a glance"><h2>Threads at a glance</h2><div role="list">${rows}</div><div class="gl-foot"><span class="gl-key set-sub"><span><i class="gl-done"></i>done ${when}</span><span><i class="gl-todo"></i>to do</span><span><i class="gl-wait"></i>waiting</span></span>${more}</div></section>`
}

/** How many tasks were done since Monday; at M, the latest few, each opening its task. */
function doneView(g, size) {
  const done = (g && g.done) || [], month = g && g.period === 'month'
  const list = size === 'M' && done.length ? `<div class="dn-list" role="list">${done.slice(0, 4).map((t) => `<div class="dn-row" role="listitem"><button class="link-btn" data-tk="open" data-project="${esc(t.project)}" data-node="${esc(t.id)}">${esc(t.title)}</button><span class="set-sub mono">${esc(t.thread)}</span></div>`).join('')}</div>` : ''
  return `<section class="td-sec dn-sec" aria-label="Done this week"><h2>Done ${month ? 'this month' : 'this week'}</h2><div class="dn-n"><b>${done.length}</b><span class="set-sub">task${done.length === 1 ? '' : 's'} ${month ? 'this month' : 'since Monday'}</span></div>${list}</section>`
}
