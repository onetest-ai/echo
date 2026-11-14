// A test plugin: its guides for Claude given at run time (`ctx.agent.set`), the whole set each call.
export function apply(ctx) {
  ctx.command('guides', (a) => {
    try {
      ctx.agent.set(a)
      return { ok: true }
    } catch (e) {
      return { error: String((e && e.message) || e) }
    }
  })
}
