// A test plugin (tests/cases/plugins/TC-160): the notes vault of a client, kept per client (ctx.clients).
export function apply(ctx) {
  ctx.command('get', ({ client }) => ctx.clients.get(client, 'vault'))
  ctx.command('set', ({ client, vault }) => { ctx.clients.set(client, 'vault', vault ? vault : null); return ctx.clients.get(client, 'vault') })
  ctx.command('list', () => ctx.clients.list('vault'))
}
