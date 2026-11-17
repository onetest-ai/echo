// A test plugin (tests/cases/plugins/TC-056–TC-058, today/TC-052): widgets that show the page they are on, offered as
// dashboards; a team table longer than its card.
const TEAM = Array.from({ length: 20 }, (_, i) => ({ id: `m${i + 1}`, cells: { name: `Person ${i + 1}`, role: i % 3 ? 'Engineer' : 'Lead' } }))

export function apply(ctx) {
  ctx.widgets.provide('glance', ({ params }) => ({ value: 3, label: `people at ${params.client}` }))
  ctx.widgets.provide('people', ({ params }) => ({ items: [{ title: 'Dana Lee', sub: params.order === 'role' ? 'lead' : '' }, { title: 'Omar Haddad', sub: params.order === 'role' ? 'engineer' : '' }] }))
  ctx.widgets.provide('pipe', ({ params }) => ({ items: [{ title: `Next step for ${params.thread}` }] }))
  // All 20 rows, the first the most important: Echo draws the ones the card holds and "+N more".
  ctx.widgets.provide('team', ({ height, fits }) => ({ columns: [{ key: 'name', label: 'Name' }, { key: 'role', label: 'Role', kind: 'pill' }],
    rows: TEAM, note: `drawn: ${fits ? fits.rows : '?'} rows at height ${height || '?'}` }))
}
