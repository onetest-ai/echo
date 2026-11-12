// The test plugin's host half (tests/cases/plugins/TC-025): Echo's device sign-in to github.com, and where the
// sign-in it is handed comes from — its length, never the token itself.
export function apply(ctx) {
  ctx.command('start', async () => await ctx.signin.device.start('github.com'))
  ctx.command('status', () => ctx.signin.device.status('github.com'))
  ctx.command('cancel', () => ctx.signin.device.cancel('github.com'))
  ctx.command('forget', () => ctx.signin.device.forget('github.com'))
  ctx.command('who', async () => { const s = await ctx.signin.get('github.com'); return s && { from: s.from, length: s.token.length, scopes: s.scopes || null } })
}
