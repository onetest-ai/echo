// A test plugin (tests/cases/today/TC-040): items for the user (My work) and for the thread acme-rollout (its Work tab),
// one of them in both (shown once there); `fail` makes it throw, `ok` brings it back; `snooze` (from its task.actions
// button) takes a row away.
export function apply(ctx) {
  const day = (n) => new Date(Date.now() + n * 86400000).toISOString().slice(0, 10)
  let failing = false
  const gone = new Set()
  const review = () => ({ id: 'pr-7', title: 'Review: fix login', kind: 'review', where: 'acme-test/api', url: 'https://git.acme.example/acme-test/api/pull/7', due: day(-2), thread: 'acme-rollout' })
  ctx.work.provide(({ scope, thread }) => {
    if (failing) throw new Error('work-demo is failing on purpose')
    const rows = scope === 'me'
      ? [{ id: 'is-3', title: 'Flaky login test <b>', kind: 'assigned', where: 'acme-test/web', url: 'https://git.acme.example/acme-test/web/issues/3', due: day(1) }, review()]
      : thread === 'acme-rollout' ? [{ id: 'pr-8', title: 'Bump dependencies', kind: 'PR', where: 'acme-test/api', url: 'https://git.acme.example/acme-test/api/pull/8' }, review()] : []
    return rows.filter((r) => !gone.has(r.id))
  })
  ctx.command('fail', () => { failing = true; ctx.work.changed(); return true })
  ctx.command('ok', () => { failing = false; gone.clear(); ctx.work.changed(); return true })
  ctx.command('snooze', ({ id }) => { gone.add(id); ctx.work.changed(); return true })
}
