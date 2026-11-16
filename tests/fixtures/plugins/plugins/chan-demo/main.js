// A test plugin (tests/cases/plugins/TC-034): asks Echo to install a Claude Code plugin it names.
export function apply(ctx) {
  ctx.command('install', async ({ spec }) => { await ctx.claude.installPlugin(spec); return true })
}
