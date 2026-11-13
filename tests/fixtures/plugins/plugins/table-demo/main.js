// A test plugin (tests/cases/today/TC-042): a table widget and stats whose items link to its rows. Made-up data.
const rows = [
  { id: 'p1', group: 'nwb', link: 'https://staffing.acme.example/position/1101000001', cells: { urgency: { text: 'Critical', tone: 'bad' }, position: '1101000001', role: 'Architect · Data Solution Architecture', start: '2026-11-15', status: 'Proposed', candidates: 3, why: 'starts in 2 days, not booked' },
    detail: [{ text: 'Dana Example', sub: 'Proposed · due Nov 14' }, { text: 'Lee Sample', sub: 'Booked · due Nov 15' }, { text: 'Ira Placeholder', sub: 'Preselected' }] },
  { id: 'p2', group: 'nwb', link: 'https://staffing.acme.example/position/1101000002', cells: { urgency: { text: 'Critical', tone: 'bad' }, position: '1101000002', role: 'Key Engineer · DevOps', start: '2026-11-18', status: 'Booked', candidates: 6, why: 'booked, starts in 5 days, not assigned' } },
  { id: 'p3', group: 'ctl', link: 'echo://thread/acme-rollout', cells: { urgency: { text: 'High', tone: 'warn' }, position: '1101000003', role: 'Quality Engineer · AI Lead', start: '2026-11-24', status: 'Proposed', candidates: 4, why: 'starts in 11 days, not booked' } },
  { id: 'p4', group: 'ctl', cells: { urgency: { text: 'High', tone: 'warn' }, position: '1101000005', role: '<img src=x onerror=alert(1)> kept as text', start: '2026-11-27', status: 'Open', candidates: 0, why: 'no candidates yet' } },
  { id: 'p5', cells: { urgency: { text: 'On hold', tone: 'muted' }, position: '1101000007', role: 'Delivery Manager', start: '2027-01-05', status: 'On hold', candidates: 0, why: { text: 'project on hold', tone: 'muted' } } },
  // Enough rows to scroll inside the card (its column names stay put).
  ...Array.from({ length: 8 }, (_, i) => ({ id: `q${i + 1}`, group: 'later', cells: { urgency: { text: 'Normal', tone: 'good' }, position: `11010001${String(i + 10)}`, role: `Engineer ${i + 1}`, start: `2027-02-${String(i + 10)}`, status: 'Open', candidates: i, why: 'starts next quarter' } })),
]
export function apply(ctx) {
  ctx.widgets.provide('staffing', () => ({
    columns: [
      { key: 'urgency', label: 'Urgency', kind: 'pill' }, { key: 'position', label: 'Position' }, { key: 'role', label: 'Role' },
      { key: 'start', label: 'Start', kind: 'date' }, { key: 'status', label: 'Status', hide: 'M' },
      { key: 'candidates', label: 'Candidates', kind: 'number' }, { key: 'why', label: 'Why', hide: 'M' },
    ],
    groups: [{ id: 'nwb', label: 'NWB-PAI · Northwind Bank' }, { id: 'ctl', label: 'CTL-GENAI · Contoso Labs' }, { id: 'later', label: 'Later this quarter' }],
    rows, note: '2 critical, 2 high', empty: 'Every position is assigned.',
  }))
  ctx.widgets.provide('counts', () => ({ items: [
    { label: 'critical', value: 2, tone: 'bad', link: 'echo://thread/acme-rollout' },
    { label: 'high', value: 2, tone: 'warn', link: 'https://staffing.acme.example/?urgency=high' },
    { label: 'on hold', value: 1 },
  ] }))
  ctx.widgets.provide('wrong', () => ({ columns: [{ key: 'start', label: 'Start', kind: 'date' }], rows: [{ id: 1, cells: { start: '15/11/2026' } }] }))
}
