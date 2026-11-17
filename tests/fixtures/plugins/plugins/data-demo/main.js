// The test plugin's host half (tests/cases/plugins/TC-100 to TC-105): each command reads one thing and answers it.
export function apply(ctx) {
  ctx.command('calls', (a) => ctx.echo.read('calls', a || {}))
  ctx.command('source', (a) => ctx.echo.read('source', a.id))
  ctx.command('calendar', () => ctx.echo.read('calendar'))
  ctx.command('me', () => ctx.echo.read('me'))
  ctx.command('threads', () => ctx.echo.read('threads'))
  ctx.command('search', (a) => ctx.m365.search(a.q, a.opts || {}))
  ctx.command('thread', (a) => ctx.m365.mailThread(a.id))
  ctx.command('chat', (a) => ctx.m365.chat(a.id, a.opts || {}))
  // Asks the user for an address in Echo's box, then fetches it: what it got, cut — and what a second fetch of it said.
  ctx.command('fetch-typed', async () => {
    const address = await ctx.http.askAddress()
    if (!address) return { address: null }
    const r = await ctx.http.fetch(address)
    let again = 'fetched again'
    try { await ctx.http.fetch(address) } catch (e) { again = e.message }
    return { address, status: r.status, body: r.body.slice(0, 200), again }
  })
  ctx.command('fetch', async (a) => {
    const r = await ctx.http.fetch(a.url, a.opts || {})
    return { status: r.status, body: r.body.slice(0, 200) }
  })
}
