// The test plugin's window half (tests/cases/plugins/TC-230): a Settings section "Routines demo" whose buttons open
// Echo's sheet of its routines on the click (`ctx.echo.routines.offer`), all of them or only "Vault health", and one
// asking for a routine it hasn't; what the sheet resolves to is shown.
export function apply(ctx) {
  let got = ''
  ctx.settings.section({
    id: 'main',
    label: 'Routines demo',
    render: () => {
      const esc = ctx.ui.esc
      return '<div class="set-card" data-rd>' +
        `<div class="set-row"><div style="flex:1">Set up its routines</div><button class="btn" data-rd-offer="">All routines</button><button class="btn" data-rd-offer="vault-health">Only Vault health</button><button class="btn" data-rd-offer="nope">A routine it hasn't</button></div>` +
        `<div class="set-row" data-rd-result>${esc(got || 'Nothing chosen yet')}</div></div>`
    },
  })
  ctx.listen('click', (ev) => {
    const b = ev.target && ev.target.closest && ev.target.closest('[data-rd-offer]')
    if (!b) return
    const ids = b.dataset.rdOffer ? [b.dataset.rdOffer] : undefined
    ctx.echo.routines.offer(ids).then(
      (list) => { got = list.map((r) => `${r.id} ${r.on ? 'on' : 'off'}`).join(', ') },
      (e) => { got = `Refused: ${e.message || e}` },
    ).then(() => ctx.render())
  })
}
