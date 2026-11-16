// The test plugin's window half (tests/cases/today/TC-040): a "Snooze" button on its own work rows only (task.actions).
export function apply(ctx) {
  ctx.slots.add('task.actions', {
    id: 'snooze',
    label: 'Snooze',
    when: (t) => t.kind === 'plugin' && t.source === ctx.id,
    run: async (t) => { await ctx.call('snooze', { id: t.row.id }); ctx.ui.notice(`Snoozed ${t.row.title}`) },
  })
}
