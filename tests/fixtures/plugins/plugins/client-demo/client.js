// The test plugin's window half (tests/cases/plugins/TC-160): a "Vault" row in each client's Profile — the client's
// vault and a box to set it; a client named "Boom" makes it throw (the Profile draws without it).
export function apply(ctx) {
  const got = {}
  ctx.slots.add('client.profile', {
    id: 'vault',
    label: 'Vault',
    render: (c) => {
      if (c.name === 'Boom') throw new Error('client-demo breaks on Boom')
      const vault = got[c.id]
      if (vault === undefined) return '<span class="pf-sub">Reading…</span>'
      return `${vault ? `<span class="tok">▤ ${ctx.ui.esc(vault)}</span>` : '<span class="pf-sub pf-inl">The default vault</span>'} <input class="pf-in" data-cd-vault placeholder="Acme notes" aria-label="Vault"> <button class="btn" data-cd-save>Save</button>`
    },
    after: (box, c) => {
      if (got[c.id] === undefined) { got[c.id] = null; ctx.call('get', { client: c.id }).then((v) => { got[c.id] = v; ctx.render() }) }
      const save = box.querySelector('[data-cd-save]')
      if (save) save.onclick = () => {
        const vault = box.querySelector('[data-cd-vault]').value.trim()
        ctx.call('set', { client: c.id, vault }).then((v) => { got[c.id] = v; ctx.render() })
      }
    },
  })
}
