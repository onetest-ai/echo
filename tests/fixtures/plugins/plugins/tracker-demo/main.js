// A test plugin (tests/cases/plugins/TC-054, TC-055): a made-up tracker's pursuits moving a thread's stage
// (ctx.threads.setStage) and its next steps as the thread's tasks (ctx.tasks), by the pursuit's key.
export function apply(ctx) {
  ctx.command('threads', () => ctx.echo.read('threads'))
  ctx.command('stage', ({ thread, stage, reason }) => ctx.threads.setStage(thread, stage, reason))
  ctx.command('task', ({ thread, task }) => ctx.tasks.upsert(thread, task))
  ctx.command('close', ({ thread, key, reason, propose }) => ctx.tasks.close(thread, key, reason, { propose }))
}
