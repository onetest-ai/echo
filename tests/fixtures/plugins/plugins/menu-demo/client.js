// The test plugin's window half (tests/cases/plugins/TC-181): one item with a fixed label in each call's ⋯ menu.
export function apply(ctx) {
  ctx.slots.add('call.menu', { id: 'hello', label: 'Say hello to the call', run: (t) => ctx.ui.notice(`Hello, ${t.call.title}`) })
}
