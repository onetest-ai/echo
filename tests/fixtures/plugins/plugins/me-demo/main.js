// The test plugin's host half (tests/cases/plugins/TC-160, TC-161): reads About me with the options it is given.
export function apply(ctx) {
  ctx.command('me', (a) => ctx.me(a || {}))
}
