// A test plugin (tests/cases/plugins/TC-216, TC-217): a Claude run under a key, stopped by that key; what Echo records now.
// The test data's fake Claude Code goes on until it is stopped when the prompt holds "echo-test: keep running".
export function apply(ctx) {
  ctx.command('start', async (a) => {
    try {
      const r = await ctx.claude.run({ prompt: 'echo-test: keep running. Index the notes of Acme.', title: 'Run demo', key: a.key, dir: 'runs/' + a.key })
      return { answer: r.answer }
    } catch (e) {
      return { error: String((e && e.message) || e) }
    }
  })
  ctx.command('stop', async (a) => await ctx.claude.stop(a.key))
  ctx.command('recording', () => ctx.echo.read('recording'))
}
