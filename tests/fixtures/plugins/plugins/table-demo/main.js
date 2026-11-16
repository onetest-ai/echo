// A test plugin (tests/cases/today/TC-042): a table widget and stats whose items link to its rows. Made-up data.
const rows = [
  { id: 'p1', group: 'nwb', link: 'https://staffing.acme.example/position/1101000001', cells: { urgency: { text: 'Critical', tone: 'bad' }, position: '1101000001', role: 'Architect · Data Solution Architecture', start: '2026-11-15', status: 'Proposed', candidates: 3, why: 'starts in 2 days, not booked' },
    detail: [{ text: 'Dana Example', sub: 'Proposed · due Nov 14' }, { text: 'Lee Sample', sub: 'Booked · due Nov 15' }, { text: 'Ira Placeholder', sub: 'Preselected' }] },
  { id: 'p2', group: 'nwb', link: 'https://staffing.acme.example/position/1101000002', cells: { urgency: { text: 'Critical', tone: 'bad' }, position: '1101000002 · Key Engineer for the DevOps platform and its release automation', role: 'Key Engineer · DevOps', start: '2026-11-18', status: 'Booked', candidates: 6, why: 'booked, starts in 5 days, not assigned' } },
  { id: 'p3', group: 'ctl', link: 'echo://thread/acme-rollout', cells: { urgency: { text: 'High', tone: 'warn' }, position: '1101000003', role: 'Quality Engineer · AI Lead', start: '2026-11-24', status: 'Proposed', candidates: 4, why: 'starts in 11 days, not booked' } },
  { id: 'p4', group: 'ctl', cells: { urgency: { text: 'High', tone: 'warn' }, position: '1101000005', role: '<img src=x onerror=alert(1)> kept as text', start: '2026-11-27', status: 'Open', candidates: 0, why: 'no candidates yet' } },
  { id: 'p5', cells: { urgency: { text: 'On hold', tone: 'muted' }, position: '1101000007', role: 'Delivery Manager', start: '2027-01-05', status: 'On hold', candidates: 0, why: { text: 'project on hold', tone: 'muted' } } },
  // More rows than the card shows: the rest behind "+N more" (today/TC-042).
  ...Array.from({ length: 8 }, (_, i) => ({ id: `q${i + 1}`, group: 'later', cells: { urgency: { text: 'Normal', tone: 'good' }, position: `11010001${String(i + 10)}`, role: `Engineer ${i + 1}`, start: `2027-02-${String(i + 10)}`, status: 'Open', candidates: i, why: 'starts next quarter' } })),
]
let nudged = 0
export function apply(ctx) {
  // A thread's or a client's table on its dashboard: a row's details, an https link (asked once), an action.
  ctx.command('nudge', () => { nudged++; return true })
  ctx.command('nudged', () => nudged)
  ctx.widgets.provide('roles', ({ params }) => ({
    columns: [{ key: 'position', label: 'Position' }, { key: 'role', label: 'Role' }],
    rows: rows.slice(0, 2).map((r) => ({ id: r.id, link: r.link, detail: r.detail, cells: { position: r.cells.position, role: r.cells.role } })),
    note: params.thread ? `thread ${params.thread}` : params.client ? `client ${params.client}` : 'everywhere',
    open: 'https://staffing.acme.example/roles', actions: [{ label: 'Nudge', command: 'nudge' }],
  }))
  // Set to a thread or a client (on Today by its ⚙, on a dashboard by the page): says which, so a case sees it.
  ctx.widgets.provide('positions', ({ params }) => ({ value: params.thread ? 3 : params.client ? 7 : 12, label: 'open positions', note: `${params.thread ? `thread ${params.thread}` : 'every thread'} · ${params.client ? `client ${params.client}` : 'every client'}` }))
  ctx.widgets.provide('staffing', () => ({
    columns: [
      { key: 'urgency', label: 'Urgency', kind: 'pill' }, { key: 'position', label: 'Position' }, { key: 'role', label: 'Role' },
      { key: 'start', label: 'Start', kind: 'date' }, { key: 'status', label: 'Status', hide: 'M' },
      { key: 'candidates', label: 'Candidates', kind: 'number' }, { key: 'why', label: 'Why', hide: 'M' },
    ],
    groups: [{ id: 'nwb', label: 'NWB-PAI · Northwind Bank' }, { id: 'ctl', label: 'CTL-GENAI · Contoso Labs' }, { id: 'later', label: 'Later this quarter' }],
    rows, note: '2 critical, 2 high', empty: 'Every position is assigned.',
    // From an earlier read of its own (made-up): three hours ago, whenever Echo asks.
    asOf: new Date(Date.now() - 3 * 3600 * 1000).toISOString(),
  }))
  ctx.widgets.provide('counts', () => ({ items: [
    { label: 'critical', value: 2, tone: 'bad', link: 'echo://thread/acme-rollout' },
    { label: 'high', value: 2, tone: 'warn', link: 'https://staffing.acme.example/?urgency=high' },
    { label: 'on hold', value: 1 },
  ] }))
  // A scorecard (today/TC-053): each unit's trend, its deals' stage mix, new deals in each of 12 weeks, and a total.
  const weeks = Array.from({ length: 12 }, (_, i) => ({ key: `w${i + 1}`, label: `W${i + 1}`, kind: 'heat', group: 'New deals by week' }))
  const units = [['Data', [2, 4, 3, 6], [5, 3, 1], [3, 0, 1, 4, 2, 0, 5, 6, 2, 1, 0, 3]], ['Cloud', [5, 3, 3, 2], [2, 2, 2], [1, 1, 0, 0, 2, 1, 0, 0, 1, 0, 0, 1]], ['Security', [1, 1, 2, 4], [1, 4, 0], [0, 0, 0, 1, 0, 2, 1, 0, 3, 2, 1, 2]]]
  ctx.widgets.provide('scorecard', () => ({
    columns: [{ key: 'unit', label: 'Unit' }, { key: 'trend', label: 'Requests', kind: 'spark' }, { key: 'mix', label: 'Stage mix', kind: 'mix', series: [{ key: 'q', label: 'Qualified' }, { key: 'p', label: 'Proposal' }, { key: 'lost', label: 'Lost', tone: 'bad' }] }, ...weeks],
    rows: units.map(([unit, trend, [q, p, lost], ws]) => ({ id: unit, cells: { unit, trend: { values: trend, text: String(trend[trend.length - 1]) }, mix: { parts: [{ key: 'q', value: q }, { key: 'p', value: p }, { key: 'lost', value: lost }], text: String(q + p + lost) }, ...Object.fromEntries(ws.map((v, i) => [`w${i + 1}`, v])) } })),
    total: { cells: Object.fromEntries(weeks.map((w, i) => [w.key, units.reduce((a, u) => a + u[3][i], 0)])) },
  }))
  ctx.widgets.provide('wrong', () => ({ columns: [{ key: 'start', label: 'Start', kind: 'date' }], rows: [{ id: 1, cells: { start: '15/11/2026' } }] }))
}
