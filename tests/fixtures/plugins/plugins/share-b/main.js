// A test plugin (tests/cases/plugins/TC-033): sets and reads values shared by its marketplace's plugins — per thread
// (ctx.threads with a key <ns>.<name>) and for the computer (ctx.shared) — and keeps what it heard of threads changed
// (TC-033) and of shared values changed (TC-043).
export function apply(ctx) {
  ctx.command('tset', ({ thread, key, value }) => ctx.threads.set(thread, key, value))
  ctx.command('tget', ({ thread, key }) => ctx.threads.get(thread, key))
  ctx.command('tlist', ({ key }) => ctx.threads.list(key))
  ctx.command('sset', ({ key, value }) => ctx.shared.set(key, value))
  ctx.command('sget', ({ key }) => ctx.shared.get(key))
  ctx.command('heardShared', () => JSON.parse(ctx.store.read('heard-shared.json') || '[]'))
  ctx.command('heard', () => JSON.parse(ctx.store.read('heard.json') || '[]'))
  ctx.on('threads_changed', (p) => { const h = JSON.parse(ctx.store.read('heard.json') || '[]'); h.push(p); ctx.store.write('heard.json', JSON.stringify(h)) })
  // A value its marketplace's plugins share changed (#612), told to the others.
  ctx.on('shared_changed', (p) => { const h = JSON.parse(ctx.store.read('heard-shared.json') || '[]'); h.push(p); ctx.store.write('heard-shared.json', JSON.stringify(h)) })
}
