// ── Dashboards (#533; Workbench `Dashboards`): a thread's Brief and a client's Overview each have a band of widgets
// over what is there — the widgets that can show one thread (one client), the page setting which. Empty until the user
// adds one; edited as Today is (62-today-edit.js: ✎, Add a widget, sizes, moves, ⚙ without the page's own settings);
// each page's layout its own (`today_layout::Page`, `echoctl today --thread / --client`).

/** The dashboard shown: 'thread:<id>' on a thread's Brief, 'client:<id>' on a client's Overview, else null (another
 *  tab: no dashboard, and edit mode waits there). */
function dashPageKey() {
  if (S.view !== 'projects' || S.pj.found) return null
  if (S.pj.client) return (S.pj.ctab || 'overview') === 'overview' ? `client:${S.pj.client}` : null
  return S.pj.sel && (!S.pj.tab || S.pj.tab === 'brief') ? `thread:${S.pj.sel}` : null
}

/** The page's dashboard: its layout, the widgets it offers, its widgets' data (editing: every offered one's, previewed). */
async function loadDashboard() {
  const page = dashPageKey()
  if (!page) return
  const all = S.dashEdit === page
  try {
    const d = await get(`api/dashboard?page=${encodeURIComponent(page)}${all ? '&all=1' : ''}`)
    if (dashPageKey() === page && !d.error) S.dash = d
  } catch { /* app restarting */ }
}

/** The band on top of a thread's Brief or a client's Overview: its widgets in their rows (S 1, M 2, L 3 of 3 columns);
 *  ✎ edits it. Nothing placed: only ✎, saying what it adds. */
function dashboardView() {
  const page = dashPageKey(), d = S.dash
  if (!page || !d || d.page !== page) return ''
  const editing = S.dashEdit === page
  const size = Object.fromEntries(d.layout.map((i) => [i.widget, i.size]))
  const parts = {}
  for (const [id, w] of Object.entries(d.plugins || {})) parts[id] = pluginWidgetView(id, w, size[id] || (typeof twPickSize === 'function' && twPickSize((S.tdCatalog || []).find((x) => x.id === id))) || 'M')
  // Placed but not answered with anything yet: its place kept while editing (to move it, or take it off).
  for (const i of d.layout) if (!(i.widget in parts)) parts[i.widget] = ''
  S.dashParts = parts
  const whose = page.startsWith('client:') ? 'client' : 'thread'
  const SPAN = { S: 1, M: 2, L: 3 }
  const rows = todayRows(d.layout, Object.keys(parts)).map((r) => r.filter((id) => editing || parts[id])).filter((r) => r.length)
  const line = (r) => {
    const used = r.reduce((a, id) => a + (SPAN[size[id]] || 2), 0), spare = Math.max(0, 3 - used)
    const cells = r.map((id) => (editing ? todayEditCell(id, parts[id], size[id]) : parts[id])).join('')
    return `<div class="td-grid td-hug" style="grid-template-columns: ${[...r.map((id) => `minmax(0, ${SPAN[size[id]] || 2}fr)`), ...(spare ? [`minmax(0, ${spare}fr)`] : [])].join(' ')}">${cells}${spare ? '<div aria-hidden="true"></div>' : ''}</div>`
  }
  const acts = editing
    ? '<span class="td-edit-acts"><button class="btn" data-tw-panel="1">Add a widget</button><button class="btn primary" data-tw-done="1">Done</button></span>'
    : `<button class="icon-btn td-edit-btn" data-tw-edit="${esc(page)}" aria-label="Edit this ${whose}'s dashboard" title="${rows.length ? `Edit this ${whose}'s dashboard — move, size, add widgets` : `Add widgets that show this ${whose}`}">${ICON('<path d="M4 20h4L19 9l-4-4L4 16z"/><path d="M14 6l4 4"/>')}</button>`
  if (editing) setTimeout(twRefocus) // a widget moved by the keyboard keeps it (as on Today)
  const empty = editing && !rows.length ? `<div class="tw-end dash-none" data-tw-end="1">Nothing here yet: Add a widget puts one that shows this ${whose}.</div>` : ''
  return `<section class="dash${editing ? ' td-editing' : ''}${rows.length || editing ? '' : ' bare'}" aria-label="Dashboard"><div class="dash-h">${acts}</div>${rows.map(line).join('')}${empty}${editing && rows.length ? todayEditEnd() : ''}</section>`
}
