// A test plugin (tests/cases/today/TC-047): a chart of every type Echo draws, a headline on each, stats with a trend.
// Made-up data.
const weeks = ['2026-10-06', '2026-10-13', '2026-10-20', '2026-10-27', '2026-11-03', '2026-11-10']
export function apply(ctx) {
  ctx.widgets.provide('calls', () => ({
    type: 'line', series: [{ key: 'rec', label: 'recorded' }, { key: 'min', label: 'with minutes' }],
    points: weeks.map((w, i) => ({ label: w, values: { rec: [12, 15, 14, 19, 22, 24][i], min: [8, 9, 13, 12, 15, 18][i] } })),
    headline: { value: 24, label: 'recorded', change: { by: 2, than: 'last week', good: true } },
  }))
  ctx.widgets.provide('hours', () => ({
    type: 'area', unit: 'h', points: weeks.map((w, i) => ({ label: w, value: [12, 15, 14, 19, 22, 24][i] })),
    headline: { value: '24 h', label: 'this week', change: { by: 2, than: 'last week', good: true } },
  }))
  ctx.widgets.provide('deals', () => ({
    type: 'bar', series: [{ key: 'won', label: 'won' }, { key: 'lost', label: 'lost' }],
    points: [['Q1', 18, 11], ['Q2', 21, 14], ['Q3', 16, 15], ['Q4', 24, 17]].map(([label, won, lost]) => ({ label, values: { won, lost } })),
    headline: { value: 24, label: 'won in Q4', change: { by: 8, than: 'Q3', good: true } },
  }))
  ctx.widgets.provide('staffing', () => ({
    type: 'stacked', series: [{ key: 'open', label: 'open' }, { key: 'proposed', label: 'proposed' }, { key: 'booked', label: 'booked' }],
    points: weeks.map((w, i) => ({ label: w, values: { open: [6, 5, 4, 3, 4, 2][i], proposed: [4, 5, 5, 4, 3, 4][i], booked: [2, 3, 5, 7, 8, 9][i] } })),
    headline: { value: 9, label: 'booked', change: { by: 1, than: 'last week', good: true } },
  }))
  ctx.widgets.provide('status', () => ({
    type: 'stacked-bars', percent: true,
    series: [{ key: 'on', label: 'on track', tone: 'good' }, { key: 'risk', label: 'at risk', tone: 'warn' }, { key: 'late', label: 'late', tone: 'bad' }],
    points: [['Northwind', 60, 25, 15], ['Contoso', 35, 40, 25], ['Fabrikam', 20, 30, 50]].map(([label, on, risk, late]) => ({ label, values: { on, risk, late } })),
    headline: { value: '38%', label: 'on track, across 3 projects', change: { by: 4, than: 'last week', good: false } },
  }))
  ctx.widgets.provide('tasks', () => ({
    type: 'bars', points: [['Acme rollout', 7], ['Globex pilot', 5], ['Initech intro', 3], ['Hiring', 2]].map(([label, value]) => ({ label, value })),
    headline: { value: 17, label: 'open tasks' },
  }))
  ctx.widgets.provide('time', () => ({
    type: 'pie', unit: 'h', points: [['Delivery', 42], ['Presales', 23], ['Internal', 18], ['Hiring', 12], ['Travel', 3], ['Admin', 2]].map(([label, value]) => ({ label, value })),
    headline: { value: '42%', label: 'delivery' },
  }))
  ctx.widgets.provide('positions', () => ({
    type: 'donut', center: { value: 16, label: 'positions' },
    points: [['Booked', 9], ['Proposed', 4], ['Open', 3]].map(([label, value]) => ({ label, value })),
  }))
  ctx.widgets.provide('pipeline', () => ({
    type: 'funnel', points: [['Leads', 120], ['Qualified', 64], ['Proposal', 30], ['Negotiation', 14], ['Won', 9]].map(([label, value]) => ({ label, value })),
    headline: { value: 9, label: 'won', change: { by: 2, than: 'last month', good: true } },
  }))
  ctx.widgets.provide('week', () => ({
    items: [
      { label: 'calls this week', value: 23, trend: [12, 15, 14, 19, 22, 23] },
      { label: 'minutes sent', value: 19, trend: [8, 9, 13, 12, 15, 19] },
      { label: 'open positions', value: 7, trend: [12, 11, 9, 10, 8, 7] },
    ],
  }))
  ctx.widgets.provide('broken', () => ({ type: 'pie', points: [{ label: 'refund', value: -40 }] }))
}
