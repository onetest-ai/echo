// ── What's new (`whats_new.rs`): after an update, a dialog with what the versions since the last one seen bring; again
// from ⌘K and Settings → General.

/** The icons an item may name (`whats_new::ICONS`). */
const WN_ICONS = {
  sparkle: '<path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z"/><path d="M19 15l.7 2 2 .7-2 .7-.7 2-.7-2-2-.7 2-.7z"/>',
  filter: '<path d="M4 5h16l-6 7.5V19l-4-2v-4.5z"/>',
  export: '<path d="M12 3v12M7.5 7.5L12 3l4.5 4.5"/><path d="M5 14v4.5A1.5 1.5 0 0 0 6.5 20h11a1.5 1.5 0 0 0 1.5-1.5V14"/>',
  layout: '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M3.5 9h17M7 13h10M7 16h6"/>',
  link: '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
  plugin: '<path d="M9 3v4M15 3v4"/><rect x="6" y="7" width="12" height="6" rx="2"/><path d="M12 13v3a4 4 0 0 1-4 4"/>',
  shield: '<path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z"/><path d="M9 12l2 2 4-4"/>',
  thread: '<path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/><path d="M8 13h8"/>',
  calendar: '<rect x="3.5" y="5" width="17" height="15" rx="2.5"/><path d="M3.5 10h17M8 3v4M16 3v4"/>',
  record: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="3.5" fill="currentColor"/>',
  fix: '<path d="M14.5 6.5a4 4 0 0 0 5 5L12 19a2.1 2.1 0 0 1-3-3z"/><path d="M14.5 6.5L17 4"/>',
}

// ── whats-new
const WhatsNew = {
  /** The dialog's heading: one version names it; several each have a header of their own. */
  heading: (entries) => entries.length === 1 ? `What's new in Echo ${entries[0].version}` : "What's new in Echo",
  /** Where an item's Show me goes: an echo:// link, or a window route. */
  target: (open) => !open ? null : /^echo:\/\//.test(open) ? { link: open } : { route: open },
  /** Open it by itself now? The app has entries to show and this page hasn't shown their newest yet. */
  due: (pending, shown) => !!(pending && pending.entries && pending.entries.length && pending.entries[0].version !== shown),
}
// end of the whats-new block

/** The dialog (`S.wn`: the entries it shows); opened by itself once when the app has something new (marked seen then). */
function whatsNewDialog() {
  const p = S.state && S.state.whats_new
  if (WhatsNew.due(p, S.wnShown)) { S.wnShown = p.entries[0].version; S.wn = p.entries; send({ cmd: 'whats_new_seen' }); whatsNewFocus() }
  const entries = S.wn
  if (!entries || !entries.length) return ''
  const one = entries.length === 1
  const item = (i) => `<li class="wn-item"><span class="wn-ic">${ICON(WN_ICONS[i.icon] || WN_ICONS.sparkle)}</span><div class="wn-t"><b>${esc(i.title)}</b><p>${esc(i.text)}</p></div>${i.open ? `<button class="btn primary wn-go" data-wn-go="${esc(i.open)}" aria-label="Show me: ${esc(i.title)}">Show me</button>` : ''}</li>`
  const body = entries.map((e) => `${one ? '' : `<h4 class="wn-ver">Echo ${esc(e.version)}</h4>`}<ul class="wn-items">${e.items.map(item).join('')}</ul>`).join('')
  return `<div class="td-modal"><div class="td-dialog wn" role="dialog" aria-modal="true" aria-labelledby="wn-h">
    <div class="wn-head"><h3 id="wn-h">${esc(WhatsNew.heading(entries))}</h3><button class="icon-btn" data-wn-close aria-label="Close">×</button></div>
    <div class="wn-body">${body}</div>
    <div class="td-dialog-a"><button class="btn primary" data-wn-close>Got it</button></div>
  </div></div>`
}

/** What's new again (⌘K, Settings → General): every version's entry. */
async function whatsNewOpen() {
  try { const r = await get('api/whats-new'); S.wn = r.entries || [] } catch { S.wn = null }
  render()
  whatsNewFocus()
}

/** The keyboard goes into the dialog (it is modal): to Got it, once it is drawn. */
const whatsNewFocus = () => setTimeout(() => { const b = document.querySelector('.wn .td-dialog-a [data-wn-close]'); if (b) b.focus() })

/** Show me: closes it and opens the item's place. */
function whatsNewGo(open) {
  const to = WhatsNew.target(open)
  S.wn = null
  if (to && to.link) { render(); return window.echo.openLink(to.link) }
  if (to) return go(to.route)
  return render()
}

document.addEventListener('keydown', (e) => {
  // The palette over it takes its own Escape first.
  if (e.key !== 'Escape' || !S.wn || PAL.open) return
  e.preventDefault(); e.stopImmediatePropagation()
  S.wn = null
  render()
}, true)
