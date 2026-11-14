// A test plugin (tests/cases/plugins/TC-042): its widget shows how many times it was bumped. `bump` tells Echo it
// changed (ctx.widgets.changed); `bumpQuiet` doesn't, so Today shows the old count until its hour-long refresh.
export function apply(ctx) {
  const n = () => Number(ctx.store.read('n') || '0')
  ctx.widgets.provide('count', () => ({ value: n(), label: 'bumps' }))
  ctx.command('bump', () => { ctx.store.write('n', String(n() + 1)); ctx.widgets.changed('count'); return n() })
  ctx.command('bumpQuiet', () => { ctx.store.write('n', String(n() + 1)); return n() })
}
