// The test plugin's window half (tests/cases/plugins/TC-059): a Settings section "Jobs demo" listing Echo's jobs with
// Needs you now's interval, a button that sets it to 10 minutes on the click, and what a change without a click got.
export function apply(ctx) {
  let list = null
  let tried = 'not tried'
  const load = () => ctx.echo.jobs.list().then((l) => { list = l; ctx.render() }).catch((e) => { list = { error: String(e.message || e) }; ctx.render() })
  load()
  // A change asked for without the user's click or key: refused.
  ctx.echo.jobs.set('needs', ['every=20']).then(() => { tried = 'changed' }, (e) => { tried = 'refused: ' + (e.message || e) }).then(() => ctx.render())
  ctx.settings.section({
    id: 'main',
    label: 'Jobs demo',
    render: () => {
      const esc = ctx.ui.esc
      if (!list) return '<div class="set-card" data-jd>Loading…</div>'
      if (list.error) return `<div class="set-card" data-jd><div class="set-row" data-jd-error>${esc(list.error)}</div></div>`
      const needs = list.jobs.find((j) => j.id === 'needs')
      const every = needs.fields.find((f) => f.key === 'every').value
      return `<div class="set-card" data-jd><div class="set-row" data-jd-jobs>${list.jobs.length} jobs: ${esc(list.jobs.map((j) => j.name).join(', '))}</div>` +
        `<div class="set-row"><div style="flex:1" data-jd-every>Needs you now every ${esc(String(every))} min</div><button class="btn" data-jd-set>Every 10 min</button></div>` +
        `<div class="set-row" data-jd-tried>Without a click: ${esc(tried)}</div></div>`
    },
  })
  ctx.listen('click', (ev) => {
    if (ev.target && ev.target.closest && ev.target.closest('[data-jd-set]')) {
      ctx.echo.jobs.set('needs', ['every=10']).then((l) => { list = l; ctx.ui.notice('Needs you now: every 10 minutes'); ctx.render() }, (e) => ctx.ui.notice(String(e.message || e)))
    }
  })
}
