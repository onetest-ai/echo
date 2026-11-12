// The test plugin's window half (tests/cases/plugins/TC-025): a Settings section "Sign-in demo" whose "Copy code"
// copies a made-up code on the click (`ctx.ui.copy`).
export function apply(ctx) {
  ctx.settings.section({
    id: 'main',
    label: 'Sign-in demo',
    render: () => '<div class="set-card" data-sdc><div class="set-row"><div style="flex:1">Code <b>MADE-UP1</b></div><button class="btn" data-sdc-copy>Copy code</button></div></div>',
  })
  const listen = ctx.listen || ((type, fn) => document.addEventListener(type, fn))
  listen('click', (ev) => {
    if (ev.target && ev.target.closest && ev.target.closest('[data-sdc-copy]')) { ctx.ui.copy('MADE-UP1'); ctx.ui.notice('Copied') }
  })
}
