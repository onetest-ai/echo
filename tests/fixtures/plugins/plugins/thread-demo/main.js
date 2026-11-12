// A test plugin (tests/cases/plugins/TC-023): the repositories of a thread, kept per thread (ctx.threads), and what it
// heard of threads renamed, merged or deleted.
export function apply(ctx) {
  ctx.command('get', ({ thread }) => ctx.threads.get(thread, 'repos'))
  ctx.command('set', ({ thread, repos }) => { ctx.threads.set(thread, 'repos', repos && repos.length ? repos : null); return ctx.threads.get(thread, 'repos') })
  ctx.command('list', () => ctx.threads.list('repos'))
  ctx.command('heard', () => JSON.parse(ctx.store.read('heard.json') || '[]'))
  ctx.on('threads_changed', (p) => { const h = JSON.parse(ctx.store.read('heard.json') || '[]'); h.push(p); ctx.store.write('heard.json', JSON.stringify(h)) })
}
