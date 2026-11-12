// The test plugin's window half (tests/cases/plugins/TC-025): a Settings section "Sign-in demo" — signed in or not,
// "Sign in with GitHub" showing the code and GitHub's address (opened only after Echo's confirmation), "Sign out".
export function apply(ctx) {
  const esc = ctx.ui.esc
  let st = null, started = null, error = '', timer = null
  const load = async () => {
    try { st = await ctx.call('status') } catch (e) { error = e.message }
    if (st && st.state !== 'waiting' && timer) { clearInterval(timer); timer = null; started = null }
    ctx.render()
  }
  ctx.settings.section({
    id: 'main',
    label: 'Sign-in demo',
    render() {
      if (!st && !error) { load(); return '<p class="set-sub">Reading…</p>' }
      const said = { denied: 'GitHub said no: not signed in.', expired: 'The code ran out: not signed in.', failed: `Not signed in: ${esc((st && st.error) || '')}` }[st && st.state] || ''
      const waiting = st && st.state === 'waiting' && started
      return `<div class="set-card" data-sd>
        <div class="set-row"><div style="flex:1">${st && st.signed_in ? 'Signed in to GitHub by Echo' : 'Not signed in to GitHub'}</div>
        ${st && st.signed_in ? '<button class="btn" data-sd-out>Sign out</button>' : waiting ? '<button class="btn" data-sd-cancel>Cancel</button>' : '<button class="btn primary" data-sd-in>Sign in with GitHub</button>'}</div>
        ${waiting ? `<div class="set-row"><div>Enter <b data-sd-code>${esc(started.code)}</b> at <a class="wp-link" data-ext href="${esc(started.url)}">${esc(started.url)}</a></div></div>` : ''}
        ${said ? `<p class="set-sub" data-sd-said>${said}</p>` : ''}
        ${error ? `<p class="error">${esc(error)}</p>` : ''}</div>`
    },
  })
  const listen = ctx.listen || ((type, fn) => document.addEventListener(type, fn))
  listen('click', async (ev) => {
    const t = ev.target
    if (!t || !t.closest || !t.closest('[data-sd]')) return
    error = ''
    try {
      if (t.closest('[data-sd-in]')) {
        started = await ctx.call('start'); await load()
        if (!timer) timer = setInterval(load, 1000)
      } else if (t.closest('[data-sd-cancel]')) {
        await ctx.call('cancel'); await load()
      } else if (t.closest('[data-sd-out]')) {
        await ctx.call('forget'); await load()
      }
    } catch (e) { error = e.message; ctx.render() }
  })
}
