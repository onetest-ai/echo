// ── Settings: every setting in one place (the tray keeps quick toggles). Plain values save at once
// (`settings_set`, the value shown, never a toggle of a stale one); flows with a dialog, a sign-in or
// a download run as the tray's own actions (`settings_action`). ────────────────────────────────
const SETTINGS_SECTIONS = [
  ['general', 'General'], ['recording', 'Recording'], ['transcription', 'Transcription'], ['ai', 'AI & minutes'], ['templates', 'Templates'],
  ['calendar', 'Calendar & chat'], ['brain', 'Knowledge'], ['me', 'About me'], ['deliver', 'Sends to you'], ['plugins', 'Plugins'], ['layout', 'Layout'], ['health', 'Health'],
]
// ── settings search: which sections a search keeps, by their names and by their settings' own labels (#103) —
// the rows' titles (`<b>` of setRow) and the cards' heads (`<h3>`) of a section as drawn; not descriptions or badges.
const SettingsSearch = (() => {
  const ENT = { amp: '&', lt: '<', gt: '>', quot: '"', '#39': "'" }
  const norm = (q) => String(q || '').trim().toLowerCase()
  /** HTML as the user reads it: badges (`<span class="tag …">`) and tags out, entities back. */
  const text = (html) => String(html || '').replace(/<span class="tag[^"]*">[\s\S]*?<\/span>/g, '').replace(/<[^>]*>/g, ' ')
    .replace(/&(amp|lt|gt|quot|#39);/g, (_, e) => ENT[e]).replace(/\s+/g, ' ').trim()
  /** The labels in a section's HTML. */
  const labels = (html) => [...String(html || '').matchAll(/<(b|h3)>([\s\S]*?)<\/\1>/g)].map((m) => text(m[2])).filter(Boolean)
  /** [{id, label, html}] → the sections `q` keeps, each with its labels that match (none when only its name does). */
  function match(sections, q) {
    q = norm(q)
    return sections.map((s) => ({ id: s.id, label: s.label, hits: q ? labels(s.html).filter((l) => l.toLowerCase().includes(q)) : [] }))
      .filter((s) => !q || s.label.toLowerCase().includes(q) || s.hits.length)
  }
  /** Does the search pick out a row with this title? */
  const hit = (title, q) => !!norm(q) && text(title).toLowerCase().includes(norm(q))
  return { match, hit }
})()
// end of the settings-search block
/** The search the rows being drawn are picked out by (set only while Settings draws its section). */
let setHitQ = ''
/** Save the profile as edited; shown rendered again once the app has it. */
function saveMeProfile() {
  if (S.meEdit == null) return
  const text = S.meEdit
  send({ cmd: 'settings_set', key: 'me_profile', value: text })
  if (S.settings) S.settings.me_profile = { text, html: S.settings.me_profile ? S.settings.me_profile.html : '' }
  S.meEdit = null
  render()
  setTimeout(() => loadSettings().then(render), 400)
}
async function loadSettings() {
  try { S.settings = await get('api/settings') } catch { /* app restarting */ }
}
/** Required permissions not granted, and Claude Code not ready for Echo: Settings' badge. */
const settingsWarnings = () => ((S.settings && S.settings.health) || []).filter((c) => c.required && c.status !== 'granted').length + claudeProblems((S.settings || {}).claude_code)
/** Claude Code missing, not starting, or without Echo's plugin where Echo's sessions run (`claude_setup::check`). */
const claudeProblems = (cc) => !cc || cc.checking && !('plugin' in cc) ? 0 : (!cc.path || cc.error ? 1 : 0) + (cc.path && !cc.error && !cc.plugin ? 1 : 0)
/** The Claude Code plugin's control: installed (as Claude Code lists it), or Connect. */
function claudePluginCtl(cc) {
  if (!cc || !('plugin' in cc)) return '<span class="set-sub">Checking…</span>'
  if (!cc.path || cc.error) return '<span class="set-sub">Needs Claude Code — see Health</span>'
  return cc.plugin ? '<span class="set-ok">Installed ✓</span>' : actBtn('connect_claude', 'Connect')
}
/** The check runs in the app's background: read the settings again shortly (one timer, however often drawn). */
let claudeTimer = null
function claudeRecheckSoon() { if (!claudeTimer) claudeTimer = setTimeout(() => { claudeTimer = null; loadSettings().then(render) }, 2500) }
/** Settings → Health's Claude Code rows: it runs, a terminal finds it, Echo's plugin is in. */
function claudeRows(cc) {
  const ok = '<span class="set-ok">✓</span>', bad = '<span class="warn">✗</span>', warn = '<span class="warn">⚠</span>'
  if (!('plugin' in cc)) { claudeRecheckSoon(); return [setRow('<span class="set-sub">…</span> Claude Code', 'Checking that it runs, that a terminal finds it, and that it has Echo\'s plugin…', '')] }
  if (cc.checking) claudeRecheckSoon()
  const get = actBtn('open_settings_url', 'Get Claude Code', 'https://claude.com/claude-code')
  if (!cc.path) return [setRow(`${bad} Claude Code`, 'Not installed. Echo needs it to write minutes and for Claude in the window. Install it, sign in once in a terminal (<code>claude</code>), then Check again.', get)]
  if (cc.error) return [setRow(`${bad} Claude Code`, `Found at <code>${esc(cc.path)}</code>, but it doesn't start: ${esc(cc.error)}. Reinstall it, then Check again.`, get)]
  const rows = [setRow(`${ok} Claude Code`, `${esc(cc.version)} · <code>${esc(cc.path)}</code>`, '')]
  const t = cc.terminal || {}
  if (t.ok === true) rows.push(setRow(`${ok} In a terminal`, `<code>claude</code> runs there${t.found && t.found !== cc.path ? ` (<code>${esc(t.found)}</code>)` : ''}.`, ''))
  else if (t.ok === false) rows.push(setRow(`${warn} In a terminal`, `A terminal can't find <code>claude</code>: <code>${esc(t.dir || '')}</code> isn't on your PATH. Echo still runs it; you can't type <code>claude</code> yet.${t.rc === null && !/Windows/.test(navigator.userAgent) ? ` Add that folder to your shell's PATH yourself (${esc(String(t.shell || '').split('/').pop())}).` : ''}`, t.rc === null && !/Windows/.test(navigator.userAgent) ? '' : actBtn('claude_path', 'Add to PATH')))
  else rows.push(setRow(`<span class="set-sub">?</span> In a terminal`, `Couldn't check: ${esc(t.why || 'no answer')}.`, ''))
  const where = (cc.installs || []).map((i) => i.scope === 'user' ? 'every session' : esc(String(i.project || '').split(/[\\/]/).pop() || i.scope)).join(', ')
  rows.push(cc.plugin ? setRow(`${ok} Echo plugin`, `Claude Code lists it (${where}).`, '')
    : setRow(`${bad} Echo plugin`, `Claude Code doesn't list Echo's plugin where Echo's sessions run${where ? ` (only: ${where})` : ''}: no /echo:process-call, record or doctor there.`, actBtn('connect_claude', 'Connect')))
  return rows
}
const setRow = (title, sub, control, id = '') => `<div class="set-row${SettingsSearch.hit(title, setHitQ) ? ' hit' : ''}"${id ? ` id="set-${id}"` : ''}><div><b>${title}</b>${sub ? `<div class="set-sub">${sub}</div>` : ''}</div><div class="set-ctl">${control}</div></div>`
const setCard = (head, rows) => `<section class="set-sec">${head ? `<h3>${esc(head)}</h3>` : ''}<div class="set-card">${rows.join('')}</div></section>`
const sw = (key, on, label) => `<button class="sw${on ? ' on' : ''}" role="switch" aria-checked="${!!on}" aria-label="${esc(label)}" data-set="${key}" data-val="${!on}"></button>`
const seg = (key, cur, opts) => `<div class="seg" role="group">${opts.map(([v, l]) => `<button data-set="${key}" data-val='${esc(JSON.stringify(v))}' aria-pressed="${cur === v}">${esc(l)}</button>`).join('')}</div>`
const actBtn = (action, label, arg = '', primary = false) => `<button class="btn${primary ? ' primary' : ''}" data-act="${action}"${arg ? ` data-arg="${esc(arg)}"` : ''}>${esc(label)}</button>`
const LANG_LIST = ['en', 'uk', 'ru', 'es', 'de', 'fr', 'it', 'pl', 'pt', 'nl', 'cs', 'ro', 'tr', 'he', 'ar', 'hi', 'ja', 'zh', 'ko']

function settingsView() {
  const st = S.settings
  const own = Plugins.sections.find((x) => x.id === S.setSection)
  let sec = own ? own.id : SETTINGS_SECTIONS.some(([id]) => id === S.setSection) ? S.setSection : 'general'
  const q = (S.setQuery || '').trim().toLowerCase()
  // Searched: each section by its name and its settings' labels, as drawn now. By name only: Templates, Plugins and
  // Sends to you (drawing loads their lists), Knowledge (it draws plugins' panels) and Health (it re-checks Claude Code).
  const drawn = (id) => {
    if (!q || !st || st.config_error || ['templates', 'plugins', 'deliver', 'brain', 'health'].includes(id)) return ''
    try { return SETTINGS_BODY[id](st) } catch { return '' }
  }
  const found = SettingsSearch.match(SETTINGS_SECTIONS.map(([id, label]) => ({ id, label, html: drawn(id) })), q)
  // The section shown is one the search kept: the first, when the one open isn't.
  if (!own && q && found.length && !found.some((s) => s.id === sec)) sec = found[0].id
  const nav = found.map(({ id, label }) => {
    const warn = id === 'health' && settingsWarnings() ? `<span class="set-warn">⚠ ${settingsWarnings()}</span>` : ''
    return `<button class="set-nav${id === sec ? ' on' : ''}" data-setsec="${id}">${esc(label)}${warn}</button>`
  }).join('')
  // Plugins' own sections (`ctx.settings.section`), under their own heading.
  const mine = Plugins.sections.filter((x) => !q || x.label.toLowerCase().includes(q))
  const pnav = mine.length ? `<div class="set-grp">From plugins</div>` + mine.map((x) => {
    let badge = ''
    try { badge = x.badge ? x.badge() : '' } catch { /* none */ }
    return `<button class="set-nav${x.id === sec ? ' on' : ''}" data-setsec="${esc(x.id)}">${x.icon || ''}${esc(x.label)}${badge !== '' && badge != null ? `<span class="set-n">${esc(String(badge))}</span>` : ''}</button>`
  }).join('') : ''
  const none = q && !found.length && !mine.length ? `<p class="set-sub set-none">Nothing matches “${esc(S.setQuery.trim())}”.</p>` : ''
  setHitQ = q
  const body = own ? (() => { try { return own.render() } catch (e) { return `<p class="error">${esc(own.label)}: ${esc(String((e && e.message) || e))}</p>` } })()
    : !st ? `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
    : st.config_error ? `<p class="error">Echo's settings (config.json) can't be read: ${esc(st.config_error)}</p>`
    // A section that fails says so in its place; the rest of the window keeps working.
    : (() => { try { return (SETTINGS_BODY[sec] || SETTINGS_BODY.general)(st) } catch (e) { console.error(e); return `<p class="error">This section couldn't be shown: ${esc(String((e && e.message) || e))}</p>` } })()
  setHitQ = ''
  if (own) return `<div class="settings"><aside class="set-side"><h1>Settings</h1><input class="search" id="set-q" placeholder="Search settings" value="${esc(S.setQuery || '')}" aria-label="Search settings">${nav}${pnav}${none}</aside>
    <div class="set-main plug-main">${body}</div></div>`
  return `<div class="settings"><aside class="set-side"><h1>Settings</h1><input class="search" id="set-q" placeholder="Search settings" value="${esc(S.setQuery || '')}" aria-label="Search settings">${nav}${pnav}${none}</aside>
    <div class="set-main${sec === 'plugins' ? ' pl-main' : sec === 'templates' ? ' tp-main' : ''}">${body}<p class="set-foot">Saved as you change it.</p></div></div>`
}

