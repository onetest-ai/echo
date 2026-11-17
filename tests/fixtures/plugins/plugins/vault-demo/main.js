// The test plugin's host half (tests/cases/plugins/TC-170 to TC-176): its one made-up vault, kept in its store.
export function apply(ctx) {
  const vault = () => JSON.parse(ctx.store.read('vault.json') || 'null')
  ctx.command('vault', () => vault())
  ctx.command('add-vault', ({ name }) => { ctx.store.write('vault.json', JSON.stringify({ name: String(name || 'Acme vault') })); return vault() })
  ctx.command('remove-vault', () => { ctx.store.write('vault.json', 'null'); return null })
}
