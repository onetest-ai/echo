// A test plugin (tests/cases/today/TC-029): one widget of each kind, its data from `provide` (asked by Echo) or `set`.
export function apply(ctx) {
  ctx.widgets.provide('open', () => ({ value: 12, label: 'open tickets', tone: 'warn', note: '3 new today' }))
  ctx.widgets.provide('sprint', () => ({ items: [{ label: 'done', value: 21, tone: 'good' }, { label: 'to do', value: 8 }, { label: 'blocked', value: 2, tone: 'bad' }] }))
  ctx.widgets.provide('builds', () => ({ items: [
    { title: 'main #41 passed', sub: '2 min ago' },
    { title: 'Open the task', link: 'echo://task/acme-rollout/T001' },
    { title: '<img src=x onerror=alert(1)> kept as text', link: 'https://evil.example' },
  ] }))
  ctx.widgets.provide('broken', () => ({ label: 'no value here' }))
  ctx.widgets.provide('trend', () => ({ type: 'bar', unit: 'calls', points: [
    { label: 'Mon', value: 3 }, { label: 'Tue', value: 7 }, { label: 'Wed', value: 5 }, { label: 'Thu', value: 2 }, { label: 'Fri', value: 4 },
  ], note: 'Busiest on Tuesday.' }))
  // Pushed by commands (`echoctl plugin call widget-demo <name>`): CI's state, the note, and taking the note off.
  ctx.command('ci', (a) => {
    const state = (a && a.state) || 'ok'
    ctx.widgets.set('ci', { state, title: 'CI', text: state === 'ok' ? 'main is green' : 'main #42 failed',
      open: 'https://ci.example/run/42', actions: state === 'ok' ? [] : [{ label: 'Re-run', command: 'ci', args: { state: 'ok' } }] })
    return true
  })
  ctx.command('note', () => { ctx.widgets.set('note', { title: 'Release notes', text: 'Ship Friday.\nTell support.' }); return true })
  ctx.command('clear-note', () => { ctx.widgets.clear('note'); return true })
}
