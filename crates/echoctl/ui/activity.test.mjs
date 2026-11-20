// Settings → Activity (#859): the runs' words (acLabel, acLine), the filters (acShown), the sums (acSummary, acDays,
// acByJob), a run's timeline rows (acRows) and what went in and came out of a step (acDetails).
// Run by scripts/check-mod.sh (`node crates/echoctl/ui/activity.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const from = '// ── activity:', to = '// end of the activity block'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'activity block not found in app.js')
const { acLabel, acLine, acShown, acSummary, acDays, acByJob, acNum, acRows, acDetails, acFind, acStepLabel, acTick, acWhenFull } =
  new Function(`${src.slice(start, end)}\nreturn { acLabel, acLine, acShown, acSummary, acDays, acByJob, acNum, acRows, acDetails, acFind, acStepLabel, acTick, acWhenFull }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

const now = new Date(2026, 10, 20, 15, 0)
const at = (d, h) => new Date(2026, 10, d, h, 5).toISOString()
const TRACES = [
  { id: 'a', name: 'job.thread_sync', kind: 'job', job: 'thread_sync', start: at(20, 14), ok: true, summary: '1 read', runs: 2, tokens: 70000, cached: 56000, requests: 3, cost_usd: 0.37, model: 'sonnet' },
  { id: 'b', name: 'job.yolo', kind: 'job', job: 'yolo', start: at(20, 13), ok: false, error: 'Claude Code ended without a result', runs: 1, tokens: 30000, cached: 0, requests: 0, cost_usd: 0.09, model: 'sonnet' },
  { id: 'c', name: 'model.request', kind: 'model', job: null, start: at(19, 9), ok: true, runs: 0, tokens: 464, cached: 0, requests: 0, cost_usd: 0, model: 'virtual' },
  { id: 'd', name: 'app.calendar_sync', kind: 'app', job: null, start: at(19, 8), ok: true, runs: 0, tokens: 0, cached: 0, requests: 4, cost_usd: 0, model: '' },
  { id: 'e', name: 'cli.server', kind: 'command', job: null, start: at(18, 8), ok: false, error: 'could not reach the server', runs: 0, tokens: 0, requests: 0, cost_usd: 0, model: '' },
  { id: 'f', name: 'job.thread_sync', kind: 'job', job: 'thread_sync', start: at(14, 8), ok: true, summary: '2 read', runs: 2, tokens: 50000, cached: 40000, requests: 1, cost_usd: 0.2, model: 'sonnet, haiku' },
]

test('each run is named as the user reads it', () => {
  assert.deepEqual(TRACES.map(acLabel), ['Thread sync', 'Yolo', 'Model call · virtual', 'Calendar sync', 'echoctl server failed', 'Thread sync'])
  assert.equal(acLine(TRACES[0]), 'Thread sync · 1 read')
  assert.equal(acLine(TRACES[1]), 'Yolo · Claude Code ended without a result', 'a failed run says why')
  assert.equal(acLine(TRACES[3]), 'Calendar sync · 4 requests')
})

test('the filters keep their kinds', () => {
  const ids = (show) => acShown(TRACES, show).map((t) => t.id)
  assert.deepEqual(ids('all'), ['a', 'b', 'c', 'd', 'e', 'f'])
  assert.deepEqual(ids('failed'), ['b', 'e'])
  assert.deepEqual(ids('jobs'), ['a', 'b', 'c', 'f'])
  assert.deepEqual(ids('requests'), ['d'])
  assert.deepEqual(ids('commands'), ['e'])
})

test('the facts: runs (requests and commands aside), failed, tokens, cached, cost, requests', () => {
  const s = acSummary(TRACES)
  assert.deepEqual({ ...s, cost: Math.round(s.cost * 100) / 100 }, { runs: 4, failed: 1, tokens: 150464, cached: 96000, cost: 0.66, requests: 8 })
})

test('tokens a day: the last 7 days to today, by group, a day with none at 0', () => {
  const days = acDays(TRACES, now, 7)
  assert.deepEqual(days.map((d) => d.label), ['Sat', 'Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Today'])
  assert.deepEqual(days[6].by, { thread_sync: 70000, other: 30000 })
  assert.equal(days[5].total, 464, 'yesterday: the model call')
  assert.equal(days[0].total, 50000, 'the 14th: the first of the 7 days')
  assert.equal(acDays([{ ...TRACES[5], start: at(13, 8) }], now, 7).reduce((n, d) => n + d.total, 0), 0, 'the 13th: before them, not counted')
  assert.deepEqual(acDays(TRACES, now, 1).map((d) => d.label), ['Today'])
  const month = acDays(TRACES, now, 30)
  assert.equal(month.length, 30)
  assert.deepEqual(month.filter((d) => d.label).map((d) => d.label), ['26 Oct', '2 Nov', '9 Nov', '16 Nov', 'Today'], 'past a week: Mondays by date, and today')
  assert.equal(month.reduce((n, d) => n + d.total, 0), 150464, 'every run of the month counted')
  assert.deepEqual(acDays(TRACES, now, 90).filter((d) => d.label).map((d) => d.label), ['Sep', 'Oct', 'Nov', 'Today'], 'past a month: each month by its first day')
})

test('by job: a row a job, most tokens first, models once each; requests and commands left out', () => {
  const rows = acByJob(TRACES)
  assert.deepEqual(rows.map((r) => [r.label, r.runs, r.failed, r.tokens]), [['Thread sync', 2, 0, 120000], ['Yolo', 1, 1, 30000], ['Model call · virtual', 1, 0, 464], ['Calendar sync', 1, 0, 0]])
  assert.deepEqual(rows[0].models, ['sonnet', 'haiku'])
})

test('tokens in few characters', () => {
  assert.deepEqual([0, 950, 1234, 12400, 6400000, 19500000].map(acNum), ['—', '950', '1.2K', '12K', '6.4M', '20M'])
})

const T = (s) => new Date(Date.UTC(2026, 10, 20, 12, 0, 0, s)).toISOString()
const TRACE = { spans: [{ span: 'j', name: 'job.thread_sync', start: T(0), end: T(1200), attrs: {}, events: [], children: [
  { span: 'f', name: 'http.request', start: T(0), end: T(100), attrs: { method: 'GET', host: 'graph.microsoft.com', path: '/v1.0/me/messages', ok: true }, events: [], children: [] },
  { span: 'r', name: 'claude.run', start: T(120), end: T(870), attrs: { model: 'virtual', input: 10, output: 1200, cache_read: 50000, cache_write: 15000, files: { 'append-system-prompt-file': "# Echo: a thread's sync — extract\nRead the evidence…" } },
    events: [
      { name: 'claude.input', attrs: { text: JSON.stringify({ type: 'user', message: { role: 'user', content: 'Acme security review (made up)' } }) } },
      { name: 'claude.assistant', attrs: { message: { content: [{ type: 'text', text: 'Reading the context first.' }, { type: 'tool_use', id: 'toolu_1', name: 'Bash', input: { command: 'echoctl project ledger globex-pilot context' } }] } } },
      { name: 'claude.user', attrs: { message: { content: [{ type: 'tool_result', tool_use_id: 'toolu_1', is_error: true, content: 'Echo’s project sync: only acme-rollout runs here' }] } } },
      { name: 'claude.result.success', attrs: { result: 'Created a risk.', is_error: false } },
    ],
    children: [{ span: 'i', name: 'claude_code.interaction', start: T(130), end: T(860), attrs: {}, events: [], children: [
      { span: 'q1', name: 'claude_code.llm_request', start: T(150), end: T(260), attrs: { model: 'virtual', input_tokens: 15000, output_tokens: 88, cache_read_tokens: 600 }, events: [], children: [] },
      { span: 't1', name: 'claude_code.tool', start: T(270), end: T(290), attrs: { tool_name: 'Bash', 'gen_ai.tool.call.id': 'toolu_1' }, events: [], children: [
        { span: 'b1', name: 'claude_code.tool.blocked_on_user', start: T(270), end: T(272), attrs: { decision: 'accept' }, events: [], children: [] }] },
    ] }] },
] }] }

test('the timeline: depth first, Claude Code’s interaction and a tool’s parts folded, offsets from the start', () => {
  const { rows, took } = acRows(TRACE)
  assert.equal(took, 1200)
  assert.deepEqual(rows.map((r) => [r.id, r.depth, r.label, r.kind, r.at, r.took]), [
    ['j', 0, 'Thread sync', 'echo', 0, 1200],
    ['f', 1, 'GET graph.microsoft.com/v1.0/me/messages', 'out', 0, 100],
    ['r', 1, "Claude Code · a thread's sync — extract", 'run', 120, 750],
    ['q1', 2, 'Request · virtual', 'req', 150, 110],
    ['t1', 2, 'Bash · echoctl project ledger globex-pilot context', 'bad', 270, 20],
  ])
  assert.equal(rows[2].tokens, 66210, 'a run: input, output and cache')
  assert.equal(rows[3].tokens, 15688)
})

test('a step: what went in and came out, in order, as text', () => {
  const d = acDetails(acFind(TRACE, 'r'))
  assert.deepEqual(d.map((x) => [x.k, x.bad || false]), [['Instructions', false], ['In', false], ['Claude', false], ['Tool', false], ['Refused', true], ['Result', false]])
  assert.equal(d[1].text, 'Acme security review (made up)', 'a stream-json message: its text')
  assert.equal(d[3].text, 'echoctl project ledger globex-pilot context')
  assert.equal(acStepLabel(acFind(TRACE, 'f')), 'GET graph.microsoft.com/v1.0/me/messages')
  assert.equal(acFind(TRACE, 'nope'), null)
  const tool = acDetails(acFind(TRACE, 't1'), TRACE)
  assert.deepEqual(tool.map((x) => [x.k, x.note || '', x.text]), [['Tool', 'Bash', 'echoctl project ledger globex-pilot context'], ['Refused', '', 'Echo’s project sync: only acme-rollout runs here']], 'a tool: its call and its result, from Echo’s stream')
})

test('the axis and the header: seconds on the axis, the start to the second', () => {
  assert.deepEqual([0, 303, 1212, 45000, 125000].map(acTick), ['0.0 s', '0.3 s', '1.2 s', '45 s', '2 min 5 s'])
  assert.equal(acWhenFull(new Date(2026, 10, 20, 10, 35, 40).toISOString(), now), 'Today 10:35:40')
  assert.equal(acWhenFull(new Date(2026, 10, 18, 9, 5, 0).toISOString(), now), 'Wed 18 Nov 09:05:00')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok  ${name}`) } catch (e) { failed++; console.log(`FAIL ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
