// ── formatting ─────────────────────────────────────────────────────────────────────────────
const clock = (s) => {
  s = Math.max(0, Math.round(s))
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60
  const mm = String(m).padStart(h ? 2 : 1, '0'), ss = String(sec).padStart(2, '0')
  return h ? `${h}:${mm}:${ss}` : `${mm}:${ss}`
}
const hm = (d) => d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
const dayLabel = (d) => {
  const today = new Date(); today.setHours(0, 0, 0, 0)
  const that = new Date(d); that.setHours(0, 0, 0, 0)
  const diff = Math.round((today - that) / 86400000)
  if (diff === 0) return 'Today'
  if (diff === 1) return 'Yesterday'
  if (diff < 7) return that.toLocaleDateString([], { weekday: 'long' })
  return that.toLocaleDateString([], { day: 'numeric', month: 'long', year: today.getFullYear() === that.getFullYear() ? undefined : 'numeric' })
}
const STATUS = {
  recording: 'Recording', transcribing: 'Transcribing', notes: 'Minutes', transcript: 'Transcript', failed: 'Failed', audio: 'Audio',
}
const speaker = (seg) => (seg.speaker === 'me' || seg.speaker === 'mic') ? 'Me' : (seg.name || 'Remote')

// ── brand: the onetest logo (the full octopus character on its violet disc) ────────────────
const mark = (cls = '') => `<img class="logo ${cls}" src="assets/logo.png" alt="" aria-hidden="true">`

// ── data ───────────────────────────────────────────────────────────────────────────────────
async function loadState() {
  try { S.state = await get('api/state') } catch { /* app restarting */ }
}
/** The calls list, a page at a time from the app (`/api/calls`, newest first): a refresh reads only the first page
 *  again and puts it on top of what is loaded (new calls, changed statuses) — never every loaded call; scrolling to the
 *  end loads the next page; a search asks the app. `S.callsGen` drops an answer a newer search or reset made stale. */
const CALLS_PAGE = 60
/** A call folder's `YYYY-MM-DD_HHMMSS` stamp, or '' when its name has none (`calls::has_stamp`). */
const callStamp = (name) => /^\d{4}-\d\d-\d\d_\d{6}/.test(name) ? name.slice(0, 17) : ''
/** A call's place in the list, as the app orders it (`calls::order_key`): undated folders oldest, then by name. */
const callOrder = (name) => (callStamp(name) ? '1' : '0') + name
const callsQuery = () => S.query.trim() ? `&q=${encodeURIComponent(S.query.trim())}` : ''
async function loadCalls(reset) {
  const gen = S.callsGen = (S.callsGen || 0) + (reset ? 1 : 0)
  try {
    const r = await get(`api/calls?limit=${CALLS_PAGE}${callsQuery()}`)
    if (gen !== S.callsGen) return
    const have = reset ? [] : (S.calls || [])
    const last = r.calls.length ? r.calls[r.calls.length - 1].name : null
    // What is loaded beyond the first page stays (older calls; a call gone from the top is dropped with it).
    const rest = last && r.next ? have.filter((c) => callOrder(c.name) < callOrder(last)) : []
    S.calls = [...r.calls, ...rest]
    S.callsNext = rest.length ? S.callsNext : r.next
    S.callsTotal = r.total; S.error = null
  } catch (e) { S.error = String(e) }
  await loadUpcoming()
}
/** Today's meetings still to come (Calls → Coming up today), and the open one's prep. */
async function loadUpcoming() {
  try { S.up = await get('api/upcoming') } catch { /* app restarting */ }
  if (S.upSel) { try { const id = S.upSel, p = await get('api/prep/' + id); if (S.upSel === id) S.upPrep = p } catch { /* app restarting */ } }
}
async function loadMoreCalls() {
  if (!S.callsNext || S.callsMore) return
  S.callsMore = true
  const gen = S.callsGen, after = S.callsNext
  try {
    const r = await get(`api/calls?limit=${CALLS_PAGE}&before=${encodeURIComponent(after)}${callsQuery()}`)
    if (gen === S.callsGen && after === S.callsNext) { const seen = new Set((S.calls || []).map((c) => c.name)); S.calls = [...(S.calls || []), ...r.calls.filter((c) => !seen.has(c.name))]; S.callsNext = r.next; S.callsTotal = r.total }
  } catch { /* the next scroll asks again */ }
  S.callsMore = false
  render()
}
async function loadProjects() {
  try { S.pj.data = await get('api/projects') } catch { /* app restarting */ }
}
async function loadToday() {
  try { S.today = await get('api/today') } catch { /* app restarting */ }
}
async function loadTasks() {
  try { S.tk.data = await get('api/tasks') } catch { /* app restarting */ }
}
/** The work of the project a Tasks card is in (its details panel reads it). */
async function loadWork() {
  const id = S.pj.sel
  if (!id) return
  try { const w = await get('api/project/' + encodeURIComponent(id) + '/work'); if (S.pj.sel === id) S.pj.work = w } catch { /* app restarting */ }
}
/** The People tab's data: the open thread's or client's (`people.rs`). */
async function loadPeople() {
  const url = S.pj.client ? `api/client/${encodeURIComponent(S.pj.client)}/people` : S.pj.sel ? `api/project/${encodeURIComponent(S.pj.sel)}/people` : null
  if (!url) { S.pj.people = null; return }
  try { const d = await get(url); d.url = url; S.pj.people = d } catch { /* app restarting */ }
}
/** The Brain as one thread sees it: its topics and terms (shared layer + its client's), for its
 *  Profile and Brief. */
async function loadThreadBrain() {
  const id = S.pj.sel
  if (!id) return
  try { const d = await get(`api/brain?thread=${encodeURIComponent(id)}`); d.thread = id; if (S.pj.sel === id) S.pj.tbrain = d; noteEditErrors(d) } catch { /* no Brain in this build */ }
}
async function loadFound() {
  try { S.pj.foundData = await get('api/found') } catch { /* app restarting */ }
}
async function loadProject() {
  if (S.pj.found) return loadFound()
  if (S.pj.client) {
    const id = S.pj.client
    try { const d = await get('api/client/' + encodeURIComponent(id)); if (S.pj.client === id) S.pj.cdata = d } catch { /* app restarting */ }
    if (S.pj.ctab === 'people') await loadPeople()
    return
  }
  const id = S.pj.sel
  if (!id) { S.pj.detail = null; return }
  try { const d = await get('api/project/' + encodeURIComponent(id)); if (S.pj.sel === id) S.pj.detail = d } catch { /* app restarting */ }
  if (S.pj.tab === 'people') await loadPeople()
  if ((S.pj.tab === 'profile' || S.pj.tab === 'brief') && (!S.pj.tbrain || S.pj.tbrain.thread !== S.pj.sel)) await loadThreadBrain()
  if (S.pj.tab === 'profile') {
    try { const d = await get('api/project/' + encodeURIComponent(id) + '/series'); if (S.pj.sel === id) S.pj.series = { id, ...d } } catch { /* app restarting */ }
  }
  if (S.pj.tab === 'work') {
    try { const w = await get('api/project/' + encodeURIComponent(id) + '/work'); if (S.pj.sel === id) S.pj.work = w } catch { /* app restarting */ }
  }
}
async function loadAgenda() {
  try { S.agenda = await get('api/agenda') } catch { S.agenda = [] }
}
async function loadLayout() {
  const before = Layout.toJSON()
  let saved = null
  try { saved = await get('api/layout') } catch { /* the default layout */ }
  // A panel opened while it was asked for (the route's Vocabulary, a click) stays open over the saved layout.
  const now = Layout.toJSON()
  Layout.load(saved)
  Layout.prune(PANEL_VIEWS)
  Layout.started()
  for (const a of Layout.AREAS) if (now[a].open && !before[a].open && now[a].active) Layout.show(now[a].active, PANEL_VIEWS)
}
let callSeq = 0
/** The selected call answered 404 (an old link, a deleted call): not polled every tick (#150), and Calls leaves its
 *  error page. `tick` asks for it about every half minute, so a restored one loads again. */
const callGone = () => !!(S.selected && S.call && S.call.missing && !(S.calls || []).some((c) => c.name === S.selected))
async function loadCall(name, retried = false) {
  const mine = ++callSeq, epoch = NotesSync.epoch(name)
  try {
    const c = await get('api/call/' + encodeURIComponent(name))
    // Another call was chosen while this answer came (it is not shown, nor its notes edited as that
    // one), or a newer fetch was started: only the newest answer counts.
    if (S.selected !== name || mine !== callSeq) return
    // Renamed meanwhile (the notes run retitles the folder): the app answers with the call under its stamp.
    // (The URL follows in `doc()`, the one place that writes it for a call.)
    if (c.name && c.name !== name) S.selected = c.name
    S.call = c; S.movedShown = !!(c.changes || []).length
    // Its minutes are read now (Today's "new" goes): once per call and minutes a session.
    if (c.notes && S.seenMinutes !== `${c.name}:${(c.notes_run && c.notes_run.finished_at) || ''}`) { S.seenMinutes = `${c.name}:${(c.notes_run && c.notes_run.finished_at) || ''}`; send({ cmd: 'minutes_seen', call: c.name }) }
    // A file that can't be read isn't "empty": the editor stays away from it (`notesTab` says why).
    if (c.my_notes_error == null) NotesSync.seen(S.call.name, S.call.my_notes || '', epoch)
    syncEditor()
  } catch (e) {
    if (S.selected !== name || mine !== callSeq) return
    S.call = { error: String(e) }
    // No such call (the app follows a renamed folder by its stamp itself): not polled again, and Calls leaves its
    // error page (#150) — unless a fresh list has it, being written a moment ago (a new recording): asked once more.
    if (/^404 /.test(e.message || '')) {
      // The list is asked once, at the first 404 (a later half-minute retry doesn't reload it).
      if (!retried) await loadCalls()
      if (S.selected !== name || mine !== callSeq) return
      if (!retried && (S.calls || []).some((c) => c.name === name)) return loadCall(name, true)
      // Missing: dropped from the loaded list (a stale entry would keep it polled), asked for now and then only
      // (`tick`) and as soon as a fresh list has it again — see `callGone`.
      S.call.missing = true; S.movedShown = false
      S.calls = (S.calls || []).filter((c) => c.name !== name)
    }
  }
}

// ── routing: #calls, #call/<name>/<tab>, #today ────────────────────────────────────────────
/** The hash as text: decoded, or as it is when it holds a broken %-sequence (an old or hand-typed link). */
function hashText() { try { return decodeURIComponent(location.hash) } catch { return location.hash } }
function route() {
  const h = hashText().slice(1)
  // A menu of the page it leaves doesn't follow to the next one (nor takes the next Escape).
  S.liveMenu = false; S.pj.menu = false; S.pj.picker = false; S.pj.evPicker = null; S.pj.fieldsMenu = false; S.pj.colMenu = null; S.pj.viewMenu = false
  // An echo:// link the window was opened at (the system, `echoctl app open --link`): routed where it points.
  // The page opened at a link goes where the link points, or, for one Echo can't open, nowhere: no call is opened
  // for it at start-up (#168).
  if (h.startsWith('link=')) { history.replaceState(null, '', '#'); S.view = 'calls'; S.fromLink = true; setTimeout(() => window.echo.openLink(h.slice(5))); return }
  // A plugin's activity (`ctx.activities.add`): `#<plugin>:<id>` and anything after a slash is its own.
  const pa = ACTIVITIES.find((a) => a.plugin && (h === a.id || h.startsWith(a.id + '/')))
  if (pa) { S.view = pa.id; S.plRoute = h.slice(pa.id.length + 1); S.plRoutes = { ...(S.plRoutes || {}), [pa.id]: S.plRoute }; return }
  const cm = h.match(/^calendar(?:\/(day|week))?$/)
  if (cm) { S.view = 'calendar'; if (cm[1]) S.cal.mode = cm[1]; return }
  // settings/plugins/<id>: that plugin picked (an Inbox update item opens it).
  const sm = h.match(/^settings(?:\/([a-z0-9:-]+))?(?:\/([a-z0-9-]+))?$/)
  if (sm && sm[2] && sm[1] !== 'plugins') sm[2] = undefined // only Plugins picks one of its own
  // The vocabulary was a Settings section: its old link opens its panel, over the calls.
  if (sm && sm[1] === 'vocabulary') { S.view = 'calls'; setTimeout(() => { Layout.show('vocabulary', panelViews()); layoutChanged() }); return }
  if (sm) { S.view = 'settings'; if (sm[1]) S.setSection = sm[1]; if (sm[1] === 'plugins' && sm[2]) { Object.assign(S, { plSel: sm[2], plTab: 'installed', plSheet: null, plQuery: '', plMenu: null }); history.replaceState(null, '', '#settings/plugins') } return }
  if (h === 'tasks') { S.view = 'tasks'; return }
  // The window opens on Today.
  if (h === 'today' || h === '') { S.view = 'today'; return }
  // Knowledge: Recall's tabs and the plugins' (`knowledge.tab`, `#brain/<plugin>:<id>/<its route>`).
  const bm = h.match(/^brain(?:\/([\w:-]+)(?:\/(.*))?)?$/)
  if (bm && !brainOn() && !knowledgeTabs().length) { S.view = 'calls'; return }
  if (bm) { S.view = 'brain'; if (bm[1]) S.br.tab = bm[1]; S.plRoute = bm[2] || ''; return }
  if (h === 'found') { S.view = 'projects'; S.pj.found = true; S.pj.client = null; S.pj.sel = null; S.pj.detail = null; return }
  const cm2 = h.match(/^client\/([a-z0-9-]+)(?:\/(\w+))?$/)
  if (cm2) {
    S.view = 'projects'; S.pj.found = false
    if (S.pj.client !== cm2[1]) { S.pj.cdata = null; S.pj.briefEdit = null; S.pj.person = null }
    S.pj.client = cm2[1]; S.pj.ctab = cm2[2] || S.pj.ctab || 'overview'; S.pj.sel = null; S.pj.detail = null; S.pj.adding = null; return
  }
  const pm = h.match(/^projects?(?:\/([a-z0-9-]+))?(?:\/(\w+))?$/)
  if (pm) {
    S.view = 'projects'
    S.pj.found = false
    S.pj.client = null
    if (pm[1] && S.pj.sel !== pm[1]) { S.pj.adding = null; S.pj.pfEdit = null; S.pj.menu = false; S.pj.sel = pm[1]; S.pj.detail = null; S.pj.work = null; S.pj.person = null; S.pj.ppAdding = false; S.pj.node = S.pj.wantNode || null; S.pj.wantNode = null; S.pj.briefEdit = null; S.pj.confirmDelete = false }
    if (pm[2]) S.pj.tab = pm[2]
    return
  }
  S.view = 'calls'
  // A meeting still to come: its prep (echo://prep, Coming up today).
  const um = h.match(/^upcoming\/([0-9a-f]{1,32})$/)
  if (um) {
    if (S.upSel !== um[1]) { S.upSel = um[1]; S.upPrep = null; S.selected = null; S.call = null; setTimeout(() => loadUpcoming().then(render)) }
    return
  }
  S.upSel = null
  // Calls after a call that isn't there: the newest call, as a fresh load opens (#150).
  if (h === 'calls' && callGone()) {
    const gone = S.selected, next = (S.calls || []).find((c) => c.name !== gone)
    commitPendingNotes(); S.selected = next ? next.name : null; S.call = null; S.confirmDelete = false; S.find = ''
    // The hashchange handler loads it (S.call is null), once.
    if (S.selected) { history.replaceState(null, '', `#call/${S.selected}/minutes`); S.tab = 'minutes' }
  }
  const m = h.match(/^call\/([^/]+)(?:\/(\w+))?$/)
  if (m) {
    // Another call: a note typed or edited on this one goes into its notes first.
    if (S.selected !== m[1]) { commitPendingNotes(); S.selected = m[1]; S.call = null; S.confirmDelete = false; S.find = '' }
    if (m[2]) S.tab = m[2]
  }
}
const go = (hash) => { if (location.hash.slice(1) !== hash) location.hash = hash; else render() }
/** An activity, where the user left it (the open call and tab; the calendar's day or week). */
const goActivity = (id) => go(ACTIVITIES.some((a) => a.id === id && a.plugin) ? ((S.plRoutes || {})[id] ? `${id}/${S.plRoutes[id]}` : id) : id === 'calendar' ? `calendar/${S.cal.mode}` : id === 'settings' ? `settings/${S.setSection}`
  : id === 'tasks' ? 'tasks' : id === 'today' ? 'today' : id === 'brain' ? `brain/${S.br.tab}`
  : id === 'projects' ? (S.pj.found ? 'found' : S.pj.client ? `client/${S.pj.client}/${S.pj.ctab || 'overview'}` : S.pj.sel ? `project/${S.pj.sel}/${S.pj.tab}` : 'projects') : (S.selected && !callGone() ? `call/${S.selected}/${S.tab}` : 'calls'))

