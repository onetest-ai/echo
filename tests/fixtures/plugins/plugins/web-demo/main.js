// A made-up plugin for tests: ctx.web on the pages of the made-up site in tests/fixtures/web-site.
export function apply(ctx) {
  ctx.command('read', (a) => ctx.web.read(a.url, a.opts || {}))
  ctx.command('fetch', (a) => ctx.web.fetch(a.url, a.opts || {}))
  ctx.command('markdown', (a) => ctx.web.markdown(a.html, a.opts || {}))
}
