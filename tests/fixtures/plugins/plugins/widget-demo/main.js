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
  // Pushed by commands (`echoctl plugin call widget-demo <name>`): CI's state, the note, and taking the note off.
  ctx.command('ci', (a) => { ctx.widgets.set('ci', { state: (a && a.state) || 'ok', title: 'CI', text: 'main is green' }); return true })
  ctx.command('note', () => { ctx.widgets.set('note', { title: 'Release notes', text: 'Ship Friday.\nTell support.' }); return true })
  ctx.command('clear-note', () => { ctx.widgets.clear('note'); return true })
}
