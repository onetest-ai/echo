// A test plugin (tests/cases/plugins/TC-045): what it heard of meetings about to start, kept in its store.
export function apply(ctx) {
  ctx.command('heard', () => JSON.parse(ctx.store.read('heard.json') || '[]'))
  ctx.on('meeting_soon', (p) => { const h = JSON.parse(ctx.store.read('heard.json') || '[]'); h.push(p); ctx.store.write('heard.json', JSON.stringify(h)) })
}
