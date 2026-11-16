// The test plugin's host half (tests/cases/plugins/TC-025): has Echo keep a sign-in it "made" (a made-up token given
// to it), says where the sign-in it is handed comes from — its length, never the token itself — and forgets it.
export function apply(ctx) {
  ctx.command('keep', (a) => ctx.signin.keep(a.host || 'github.com', a.token))
  ctx.command('forget', (a) => ctx.signin.forget((a && a.host) || 'github.com'))
  ctx.command('who', async () => { const s = await ctx.signin.get('github.com'); return s && { from: s.from, length: s.token.length } })
}
