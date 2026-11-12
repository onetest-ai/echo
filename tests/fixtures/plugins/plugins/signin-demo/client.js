// The test plugin's window half (tests/cases/plugins/TC-025): a Settings section "Sign-in demo" whose button copies a
// made-up code (`ctx.ui.copy`, on the click), and a copy tried by a timer 1 s after it loads, with no click — which
// Echo refuses; the section says how that went.
export function apply(ctx) {
  let timer = 'not tried yet'
  setTimeout(() => {
    try { ctx.ui.copy('TIMER-COPY'); timer = 'copied without a click' } catch (e) { timer = `refused: ${e.message}` }
    ctx.render()
  }, 1000)
  ctx.settings.section({
    id: 'main',
    label: 'Sign-in demo',
    render: () => `<div class="set-card" data-sdc><div class="set-row"><div style="flex:1">Code <b>MADE-UP1</b></div><button class="btn" data-sdc-copy>Copy code</button></div><p class="set-sub" data-sdc-timer>A copy with no click: ${ctx.ui.esc(timer)}</p></div>`,
  })
  const listen = ctx.listen || ((type, fn) => document.addEventListener(type, fn))
  listen('click', (ev) => {
    if (ev.target && ev.target.closest && ev.target.closest('[data-sdc-copy]')) { ctx.ui.copy('MADE-UP1'); ctx.ui.notice('Copied') }
  })
}
