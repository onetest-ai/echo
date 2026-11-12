// A test plugin (tests/cases/today/TC-040): items for the user (My work) and for the thread acme-rollout (its Work tab),
// one of them in both (shown once there); `fail` makes it throw, `ok` brings it back; `snooze` (from its task.actions
// button) takes a row away.
export function apply(ctx) {
  // n days from today, by this computer's calendar (as Echo dates its tasks), not UTC's.
  const day = (n) => { const d = new Date(); d.setDate(d.getDate() + n); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}` }
  let failing = false
  const gone = new Set()
  const review = () => ({ id: 'pr-7', title: 'Review: fix login', kind: 'review', where: 'acme-test/api', url: 'https://git.acme.example/acme-test/api/pull/7', due: day(-2), thread: 'acme-rollout' })
  ctx.work.provide(({ scope, thread }) => {
    if (failing) throw new Error('work-demo is failing on purpose')
    const rows = scope === 'me'
      // is-3 due today (this week whatever the day); is-9 undated, so not on My work (as an undated task isn't).
      ? [{ id: 'is-3', title: 'Flaky login test <b>', kind: 'assigned', where: 'acme-test/web', url: 'https://git.acme.example/acme-test/web/issues/3', due: day(0) }, { id: 'is-9', title: 'Tidy the labels', kind: 'assigned', where: 'acme-test/web' }, review()]
      : thread === 'acme-rollout' ? [{ id: 'pr-8', title: 'Bump dependencies', kind: 'PR', where: 'acme-test/api', url: 'https://git.acme.example/acme-test/api/pull/8' }, review()] : []
    return rows.filter((r) => !gone.has(r.id))
  })
  ctx.command('fail', () => { failing = true; ctx.work.changed(); return true })
  ctx.command('ok', () => { failing = false; gone.clear(); ctx.work.changed(); return true })
  ctx.command('snooze', ({ id }) => { gone.add(id); ctx.work.changed(); return true })
}
