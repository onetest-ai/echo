// A test plugin (tests/cases/plugins/TC-180 to TC-183): which calls it filed, kept in its own store by call name, and
// how many times its sheet was closed.
export function apply(ctx) {
  const filed = () => JSON.parse(ctx.store.read('filed.json') || '{}')
  ctx.command('get', ({ call }) => !!filed()[call])
  ctx.command('file', ({ call, on }) => {
    const f = filed()
    if (on) f[call] = true; else delete f[call]
    ctx.store.write('filed.json', JSON.stringify(f))
    return !!f[call]
  })
  ctx.command('count', () => Object.keys(filed()).length)
  ctx.command('closed', () => { const n = Number(ctx.store.read('closed') || '0') + 1; ctx.store.write('closed', String(n)); return n })
  ctx.command('closes', () => Number(ctx.store.read('closed') || '0'))
}
