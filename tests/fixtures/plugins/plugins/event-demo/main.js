// A test plugin (tests/cases/plugins/TC-045): what it heard of meetings about to start, and of minutes written, kept in its store.
export function apply(ctx) {
  const keep = (file, p) => { const h = JSON.parse(ctx.store.read(file) || '[]'); h.push(p); ctx.store.write(file, JSON.stringify(h)) }
  ctx.command('heard', () => JSON.parse(ctx.store.read('heard.json') || '[]'))
  ctx.command('minutes', () => JSON.parse(ctx.store.read('minutes.json') || '[]'))
  ctx.on('meeting_soon', (p) => keep('heard.json', p))
  ctx.on('minutes_written', (p) => keep('minutes.json', p))
}
