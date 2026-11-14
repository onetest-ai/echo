// ── Filters (`core:filters`): Echo's widget that gathers the settings of the widgets beside it — one control a setting
// they share (same key and type), set on all of them at once (`today_layout::set_filter_on`, the layout op `filter`,
// as `echoctl today filter`). Data: `today_layout::filters_on` (api/today's `filters`, a dashboard's core data).

const FILTER_ALL = { thread: 'All threads', client: 'Any client' }

/** One control a setting; Mixed while the widgets differ (choosing a value sets it on all of them). */
function filtersView(v, size) {
  if (!v) return ''
  const list = v.filters || []
  const control = (f) => {
    const fid = `tf-${`${f.key}-${f.type}`.replace(/[^a-z0-9-]/gi, '-')}`
    const opts = f.type === 'bool' ? [['true', 'On'], ['false', 'Off']] : (f.options || []).map((o) => [String(o.id), o.name])
    const now = f.mixed ? null : f.type === 'bool' ? String(f.value === true) : f.value == null ? null : String(f.value)
    const all = FILTER_ALL[f.type]
    const head = f.mixed ? '<option value="" selected disabled>Mixed</option>' : now === null && !all ? '<option value="" selected disabled>—</option>' : ''
    const n = (f.widgets || []).length
    const title = `${n} widget${n === 1 ? '' : 's'} here ${n === 1 ? 'has' : 'have'} it`
    return `<label class="tf-ctl" title="${esc(title)}"><span>${esc(f.label)}</span><select id="${fid}" data-tf-key="${esc(f.key)}">${head}${all ? `<option value=""${now === '' ? ' selected' : ''}>${all}</option>` : ''}${opts.map(([id, name]) => `<option value="${esc(id)}"${id === now ? ' selected' : ''}>${esc(name)}</option>`).join('')}</select></label>`
  }
  const body = list.length ? `<div class="tf-row${size === 'S' ? ' tf-stack' : ''}">${list.map(control).join('')}</div>` : '<p class="set-sub">No widget here has a setting to filter by.</p>'
  return `<section class="td-sec tf-sec" aria-label="Filters"><h2>Filters</h2>${body}</section>`
}

// A filter chosen: set on every widget of this page that has it (the page then reads its widgets again).
document.addEventListener('change', (e) => {
  const el = e.target
  if (!el || !el.dataset || el.dataset.tfKey === undefined) return
  const page = S.view === 'today' ? null : dashPageKey()
  if (S.view !== 'today' && !page) return
  send({ cmd: 'today_layout', op: 'filter', ...(page ? { page } : {}), key: el.dataset.tfKey, value: el.value })
})
// end of the filters block
