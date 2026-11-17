// The test plugin's window half (tests/cases/plugins/TC-105): a chip "Has chat" on each call's row whose call has a
// meeting chat, from its host half's list of calls.
export function apply(ctx) {
  let withChat = null
  ctx.slots.add('call.row', {
    id: 'chat',
    label: 'Data demo: this call has a meeting chat',
    chip: (c) => {
      if (withChat === null) {
        withChat = new Set()
        ctx.call('calls', { limit: 500 }).then((list) => { withChat = new Set(list.filter((x) => x.chat).map((x) => x.id.slice(5))); ctx.render() })
      }
      return withChat.has(c.name) ? 'Has chat' : null
    },
  })
}
