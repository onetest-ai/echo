// The test plugin's window half (tests/cases/plugins/TC-170 to TC-176): three tabs of Knowledge — Pages (only once a vault
// exists; `#brain/vault-demo:pages/<page>` opens a page, its name encoded; a search), Inbox (files dropped while any of
// its tabs is shown; its badge their count; no search) and Vaults ('!' while there is none) —, a header strip naming the
// vault with an "Ask the vault" box, and a card in Settings → Knowledge. Its state lives in this page, never reset by
// moving between its tabs; `window.vaultDemoUnloads` counts its onUnload runs.
const PAGES = Array.from({ length: 60 }, (_, i) => `Page ${String(i + 1).padStart(2, '0')}${i === 6 ? ' Q3 plan/draft' : ''}`)
const ICON = (d) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`
export function apply(ctx) {
  const esc = ctx.ui.esc
  const A = { vault: undefined, q: '', dropped: [], asked: '' }
  ctx.onUnload(() => { window.vaultDemoUnloads = (window.vaultDemoUnloads || 0) + 1 })
  ctx.call('vault').then((v) => { A.vault = v; ctx.render() })
  const has = () => !!A.vault
  ctx.slots.add('knowledge.header', {
    header: () => `<div style="display:flex;gap:10px;align-items:center;padding:8px 22px"><b>${has() ? esc(A.vault.name) : 'No vault yet'}</b><input id="vd-ask" placeholder="Ask the vault" aria-label="Ask the vault" style="width:240px"><span id="vd-asked">${esc(A.asked)}</span></div>`,
    after: (box) => {
      const i = box.querySelector('#vd-ask')
      if (i && !i._vd) { i._vd = true; i.addEventListener('keydown', (e) => { if (e.key === 'Enter') { A.asked = `Asked: ${i.value}`; ctx.render() } }) }
    },
  })
  ctx.slots.add('knowledge.tab', {
    id: 'pages', label: 'Pages', order: 10, icon: ICON('<path d="M6 3h9l3 3v15H6z"/><path d="M9 9h6M9 13h6M9 17h4"/>'),
    visible: has,
    onSearch: (q) => { A.q = q; ctx.render() },
    render: (rest) => {
      let page = ''
      try { page = decodeURIComponent(rest || '') } catch { page = rest }
      const list = PAGES.filter((p) => p.toLowerCase().includes(A.q.toLowerCase()))
      return `<div style="display:flex;min-height:100%"><nav aria-label="Pages" style="width:260px;flex:none;border-right:1px solid var(--line)">${list.map((p) => `<a style="display:block;padding:6px 14px" href="#brain/vault-demo:pages/${encodeURIComponent(p)}">${esc(p)}</a>`).join('')}</nav>`
        + `<article aria-label="Page" style="padding:16px 22px"><h2>${page ? esc(page) : 'Pick a page'}</h2><p class="set-sub">Route: ${esc(rest || '(none)')}</p></article></div>`
    },
  })
  ctx.slots.add('knowledge.tab', {
    id: 'inbox', label: 'Inbox', order: 20, icon: ICON('<path d="M4 13l2.5-7h11L20 13v6H4z"/><path d="M4 13h5l1 2h4l1-2h5"/>'),
    search: false,
    badge: () => A.dropped.length,
    render: () => `<div style="padding:16px 22px"><h2>Inbox</h2>${A.dropped.length ? `<ul aria-label="Dropped files">${A.dropped.map((f) => `<li>${esc(f.name)} · ${esc(f.where)}</li>`).join('')}</ul>` : '<p class="set-sub">Drop files on any of Vault demo’s tabs.</p>'}</div>`,
  })
  ctx.slots.add('knowledge.tab', {
    id: 'vaults', label: 'Vaults', order: 90, icon: ICON('<rect x="4" y="5" width="16" height="14" rx="2"/><circle cx="12" cy="12" r="3"/>'),
    badge: () => (A.vault === null ? '!' : ''),
    render: () => `<div style="padding:16px 22px"><h2>Vaults</h2>${has() ? `<p>${esc(A.vault.name)}</p><button class="btn" data-vd-remove>Remove the vault</button>` : '<p class="set-sub">No vault yet.</p><button class="btn primary" data-vd-add>Add a vault</button>'}</div>`,
  })
  ctx.slots.add('knowledge.settings', {
    label: 'Vault demo',
    render: () => ctx.ui.setRow('Vault', has() ? esc(A.vault.name) : 'None yet', '<button class="btn" data-vd-open>Open its tabs</button>'),
  })
  ctx.onFileDrop((files) => { const w = ctx.where(); for (const f of files) A.dropped.push({ name: f.name, where: w.tab }); ctx.render() })
  ctx.listen('click', (e) => {
    const t = e.target.closest && e.target.closest('[data-vd-add],[data-vd-remove],[data-vd-open]')
    if (!t) return
    if ('vdAdd' in t.dataset) ctx.call('add-vault', { name: 'Acme vault' }).then((v) => { A.vault = v; ctx.render() })
    if ('vdRemove' in t.dataset) ctx.call('remove-vault').then(() => { A.vault = null; ctx.render() })
    if ('vdOpen' in t.dataset) ctx.go('brain/vault-demo:vaults')
  })
}
