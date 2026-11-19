// A test plugin (tests/cases/plugins/TC-216, TC-217, TC-242): a Claude run under a key, stopped by that key; what Echo
// records now; a run with pictures, its answer streamed, in a conversation it continues and forgets.
// The test data's fake Claude Code goes on until it is stopped when the prompt holds "echo-test: keep running", and
// answers in a stream when it holds "echo-test: talk".
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
  ctx.command('talk', async (a, { files }) => {
    const pieces = []
    try {
      const r = await ctx.claude.run({ prompt: 'echo-test: talk. ' + (a.say || 'Describe the pictures.'), title: 'Run demo talk', tools: 'none',
        profile: a.profile, images: files, resume: a.resume, onText: (t) => pieces.push(t), dir: 'runs/talk' })
      return { answer: r.answer, pieces }
    } catch (e) {
      return { error: String((e && e.message) || e) }
    }
  })
  ctx.command('forget', async (a) => await ctx.claude.forget(a.resume))
}
