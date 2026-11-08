// ── projects: how a project is drawn, and its stages (no DOM: `projects.test.mjs`) ────────────
// A project shows as a small tile: its logo if it has one, else a symbol the user picked, else an
// identicon from its id (a 5×5 pattern mirrored left to right, as GitHub draws a user without an
// avatar) — in its colour. The same id always gives the same pattern.
const ProjectArt = (() => {
  /** FNV-1a of a string, as an unsigned 32-bit number. */
  function hash(str) {
    let h = 0x811c9dc5
    for (const ch of String(str)) { h ^= ch.codePointAt(0); h = Math.imul(h, 0x01000193) >>> 0 }
    return h >>> 0
  }
  /** 25 cells, row by row: columns 0–2 from the hash, 3–4 mirror 1–0. Never (nearly) empty or full. */
  function cells(id) {
    const h = hash(id)
    let bits = Array.from({ length: 15 }, (_, i) => (h >>> i) & 1)
    const on = bits.reduce((a, b) => a + b, 0)
    if (on < 4 || on > 12) bits = bits.map((b, i) => (i % 2 ? b : 1 - b))
    const out = []
    for (let r = 0; r < 5; r++) for (let c = 0; c < 5; c++) out.push(bits[r * 3 + (c < 3 ? c : 4 - c)] === 1)
    return out
  }
  const STAGES = [
    ['pursuit', 'Pursuit', [['lead', 'Lead'], ['proposal', 'Proposal'], ['negotiation', 'Negotiation']]],
    ['delivery', 'Delivery', [['kickoff', 'Kickoff'], ['active', 'Active'], ['on_hold', 'On hold'], ['wrap_up', 'Wrap-up']]],
    ['closed', 'Closed', [['done', 'Done'], ['lost', 'Lost'], ['cancelled', 'Cancelled']]],
  ]
  /** A stage's label and its group's ("on_hold" → {label: 'On hold', group: 'Delivery'}). */
  function stage(id) {
    for (const [gid, glabel, xs] of STAGES) for (const [sid, label] of xs) if (sid === id) return { id, label, group: glabel, gid }
    return { id, label: id, group: '', gid: '' }
  }
  return { hash, cells, STAGES, stage }
})()
// end of the projects block

