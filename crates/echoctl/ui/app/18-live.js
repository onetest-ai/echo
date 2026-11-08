// ── Live notes: in My notes beside the call, written by the app (`livenotes.rs`) ────────────
/** A call shows live notes (My notes' Now box) when it has live notes or answers, or while it records or is
 * transcribed with live notes on (or the AI settings unreadable). */
function hasLive(c) {
  const l = c.live || {}, st = l.status || {}
  if (l.notes || (l.answers && l.answers.length)) return true // what was written stays readable
  return (!!st.enabled || !!st.settings_error) && (c.status === 'recording' || c.status === 'transcribing')
}

const LIVE_STATE = {
  waiting: 'Waiting for enough to summarise',
  updating: 'Updating…',
  updated: 'Updated',
  failed: 'The last update failed',
  busy: 'Waiting for the model to finish another job',
  signing_in: 'Waiting for the sign-in',
  paused: 'Paused',
  ended: 'The call ended',
}

function ago(iso) {
  if (!iso) return ''
  const s = Math.max(0, Math.round((Date.now() - new Date(iso)) / 1000))
  return s < 60 ? `${s} s ago` : `${Math.round(s / 60)} min ago`
}

// ── live timeline: the user's notes and the live notes in one list, by time ───────────────────
/** The live notes' items (`live-notes.json`: decisions, action items, open questions) as entries of
 *  My notes' timeline, and where each goes among the user's own. No DOM: `ui/live-timeline.test.mjs`. */
const LiveTimeline = (() => {
  const at = (t) => (typeof t === 'number' && Number.isFinite(t) ? t : null)
  /** Decisions as ! , action items as ☐ (with their owner), open questions as ? — each with its time or null. */
  function items(n) {
    if (!n) return []
    return [
      ...(n.decisions || []).map((x) => ({ kind: 'decision', text: String(x.text || ''), t: at(x.t) })),
      ...(n.actions || []).map((x) => ({ kind: 'todo', text: String(x.task || ''), owner: String(x.owner || ''), t: at(x.t) })),
      ...(n.questions || []).map((x) => ({ kind: 'question', text: String(x.text || ''), t: at(x.t) })),
    ]
  }
  /** One list. The user's entries keep their own order (their file is never re-sorted); a live item goes
   *  just before the first of theirs that is later than it (the same second: theirs first), and after the
   *  last one otherwise. Live items with no time come last, in the draft's order. `mine`: `{ t }` (t or null). */
  function merge(mine, live) {
    const timed = live.map((x, i) => ({ x, i })).filter((a) => a.x.t != null).sort((a, b) => a.x.t - b.x.t || a.i - b.i).map((a) => a.x)
    const out = []
    let k = 0
    for (const m of mine) {
      if (m.t != null) while (k < timed.length && timed[k].t < m.t) out.push({ live: timed[k++] })
      out.push({ mine: m })
    }
    while (k < timed.length) out.push({ live: timed[k++] })
    for (const x of live) if (x.t == null) out.push({ live: x })
    return out
  }
  return { items, merge }
})()
// end of the live timeline block

/** The usage of the call's live jobs so far (Claude Code's own count), as "N updates · 62K in · 25K out · $0.02". */
function liveUsage(u) {
  if (!u || !u.jobs) return ''
  const tok = (x) => x >= 1e6 ? `${(x / 1e6).toFixed(1)}M` : x >= 1e3 ? `${Math.round(x / 1e3)}K` : String(x)
  const title = `${u.jobs} jobs · input ${u.input_tokens} + cache read ${u.cache_read_tokens} + cache write ${u.cache_write_tokens} · output ${u.output_tokens}${u.cost_usd ? ` · $${u.cost_usd.toFixed(3)}` : ''}`
  return `<span class="live-usage" title="${esc(title)}">${u.jobs} update${u.jobs === 1 ? '' : 's'} · ${tok(u.input_tokens + u.cache_read_tokens + u.cache_write_tokens)} in · ${tok(u.output_tokens)} out${u.cost_usd ? ` · $${u.cost_usd.toFixed(2)}` : ''}</span>`
}

/** Live notes at the top of My notes (only for a call that has them, or records with them on): Now in a
 *  small box, the status line under it (profile · state · usage), a sign-in or settings problem said with
 *  what to do, "Last time", and the ⋯ menu with what the live column had (the model, Pause / Resume, Ask Claude). */
function liveBox(c) {
  const l = c.live || {}, st = l.status || {}, n = l.notes
  const recording = c.status === 'recording'
  // Nothing kept and not recording (live notes off, or on with none written): My notes as without them.
  if (!hasLive(c) || (!recording && !n && !(l.answers || []).length)) return ''
  const state = st.paused ? 'paused' : st.state || (recording ? 'waiting' : 'ended')
  let line = LIVE_STATE[state] || ''
  if (state === 'updated' && n && n.updated_at) line = `Updated ${ago(n.updated_at)}`
  const p = profileOf(l.profile)
  const draftBy = n && n.profile && l.profile && n.profile !== l.profile ? `Draft by ${esc(profileName(n.profile))}; the next update by ${esc(profileName(l.profile))}` : ''
  const gaps = n ? (n.gaps || []).map((g) => `${clock(g[0])}–${clock(g[1])}`).join(', ') : ''
  const status = [`<span class="live-model">${esc(profileName(l.profile))}</span>`, `<span class="live-state ${esc(state)}">${esc(line)}</span>`, liveUsage(l.usage),
    draftBy && `<span>${draftBy}</span>`, gaps && `<span>Not covered: ${esc(gaps)}</span>`].filter(Boolean).join('<span class="nl-dot">·</span>')
  // A sign-in problem (or one going on) for this call's profile: said with what to do about it.
  const failedFor = st.needs_sign_in && profileOf(st.needs_sign_in)
  const sign = failedFor ? (signinNote(failedFor) || `<div class="signin error"><span class="signin-text">${esc(st.error || '')}</span><span class="signin-acts"><button class="btn primary" data-signin="${esc(failedFor.id)}">Sign in…</button></span></div>`)
    : signinNote(p)
  const problem = st.settings_error ? `The AI settings can't be read: ${st.settings_error}` : !st.enabled ? 'Live notes are off (menu → AI for calls → Live notes during calls).' : (!failedFor && st.state === 'failed' && st.error) || ''
  // Why there is no Now yet (none of the draft's items either).
  const why = n ? '' : !recording ? 'No live notes for this call.'
    : st.paused ? 'Paused — no live notes until you resume.'
    : st.needs_sign_in ? `No live notes until ${profileName(st.needs_sign_in)} can be used.`
    : st.state === 'signing_in' ? 'No live notes until the sign-in is done.'
    : st.state === 'failed' ? 'No live notes yet: the last update failed.'
    : st.state === 'busy' ? 'No live notes yet: the model is finishing another job.'
    : 'The first live notes appear after about 90 seconds of conversation.'
  const now = n && n.now ? `<p class="nl-now">${esc(n.now)}</p>` : why ? `<p class="nl-now nl-why">${esc(why)}</p>` : ''
  const model = ai().profiles.length > 1
    ? `<label class="btn split-lang pick live-model" title="Model for this call's live notes and minutes"><span>${esc(profileName(l.profile))}</span>${CARET}<select data-callprofile aria-label="Model for this call">${profileOptions(l.profile)}</select></label>`
    : `<span class="live-model">${esc(profileName(l.profile))}</span>`
  const menu = S.liveMenu ? `<div class="menu nl-menu" role="menu" aria-label="Live notes">
      <div class="menu-label">Model for this call</div><div class="nl-model">${model}${p ? `<span class="hint">${esc(profileWhere(p))}</span>` : ''}</div>
      ${recording ? `<div class="menu-sep"></div><button class="menu-item" role="menuitem" data-cmd="${st.paused ? 'live-resume' : 'live-pause'}">${st.paused ? 'Resume live notes' : 'Pause live notes'}</button>` : ''}
      <div class="menu-sep"></div><button class="menu-item" role="menuitem" data-cmd="ask-claude">${ICON(CLAUDE_MARK)}Ask Claude about this call</button>
    </div>` : ''
  return `<section class="nt-live" aria-label="Live notes">
    <div class="nl-head"><b class="nl-cap">Now</b><span class="grow"></span><span class="nl-menu-wrap"><button class="icon-btn" data-live-menu aria-haspopup="menu" aria-expanded="${!!S.liveMenu}" aria-label="Live notes menu" title="Live notes: the model, Pause, Ask Claude">${ICON('<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>')}</button>${menu}</span></div>
    ${now}
    <div class="nl-status">${status}</div>
    ${sign}${problem ? `<p class="live-error">${esc(problem)}</p>` : ''}
    ${prepCard(c)}
  </section>`
}

/** A live item in My notes' timeline: ✦, its time (a jump into the transcript) and its kind as the
 *  user's kinds show; read-only (the next update rewrites the draft). */
function liveItemHtml(x) {
  const [icon, label] = NOTE_LOOK[x.kind] || NOTE_LOOK.note
  const at = x.t != null ? `<a class="at" data-at="${x.t}" title="Jump to ${clock(x.t)} in the transcript">${esc(NoteDoc.stamp(x.t))}</a> ` : ''
  const owner = x.owner ? `<span class="owner${x.owner === 'Me' ? ' me' : ''}">${esc(x.owner)}</span> ` : ''
  return `<div class="nt-entry nt-livei k-${x.kind}" data-live-item><span class="nt-icon nl-mark" title="Live notes (written by the model)">✦</span><div class="nt-body"><div>${at}<span class="nl-kind" title="${label}">${icon}</span> ${owner}<span class="nt-text">${esc(x.text)}</span></div></div><span></span></div>`
}

/** Answers to questions about the call (`live-asks.jsonl`), after the timeline. */
function liveAnswersHtml(c) {
  const a = liveAnswers(c)
  return a ? `<div class="nt-head">Asked about this call</div><div class="nl-answers">${a}</div>` : ''
}

/** The Transcript tab of a call that records with live notes: the live transcript at full width, kept at
 *  its end as people speak (`#live-transcript`); its live notes are in My notes. */
function liveTranscript(segs) {
  return `<section class="live-transcript" id="live-transcript" aria-label="Transcript">${segs.length ? turnsHtml(segs, '', null, noteMarks(S.call)) : '<p class="muted">Listening. The transcript appears here as people speak.</p>'}</section>`
}

// The ⋯ menu at Now: opens and closes on its button, closes on a click outside it or Escape.
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-live-menu]')
  if (b) { S.liveMenu = !S.liveMenu; return render() }
  if (!S.liveMenu) return
  const inMenu = e.target.closest && e.target.closest('.nl-menu')
  if (inMenu && !e.target.closest('.menu-item')) return // the model's select stays open
  // Closed after the click's own handler (a menu item's command, a time chip) has run on the page as it was.
  S.liveMenu = false
  setTimeout(render, 0)
})
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && S.liveMenu) { S.liveMenu = false; render() }
})

function prepCard(c) {
  const p = (c.live || {}).prep
  if (!p || (!(p.open_actions || []).length && !(p.highlights || []).length)) return ''
  const a = p.open_actions || [], h = p.highlights || []
  const mine = S.prepOpen[c.name]
  const open = (mine !== undefined ? mine : c.status === 'recording' && !(c.live.notes && c.live.notes.updated_at)) ? ' open' : ''
  return `<details class="prep" data-prep="${esc(c.name)}"${open}><summary>Last time${p.date ? ` (${esc(p.date)})` : ''}: ${a.length} still to do, ${h.length} highlight${h.length === 1 ? '' : 's'}</summary>
    ${a.length ? `<h3>Still to do</h3><ul>${a.map((x) => `<li>${x.owner ? `<span class="owner">${esc(x.owner)}</span> ` : ''}${x.link ? `<a href="${esc(x.link)}">${esc(x.task)}</a>` : esc(x.task)}${x.due ? ` <span class="muted">due ${esc(x.due)}</span>` : ''}</li>`).join('')}</ul>` : ''}
    ${h.length ? `<h3>Highlights</h3><ul>${h.map((x) => `<li>${esc(x)}</li>`).join('')}</ul>` : ''}
  </details>`
}

/** An answer: escaped, line breaks kept, "* " / "- " bullets as •, [mm:ss] as jumps. */
function answerText(t) {
  return esc(String(t).replace(/^\s*[*-]\s+/gm, '• ')).replace(/\[((?:\d{1,2}:)?\d{1,2}:\d{2})\]/g, (_, x) => {
    const p = x.split(':').map(Number)
    const sec = p.length === 3 ? p[0] * 3600 + p[1] * 60 + p[2] : p[0] * 60 + p[1]
    return `<a class="at" data-at="${sec}" title="Jump to ${x} in the transcript">${x}</a>`
  }).replace(/\n/g, '<br>')
}

const ASK_TITLE = { missed: 'What did I miss?', question: 'Questions you could ask', ask: 'You asked' }
/** What a question put back waits for (`asking.waiting`). */
const ASK_WAIT = { model: 'Waiting for the model to finish another job…', sign_in: 'Waiting for the sign-in to finish…' }

function liveAnswers(c) {
  const l = c.live || {}, st = l.status || {}
  const pending = st.asking
    ? `<div class="answer pending"><b>${esc(ASK_TITLE[st.asking.kind] || '')}</b>${st.asking.text ? ` ${esc(st.asking.text)}` : ''}<p class="muted">${ASK_WAIT[st.asking.waiting] || 'Thinking…'}</p></div>`
    : st.queued > 0 ? `<div class="answer pending"><p class="muted">Your question is next.</p></div>` : ''
  const list = (l.answers || []).slice().reverse().slice(0, 30).map((a) => `<div class="answer">
      <div class="answer-head"><b>${esc(ASK_TITLE[a.kind] || '')}</b>${a.question ? ` <span>${esc(a.question)}</span>` : ''}<span class="muted">${esc(hm(new Date(a.asked_at)))}</span></div>
      ${a.error ? `<p class="error">${esc(a.error)}</p>` : `<p>${answerText(a.answer)}</p>`}</div>`).join('')
  return pending + list
}
