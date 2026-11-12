// The test plugin's window half (tests/cases/plugins/TC-023): a "Repositories" row in each thread's Profile — its
// repositories as chips and a box to set them; a thread named "Boom" makes it throw (the Profile draws without it).
export function apply(ctx) {
  const got = {}
  ctx.slots.add('thread.profile', {
    id: 'repos',
    label: 'Repositories',
    render: (t) => {
      if (t.name === 'Boom') throw new Error('thread-demo breaks on Boom')
      const repos = got[t.id]
      if (repos === undefined) return '<span class="pf-sub">Reading…</span>'
      const chips = (repos || []).map((r) => `<span class="tok">⎇ ${ctx.ui.esc(r)}</span>`).join('')
      return `${chips || '<span class="pf-sub pf-inl">None yet</span>'} <input class="pf-in" data-td-repos placeholder="acme-test/api, acme-test/web" aria-label="Repositories"> <button class="btn" data-td-save>Save</button>`
    },
    after: (box, t) => {
      if (got[t.id] === undefined) { got[t.id] = null; ctx.call('get', { thread: t.id }).then((v) => { got[t.id] = v; ctx.render() }) }
      const save = box.querySelector('[data-td-save]')
      if (save) save.onclick = () => {
        const repos = box.querySelector('[data-td-repos]').value.split(',').map((s) => s.trim()).filter(Boolean)
        ctx.call('set', { thread: t.id, repos }).then((v) => { got[t.id] = v; ctx.render() })
      }
    },
  })
}
