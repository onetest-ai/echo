// Compatibility routes and owned rendering hooks for a native Knowledge plugin client.
'use strict'

// ── kn-route: `#kn/<vault>/<tab>/<path>` (a page's path encoded as one part); a vault opens on Home (spec-2 K10-01) ──
const KN_TABS = ['home', 'pages', 'graph', 'ask', 'ingest', 'health', 'settings']
/** `decodeURIComponent`, or `null` for a broken escape (a `%` that is the name's own). */
function knDecode(s) {
  try { return decodeURIComponent(s) } catch { return null }
}
function knParseRoute(hash) {
  const parts = String(hash || '').split('/')
  if (parts[0] !== 'kn') return null
  const vault = parts[1] ? knDecode(parts[1]) : ''
  if (vault === null) return null
  if (vault && !/^[\p{L}\p{N}_-]+$/u.test(vault)) return null
  const tab = parts[2] || 'home'
  if (!KN_TABS.includes(tab)) return null
  const path = parts.length > 3 ? knDecode(parts.slice(3).join('/')) : ''
  if (path === null) return null
  return { vault, tab, path }
}
function knRouteOf({ vault, tab = 'home', path = '' }) {
  return `kn/${encodeURIComponent(vault)}/${tab}${path ? '/' + encodeURIComponent(path) : ''}`
}
// end of the kn-route block

const Kn = (() => {
  let owner = null, view = null, pending = null, source = 'vault'
  const empty = () => ''
  const missing = () => `<div class="wk-empty"><b>Knowledge is a plugin</b><p>Install or enable Knowledge in Settings → Plugins to open your vaults. Your folders and saved Knowledge state stay where they are.</p><button class="btn primary" data-go="settings/plugins">Open plugins</button></div>`
  const invoke = (name, fallback, args) => view && typeof view[name] === 'function' ? view[name](...args) : fallback(...args)
  const api = {
    register(id, next) {
      if (owner && owner !== id) throw new Error('A Knowledge client is already registered')
      if (!next || !['view', 'route', 'load', 'active'].every((k) => typeof next[k] === 'function')) throw new Error('Incomplete Knowledge client')
      owner = id; view = next
      view.setSource(source)
      if (pending) view.route(pending)
      queueMicrotask(async () => {
        if (view !== next) return
        route()
        await next.load()
        if (view === next) render()
      })
    },
    unload(id) { if (owner === id) { view = null; owner = null } },
    route(r) { pending = r; source = 'vault'; if (view) view.route(r) },
    setSource(value) { source = value; if (view) view.setSource(value) },
    active: () => view ? view.active() : source === 'vault',
    home: () => view ? view.home() : 'kn',
    title: () => view ? view.title() : 'Knowledge',
    view: () => view ? view.view() : missing(),
    settingsVaults: () => view ? view.settingsVaults() : missing(),
    editing: () => !!(view && view.editing()),
    vaultId: () => view ? view.vaultId() : '',
    askLabel: () => view ? view.askLabel() : 'Claude · vault',
  }
  for (const name of ['after', 'overlay', 'callChip', 'callMenuItems', 'countdownLine', 'paletteActions', 'askView', 'feedQueued', 'dropped', 'syncDrops']) api[name] = (...args) => invoke(name, empty, args)
  for (const name of ['tick', 'load', 'resume']) api[name] = (...args) => invoke(name, () => Promise.resolve(), args)
  return api
})()
