// The test plugin's window half (tests/cases/plugins/TC-059): a Settings section "Jobs demo" listing Echo's jobs with
// Needs you now's interval and two buttons, each a change on the click: every 10 minutes, and every 0 (refused).
export function apply(ctx) {
  let list = null
  ctx.echo.jobs.list().then((l) => { list = l; ctx.render() }).catch((e) => { list = { error: String(e.message || e) }; ctx.render() })
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
        `<div class="set-row"><div style="flex:1" data-jd-every>Needs you now every ${esc(String(every))} min</div><button class="btn" data-jd-set="10">Every 10 min</button><button class="btn" data-jd-set="0">Every 0 min</button></div></div>`
    },
  })
  ctx.listen('click', (ev) => {
    const b = ev.target && ev.target.closest && ev.target.closest('[data-jd-set]')
    if (b) {
      ctx.echo.jobs.set('needs', [`every=${b.dataset.jdSet}`]).then((l) => { list = l; ctx.ui.notice(`Needs you now: every ${b.dataset.jdSet} minutes`); ctx.render() }, (e) => ctx.ui.notice(String(e.message || e)))
    }
  })
}
