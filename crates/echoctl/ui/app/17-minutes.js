// ── Minutes: the page as it would be sent, in a sandboxed frame (`/minutes/<call>`) ───────────
const SHOW_FILE = /Windows/.test(navigator.userAgent) ? 'Show in Explorer' : 'Show in Finder'
function minutesTab(c) {
  const run = c.notes_run
  const busy = run && (run.state === 'queued' || run.state === 'running')
  // The first minutes being written: their box until they are done (the file appears mid-run).
  if (!c.notes || (busy && !run.had_minutes)) return minutesHint(c, 'No minutes yet', 'The transcript is ready.')
  const frame = `<iframe id="minutes-frame" sandbox="allow-scripts" referrerpolicy="no-referrer" title="Minutes" src="minutes/${encodeURIComponent(c.name)}?v=${encodeURIComponent(c.minutes_rev || '')}${Theme.chosen() ? `&theme=${Theme.chosen()}` : ''}"></iframe>`
  return { bar: minutesBar(c), frame }
}

/** Above the page: an update going on or one that didn't finish, notes changed since, and the file. */
/** Write now: a run waiting for another call's recording or transcript that may start anyway
 *  (`may_start`: Claude Code as configured, not on a model on this computer). */
const notesNowBtn = (run) => {
  return run && run.state === 'queued' && run.wait === 'call' && run.may_start && !run.now ? '<button class="btn ghost" data-cmd="notes-now" title="Start them now; the other call keeps recording, and both share this computer">Write now</button>' : ''
}
function minutesBar(c) {
  const run = c.notes_run
  const log = c.has_log ? '<button class="btn ghost" data-cmd="show-log">Show log</button>' : ''
  let status = ''
  if (run && run.state === 'running') status = `<span class="grow">Updating the minutes…</span>${log}`
  else if (run && run.state === 'queued') status = `<span class="grow">${esc(run.message || 'The update waits its turn.')}</span>${notesNowBtn(run)}`
  else if (run && (run.state === 'failed' || run.state === 'needs_login')) {
    const p = run.state === 'needs_login' && profileOf(run.profile)
    const signin = p && p.kind === 'claude' ? `<button class="btn ghost" data-signin="${esc(p.id)}">Sign in…</button>` : ''
    status = `<span class="grow warn">The last update didn't finish: ${esc(run.message || '')}</span>${signin}<button class="btn ghost" data-cmd="write-notes">Try again</button>${log}`
  } else if (run && run.state === 'skipped') status = `<span class="grow">${esc(run.message || 'The last update wrote nothing.')}</span>`
  // Opening or showing the page is in the call's ⋯ menu: this bar is only for what is going on.
  return status
}

/** No minutes (or no action items) yet: when Echo writes them by itself, or why not, and Write
 *  minutes — Claude Code run from the app (the same process-call skill a session runs); the run's
 *  state and log come with the call. */
/** Whole minutes until `at` (at least 1 while it is ahead). */
const minutesLeft = (at) => Math.max(1, Math.ceil((at - Date.now()) / 60000))
/** A call's automatic minutes still ahead (the list's and the status bar's clock). */
const minutesAhead = (c) => c.minutes_at && new Date(c.minutes_at) > Date.now() ? new Date(c.minutes_at) : null

function minutesHint(c, heading, lead) {
  const n = (S.state && S.state.notes) || {}
  const run = c.notes_run
  const auto = c.auto || {}
  const cmd = `/echo:process-call ${c.name}`
  const log = c.has_log ? '<button class="btn ghost" data-cmd="show-log">Show log</button>' : ''
  const box = (h, p, buttons, busy) => `<div class="empty" style="height:auto;padding:40px 0"><div>${mark(busy ? 'busy' : '')}<h3>${esc(h)}</h3>
    ${p}${buttons ? `<div class="notes-actions">${buttons}</div>` : ''}</div></div>`
  if (run && run.state === 'queued') {
    return box('Minutes are waiting their turn', `<p>${esc(run.message || 'They start as soon as the model is free.')}</p><p class="alt">With <b>${esc(run.model || '')}</b>. You can close this window.</p>`, notesNowBtn(run), true)
  }
  if (run && run.state === 'running') {
    return box('Writing minutes…', `<p>Claude Code is writing the minutes and action items with <b>${esc(run.model || n.model || 'sonnet')}</b>. It takes a few minutes; you can close this window.</p>`, log, true)
  }
  if (c.status === 'recording' || c.status === 'transcribing') {
    // Promised only when they are due; whether Claude Code is there only once the state says so.
    const known = !!(S.state && S.state.notes)
    const when = known && !n.claude ? "Minutes are written by Claude Code, which isn't installed on this computer: install it from claude.com/claude-code to get them."
      : known && !n.plugin ? "Echo's plugin for Claude Code wasn't found: menu → Claude Code → Connect Claude Code, then write them once the call is transcribed."
      : auto.due === 'error' ? `Automatic minutes can't start${auto.reason ? `: ${auto.reason.replace(/^(the )?automatic minutes (can't|couldn't) start: /i, '')}` : ''}. Write them once the call is transcribed.`
      : auto.due === 'not_ready' && !ai().error ? 'Echo writes them 15 minutes after the call. Add your own notes in the Notes tab meanwhile: the minutes are written around them.'
      : 'Write them once the call is transcribed.'
    return box('Minutes come after the call', `<p>${esc(when)}</p>`, '')
  }
  if (!run && auto.due === 'now') return box('Starting the minutes…', '<p>Echo is starting them now.</p>', '', true)
  // A Claude Code session holds the call (#43): who, since when — and Write minutes takes it over.
  if (!(run && ['queued', 'running'].includes(run.state)) && auto.due === 'tried' && auto.held_by) {
    const since = auto.held_by.at ? new Date(auto.held_by.at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : ''
    return box('A Claude Code session is writing the minutes', `<p>Held by a Claude Code session (${auto.held_by.pid ? `process ${esc(String(auto.held_by.pid))}` : 'on this computer'}${since ? `, since ${esc(since)}` : ''}). If nothing comes, write them here instead.</p>`, '<button class="btn primary" data-cmd="write-notes">Write minutes</button>', true)
  }
  // A run refused for a Claude profile's sign-in (the built-in profile signs in in Terminal), and
  // that profile's sign-in now: signed in since (or a model refused it although it looks signed in).
  const signinOf = run && run.state === 'needs_login' && [profileOf(run.profile)].find((x) => x && x.kind === 'claude')
  const now = signinOf && signinOf.signin
  const again = now && now.state === 'signed_in'
  const why = {
    needs_login: again ? ['Ready to try again', run.message ? `The last try said: ${run.message}` : '']
      : [now && now.state === 'other_account' ? 'These minutes need another account' : 'Claude Code needs you to sign in', run && run.message],
    failed: ["Couldn't write the minutes", run && run.message],
    skipped: ['No minutes for this call', run && run.message],
  }[run && run.state]
  let label = 'Write minutes'
  // The countdown (#42): a ring of the time left, why it moved, and Not yet.
  let ring = '', moved = '', later = ''
  if (why) [heading, lead, label] = [why[0], why[1] || lead, run.state !== 'skipped' ? 'Try again' : label]
  else if (auto.due === 'at' && auto.at) {
    const at = new Date(auto.at), left = minutesLeft(at)
    ;[heading, lead, label] = [`Minutes in ${left} min (${hm(at)})`, 'Echo writes them 15 minutes after the call or your last note, so what you add in My notes goes in.', 'Write now']
    // Against the longest wait seen for this time (15 minutes, or more after Not yet): it empties as it comes.
    const span = S.ringSpan && S.ringSpan.at === auto.at ? S.ringSpan.ms : Math.max(15 * 60000, at - Date.now())
    S.ringSpan = { at: auto.at, ms: span }
    const share = Math.max(0, Math.min(1, (at - Date.now()) / span))
    ring = `<div class="mn-ring" style="--left:${Math.round(share * 100)}%" aria-hidden="true"><span><b>${left}</b><small>min</small></span></div>`
    const m = auto.moved || {}
    moved = m.by === 'later' ? '<p class="mn-moved">Moved: you said not yet.</p>'
      : m.by === 'notes' && m.at ? `<p class="mn-moved">Moved: you edited your notes at ${esc(hm(new Date(m.at)))}.</p>` : ''
    later = `<span class="mn-later"><button class="btn ghost" data-cmd="later-menu" aria-haspopup="menu" aria-expanded="${!!S.laterOpen}">Not yet ▾</button>${S.laterOpen ? `<div class="menu" role="menu"><button class="menu-item" role="menuitem" data-cmd="later-15">15 minutes more</button><button class="menu-item" role="menuitem" data-cmd="later-manual">Until I say — I'll click Write minutes</button></div>` : ''}</span>`
  } else if (auto.due === 'held') {
    ;[heading, lead] = ['Minutes when you say', "You asked for these later: Echo won't write them by itself."]
    later = '<button class="btn ghost" data-cmd="later-clear">Write them automatically again</button>'
  }
  else if (auto.due === 'gate') [heading, lead] = ['No minutes written automatically', `The call is too thin for them (${auto.reason || 'too short'}). Write them anyway if you want them.`]
  else if (auto.due === 'error') [heading, lead] = ["Automatic minutes can't start", (auto.reason || "Echo's settings can't be read").replace(/^(the )?automatic minutes (can't|couldn't) start: /i, '')]
  const session = `<p class="alt">Or in a Claude Code session: <code>${esc(cmd)}</code> <button class="btn ghost" data-copy="${esc(cmd)}">Copy</button></p>`
  if (!n.claude) {
    return box(heading, `<p>${esc(lead)}</p><p>Minutes and action items are written by Claude Code. Install it from <a href="https://claude.com/claude-code" data-ext="1">claude.com/claude-code</a>, then come back here.</p>`, log)
  }
  if (!n.plugin) {
    return box(heading, `<p>${esc(lead)}</p><p>Connect Claude Code first: menu → Claude Code → Connect Claude Code.</p>`, log)
  }
  // The call's own profile; none when it can't be read (the run would fail: say why, not the default).
  const live = c.live || {}, picked = S.notesProfile && S.notesProfile.call === c.name
  const pid = picked ? S.notesProfile.id : (live.profile || (live.profile_error ? '' : ai().default))
  const p = profileOf(pid)
  // The model that writes them, as Echo decides it (`Profile::notes_model`), for every kind.
  const model = (p && p.notes_model_label) || n.model || 'sonnet'
  // The profile these minutes need signed in: the call's profile now (picked here or in the Live
  // tab); the one the last run was refused for only while it still is (or none can be read).
  // Signed in now: Try again (or sign in again, if a model keeps refusing it).
  const target = signinOf && (!p || p.id === signinOf.id) ? signinOf : p
  const sign = signinNote(target) || (again && target === signinOf
    ? `<div class="signin"><span class="signin-text">${esc(signinOf.name)} is signed in as ${esc(now.account)} now — click Try again.</span><span class="signin-acts"><button class="btn ghost" data-signin="${esc(signinOf.id)}">Sign in again…</button></span></div>` : '')
  const where = ai().error ? `The AI settings can't be read: ${esc(ai().error)}`
    : !p && !picked && live.profile_error ? `This call's model can't be read: ${esc(live.profile_error)}`
    : !p ? (pid === '!unreadable' ? 'No model could be chosen when this call started — pick one.' : `The model this call was set to (${esc(pid || '?')}) no longer exists — pick another.`)
    : p.kind !== 'system'
      ? `Claude Code writes them with <b>${esc(p.name)}</b> (${esc(model)}), ${esc(profileWhere(p))}.`
      : `Claude Code writes them with <b>${esc(model)}</b>, as it is set up (menu → AI for calls → Minutes model, while it is the profile new calls use).`
  const pick = ai().profiles.length > 1 || (ai().profiles.length && !p) ? `<label class="notes-pick">Model <select data-notesprofile aria-label="Model for these minutes">${p ? '' : '<option value="" selected>Choose…</option>'}${profileOptions(pid)}</select></label>` : ''
  return box(heading, `${ring}<p>${esc(lead)}</p>${moved}<p class="alt">${where}</p>${sign}`,
    `${pick}<button class="btn primary" data-cmd="write-notes">${label}</button>${later}${log}`) + session
}

// ── What moved: a view of the right panel (was beside the minutes; the minutes now take the tab's width) ──
const MOVED_ICON = ICON('<path d="M4 7h11M11 3l4 4-4 4"/><path d="M20 17H9M13 13l-4 4 4 4"/>')
function movedPanel() {
  if (!S.selected) return '<p class="panel-empty">Open a call to see what it changed in its threads.</p>'
  const c = S.call
  if (!c || c.error) return '<div class="skeleton" style="padding:12px"><i></i><i></i></div>'
  return `<div class="mv">${callChanges(c)}</div>`
}
// Shown for a call that is in a thread (each thread says what it changed, or why it hasn't read the call yet);
// the tab carries the count of changes. A call in no thread has no such view, as Also discussed without the Brain.
PANEL_VIEWS.push({ id: 'moved', label: 'What moved', icon: MOVED_ICON, area: 'right', when: () => S.view === 'calls' && !!S.selected && !!(S.call && (S.call.changes || []).length), badge: () => movedCount(S.call) || '', render: movedPanel })
