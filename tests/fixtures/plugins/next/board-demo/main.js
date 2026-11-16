// A test plugin (tests/cases/plugins/TC-056–TC-058): widgets that show the page they are on, offered as dashboards.
export function apply(ctx) {
  ctx.widgets.provide('glance', ({ params }) => ({ value: 3, label: `people at ${params.client}` }))
  ctx.widgets.provide('people', ({ params }) => ({ items: [{ title: 'Dana Lee', sub: params.order === 'role' ? 'lead' : '' }, { title: 'Omar Haddad', sub: params.order === 'role' ? 'engineer' : '' }] }))
  ctx.widgets.provide('pipe', ({ params }) => ({ items: [{ title: `Next step for ${params.thread}` }] }))
}
