// A test plugin (tests/cases/today/TC-051): a table declared `height: "fill"`, longer than the window.
export function apply(ctx) {
  ctx.widgets.provide('rows', () => ({
    columns: [{ key: 'name', label: 'Name' }, { key: 'n', label: 'Count', kind: 'number' }],
    rows: Array.from({ length: 60 }, (_, i) => ({ id: `r${i + 1}`, cells: { name: `Row ${i + 1}`, n: i + 1 } })),
  }))
}
