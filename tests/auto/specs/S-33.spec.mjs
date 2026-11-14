// S-33 Prep, Yolo and a thread's sync on the virtual model — the real Claude Code, its tools, Echo's guard, the CLI's
// refusals and the ledger; only the model is scripted (standard data).
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, textOf, fill, changed, gitStatus } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { MeetingPage } from '../pages/meeting.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'
import { WorkPage } from '../pages/work.page.mjs'

const data = useData(test, { set: 'standard', ai: 'virtual' })

/** The lines of a run's log. */
const lines = (text) => text.split('\n').filter(Boolean)

/** "Claude acts on its own" on or off, through the served window's own IPC (the page loaded first). */
async function setActs(page, on) {
  await page.goto(data.base + '/')
  await page.evaluate((v) => fetch('ipc', { method: 'POST', body: JSON.stringify({ cmd: 'settings_set', key: 'folder_skip_permissions', value: v }) }), on)
  await page.waitForLoadState('networkidle')
}

scenario(test, 'S-33', "Prep, Yolo and a thread's sync on the virtual model", () => {
  test("1 · prep --write sends the meeting's facts in one request and writes the prep with the model's In short and its picks, shown in the window", covers('prep/TC-015', 'F-PREP-05'), async ({ page }) => {
    const { prep: p, virtualModel: vm } = data.set
    const t = data.text.prep
    const meeting = new MeetingPage(page, data)
    await meeting.goto()
    data.model.use(p.script)
    data.model.clear()

    // 1: the meeting, still without a prep.
    const upcoming = () => data.cli('prep', '--upcoming').json().meetings.find((m) => m.subject === p.subject)
    expect(upcoming()).toMatchObject({ id: p.id, threads: p.threads, prep: null })

    // 2: the prep, with the model's summary and only the items it picked.
    const cli = data.cli('prep', '--write', p.id)
    expect(cli.code).toBe(0)
    const prep = cli.json()
    expect(prep).toMatchObject({ id: p.id, subject: p.subject, profile: vm.profile, model: vm.model, summary: p.summary, threads: p.threads })
    expect(prep.error ?? null).toBeNull()
    expect(Date.now() - Date.parse(prep.written_at)).toBeLessThan(60_000)
    expect(prep.expected.map((x) => x.title)).toEqual(p.expected)
    expect(prep.open.map((x) => x.title)).toEqual(p.open)
    expect(prep.people.length).toBeGreaterThan(0)

    // 3: one request to the Messages API, no Claude Code, the facts numbered for the model.
    expect(data.model.requests()).toHaveLength(1)
    const [e] = data.model.turns(p.rule)
    expect(e).toMatchObject({ path: p.path, model: vm.model, stream: false, tools: [] })
    expect(e.messages).toHaveLength(1)
    expect(e.system.startsWith(p.system)).toBe(true)
    expect(e.system).toContain(p.systemHas)
    const facts = textOf(e.messages[0].content)
    for (const line of p.facts) expect(facts).toContain(line)

    // 4–5: the prep's file, --show, and the meeting's prep in --upcoming.
    expect(data.exists(data.home, t.folder, p.id, t.file)).toBe(true)
    expect(data.cli('prep', '--show', p.id).json()).toMatchObject({ summary: p.summary, expected: prep.expected, open: prep.open })
    expect(upcoming().prep).toMatchObject({ written_at: prep.written_at, error: null })

    // 6: the window.
    await meeting.open(p.id)
    await expect(meeting.header).toHaveText(p.subject)
    await expect(meeting.prepTab).toHaveAttribute('aria-selected', 'true')
    await expect(meeting.section(t.inShort)).toBeVisible()
    await expect(meeting.prep.getByText(p.summary, { exact: true })).toBeVisible()
    await expect(meeting.section(t.expected)).toBeVisible()
    for (const title of p.expected) await expect(meeting.prepItem(title)).toBeVisible()
    // The first comes with its date.
    await expect(meeting.prepRow(p.expected[p.dated])).toContainText(await meeting.shortDay(prep.expected[p.dated].due))
    await expect(meeting.prepItem(p.leftOut)).toHaveCount(0)
    await expect(meeting.section(t.open)).toBeVisible()
    for (const title of p.open) await expect(meeting.prepItem(title)).toBeVisible()
    await expect(meeting.prepFooter).toHaveText(fill(t.written, { time: await meeting.clock(prep.written_at), model: vm.model }))
    await expect(meeting.writeAgain).toBeVisible()
  })

  test('2 · Yolo on a task does it on its own, says the outcome above My work, sends nothing and changes only its task', covers('tasks/TC-026', 'F-TASK-12', 'F-CC-04'), async ({ page }) => {
    const { yolo: y, virtualModel: vm } = data.set
    const c = data.text.claude
    const run = data.text.minutesRun
    const ledger = path.join(data.home, 'projects', y.thread.id, data.text.work.ledger)
    data.model.use(y.script)
    data.model.clear()
    // Yolo runs as the folder session (#590): this script needs its tools without prompts, so the user's choice is on.
    await setActs(page, true)

    // 1: what the data folder and this checkout hold before; T005 open.
    const before = data.files()
    const git = gitStatus()
    expect(data.ledger(y.thread.id, 'tree').nodes[y.task.id].state).toBe(y.task.state)
    const events = lines(data.read(ledger)).length

    // 2: the task's Claude dialog.
    const today = new TodayPage(page, data)
    await today.open()
    await today.claudeFor(y.task.title).click()
    await expect(today.claudeDialog).toContainText(`${y.thread.name} · ${y.task.title}`)
    for (const skill of y.skills) await expect(today.skill(skill)).toBeVisible()
    await expect(today.claudeWorkOnIt).toBeVisible()
    await expect(today.claudeYolo).toBeVisible()
    await expect(today.claudeCancel).toBeVisible()
    await expect(today.claudeDialog.getByText(c.yoloLine)).toBeVisible()

    // 3: Yolo; the bar says Claude is on it (or already its outcome: the run takes seconds).
    await today.claudeYolo.click()
    await expect(today.claudeDialog).toBeHidden()
    const outcome = today.askSays(y.say)
    await expect(today.yoloWorking.or(outcome)).toBeVisible()

    // 4: the ask, done.
    const asks = () => (data.exists(data.home, c.asks) ? data.json(data.home, c.asks) : [])
    await expect.poll(() => asks()[0]?.state, { timeout: 60_000, intervals: [1_000] }).toBe(y.states.at(-1))
    expect(asks()[0]).toMatchObject({ title: y.task.title, state: y.states.at(-1), say: y.say })

    // 5: its outcome above My work, under it where the draft is, with a Dismiss button (no reload).
    await expect(outcome).toBeVisible()
    await expect(today.askSays(y.draftSays)).toBeVisible()
    await expect(today.dismiss).toBeVisible()
    await expect(today.yoloWorking).toBeHidden()

    // 6: the run's log — the three refusals, each after its step — and the mail, a draft.
    const yoloDir = path.join(data.env.ECHOCTL_CONFIG_DIR, c.yoloFolder)
    const asked = fs.readdirSync(yoloDir).filter((n) => n.startsWith(y.askFolder))
    expect(asked).toHaveLength(1)
    const [ask] = asked
    const log = lines(data.read(yoloDir, ask, c.log))
    expect(log[0]).toContain(y.logFirst)
    const bash = fill(c.tool, { tool: c.tools.bash })
    const steps = log.filter((l) => l.includes(bash))
    expect(steps).toHaveLength(y.bashSteps)
    y.bashOrder.forEach((words, k) => expect(steps[k]).toContain(words))
    const refused = log.map((l, i) => [l, i]).filter(([l]) => l.includes(run.logRefused))
    expect(refused).toHaveLength(y.refusals.length)
    y.refusals.forEach((r, k) => {
      const [line, i] = refused[k]
      expect(line).toContain(r.says)
      expect(log.slice(0, i).findLast((l) => l.includes(bash))).toContain(r.after)
    })
    expect(log.some((l) => l.includes(run.logOk))).toBe(true)
    const drafts = lines(data.read(data.dir, c.drafts))
    expect(drafts).toHaveLength(1)
    expect(drafts[0].startsWith(y.draft)).toBe(true)

    // 7: what the model was given (the task by its link only) and told back.
    const turns = data.model.turns(y.rule)
    expect(turns).toHaveLength(y.turns)
    expect(turns[0].model).toBe(vm.model)
    for (const tool of y.tools) expect(turns[0].tools).toContain(tool)
    const prompt = textOf(turns[0].messages[0].content)
    expect(prompt.split(y.preamble)).toHaveLength(2)
    for (const words of y.promptHas) expect(prompt).toContain(words)
    for (const words of y.promptHasNot) expect(prompt).not.toContain(words)
    expect(data.model.toolResults(y.rule).map(Boolean)).toEqual(y.toolResults)

    // 8: only the run's own files changed, and only T005's state in the ledger.
    const diff = changed(before, data.files())
    for (const f of diff) expect(y.changes.some((r) => new RegExp(r).test(f)), f).toBe(true)
    for (const f of y.mustChange) expect(diff).toContain(f)
    const added = lines(data.read(ledger)).slice(events).map((l) => JSON.parse(l))
    expect(added).toEqual(y.states.map((state) => expect.objectContaining({ op: 'edit', node: y.task.id, set: { state } })))

    // 9: nothing changed in this checkout.
    expect(gitStatus()).toBe(git)

    // 10: Dismiss; the done task leaves My work.
    await today.dismiss.click()
    await expect(outcome).toBeHidden()
    await expect(today.closeFor(y.task.title)).toHaveCount(0)

    // 11: the task's details: done, its owner, no description, the run's two state changes on top of History.
    const work = new WorkPage(page, data)
    await work.open(y.thread.id)
    await work.openItem(y.task.title)
    const w = data.text.work
    await expect(work.status).toHaveText(fill(w.status, { state: y.states.at(-1) }))
    await expect(work.value(w.owner)).toHaveText(y.task.owner)
    await expect(work.description).toBeVisible()
    // The run's two state changes: Echo's run's, not yours, each with its undo.
    for (const k of [0, 1]) {
      await expect(work.history.nth(k)).toContainText(fill(w.runSet, { field: w.fields.state }))
      await expect(work.history.nth(k).getByRole('button', { name: w.undo, exact: true })).toBeVisible()
    }
    // The choice back off, as the data had it.
    await setActs(page, false)
  })

  test("3 · Sync now reads the thread's new call into its work under the guard, verifies it in a fresh run, and keeps the fields the user set", covers('work/TC-028', 'F-THR-20', 'F-WORK-16', 'F-CC-04'), async ({ page }) => {
    const { sync: s, virtualModel: vm } = data.set
    const w = data.text.work
    const run = data.text.minutesRun
    const id = s.thread.id
    const { pinDue: due, pinOwner: owner } = s
    data.model.use(s.script)
    data.model.clear()

    // 1: the user sets T008's date and T003's owner: both pinned.
    for (const p of [due, owner]) {
      const r = data.ledger(id, 'edit', JSON.stringify({ op: 'edit', node: p.node, set: { [p.field]: p.value } }))
      expect(r).toMatchObject({ type: 'human', by: 'human', op: 'edit' })
    }
    let tree = data.ledger(id, 'tree')
    for (const p of [due, owner]) {
      expect(tree.nodes[p.node][p.field]).toBe(p.value)
      expect(tree.nodes[p.node].pinned).toContain(p.field)
    }

    // 2: all but Acme security review marked done by the user.
    const pending = data.ledger(id, 'pending')
    expect(pending.map((x) => x.title)).toEqual(s.pending)
    for (const x of pending.filter((x) => x.title !== s.read)) expect(data.ledger(id, 'done', '--evidence', x.id)).toEqual({ done: x.id })
    const left = data.ledger(id, 'pending')
    expect(left.map((x) => x.title)).toEqual([s.read])
    const evid = left[0].id
    const before = data.ledger(id, 'tree').nodes
    const last = data.ledger(id, 'status')
    const top = (k) => Math.max(...Object.keys(before).filter((n) => n.startsWith(k)).map((n) => Number(n.slice(1))))
    const next = (k) => k + String(top(k) + 1).padStart(3, '0')

    // 3: Sync now (T003's Handed over by read first, to compare after).
    const work = new WorkPage(page, data)
    await work.open(id)
    await work.openItem(owner.title)
    const handedOver = await work.value(data.text.work.handedOverBy).textContent()
    await work.syncNow.click()

    // 4: the sync's status: done, the three scripted reports.
    const status = () => data.ledger(id, 'status')
    await expect.poll(() => { const st = status(); return st && st.run !== last?.run && st.state !== 'running' ? st.state : null }, { timeout: 60_000, intervals: [1_000] }).toBe(s.state)
    const st = status()
    expect(st).toMatchObject({ processed: 1, left: 0, message: s.message })

    // 5: nothing pending; the run's folder.
    expect(data.ledger(id, 'pending')).toEqual([])
    const runDir = path.join(data.home, 'projects', id, w.runs, st.run)
    expect(fs.readdirSync(runDir)).toEqual(expect.arrayContaining(s.runFiles.map((f) => fill(f, { evid }))))

    // 6: two new items with the next ids, each with its words; T005's open claim verified.
    tree = data.ledger(id, 'tree').nodes
    const added = Object.keys(tree).filter((n) => !(n in before)).sort()
    expect(added).toEqual([next(s.ids.risk), next(s.ids.task)])
    for (const c of s.created) {
      const node = added.map((n) => tree[n]).find((n) => n.title === c.title)
      expect(node).toMatchObject({ ...c, by: 'agent', created_in: evid })
      expect(node.history[0]).toMatchObject({ by: 'agent', op: 'create', evidence: evid, quote: expect.any(String) })
    }
    const claim = (nodes) => nodes[s.claim.node].history.find((h) => h.op === s.claim.op && h.who === s.claim.who && h.what === s.claim.what)
    expect(claim(before).verdict).not.toBe('supports')
    expect(claim(tree).verdict).toBe('supports')

    // 7: the date and the owner the user set are kept; the agent's proposals are recorded, not applied.
    const t8 = tree[due.node]
    expect(t8[due.field]).toBe(due.value)
    expect(t8.pinned).toContain(due.field)
    expect(t8.history.at(-1)).toMatchObject({ by: 'agent', op: 'status', effect: s.due.effect, due: `${evid.slice(0, 4)}-${s.due.monthDay}`, verdict: 'supports', applied: false })
    const t3 = tree[owner.node]
    expect(t3[owner.field]).toBe(owner.value)
    expect(t3.pinned).toContain(owner.field)
    expect(t3.delegated_by).toBe(before[owner.node].delegated_by)
    expect(t3.history.find((h) => h.by === 'agent' && h.effect === s.reassign.effect)).toMatchObject({ internal_owner: s.reassign.to, verdict: 'supports', applied: false })

    // 8: the run's log: three runs; the guard refused only the other thread's context.
    const log = lines(data.read(runDir, data.text.claude.log))
    expect(log[0]).toContain(s.log.first)
    expect(log.filter((l) => l.includes(s.log.started))).toHaveLength(s.log.runs)
    const c = data.text.claude
    const tool = (name) => log.filter((l) => l.includes(fill(c.tool, { tool: name })))
    expect(tool(c.tools.bash).filter((l) => !l.includes(s.log.own))).toEqual([expect.stringContaining(s.log.other)])
    const refused = log.map((l, i) => [l, i]).filter(([l]) => l.includes(run.logRefused))
    expect(refused).toHaveLength(1)
    expect(refused[0][0]).toContain(s.log.refusal)
    expect(log[refused[0][1] - 1]).toContain(s.log.other)
    expect(log.filter((l) => l.includes(run.logNotAllowed))).toEqual([expect.stringContaining(s.log.notAllowed)])
    for (const l of tool(c.tools.write)) expect(l).toContain(path.join(data.home, 'projects', id, w.runs) + path.sep)
    for (const l of tool(c.tools.read)) expect(l).toContain(path.join(data.home, 'projects', id, 'ledger') + path.sep)
    expect(log.filter((l) => l.includes(run.logOk))).toHaveLength(s.log.runs)

    // 9: what each run was given (its own prompt and the sync's tools only) and told back.
    expect(s.turns.map((_, r) => data.model.turns(r).length)).toEqual(s.turns)
    expect([...new Set(data.model.requests().filter((e) => e.rule != null).map((e) => e.model))]).toEqual([vm.model])
    s.prompts.forEach((has, r) => {
      const [first] = data.model.turns(r).filter((e) => e.step === 0)
      expect([...new Set(first.tools)].filter((t) => !run.ignoredTools.includes(t)).sort()).toEqual(s.tools)
      const prompt = textOf(first.messages[0].content)
      for (const words of has) expect(prompt).toContain(words)
    })
    for (const e of data.model.turns(1)) expect(JSON.stringify(e.messages)).not.toContain(s.notToVerifier)
    expect(data.model.toolResults(0).map(Boolean)).toEqual(s.extractResults)

    // 10: the people pass: Priya Nair's role suggested and a fact added; Tom Becker's role, the user's, left alone.
    const suggestions = data.json(data.home, 'projects', id, w.suggestions)
    expect(suggestions).toContainEqual(expect.objectContaining({ field: 'role', value: s.person.role, state: 'pending', quote: expect.any(String), target: expect.objectContaining({ name: s.person.name }) }))
    expect(suggestions.filter((x) => x.field === 'role' && x.target?.name === s.userRole)).toEqual([])
    const priya = data.json(data.home, 'projects', w.people).find((p) => p.name === s.person.name)
    expect(priya.facts).toContainEqual(expect.objectContaining({ text: s.person.fact, evidence: evid }))
    const answer = JSON.stringify(data.model.turns(2).at(-1).messages.at(-1).content)
    for (const words of s.peopleApply) expect(answer).toContain(JSON.stringify(words).slice(1, -1))

    // 11: the window: synced, three to review; T008 and T003 still the user's, the agent's proposals "kept yours".
    await page.reload()
    await expect(work.syncLine(fill(w.synced, { time: await work.clock(st.finished_at), n: s.toReview }))).toBeVisible()
    await expect(work.syncNow).toBeEnabled()
    await work.openItem(due.title)
    await expect(work.value(w.due)).toHaveText(due.shown)
    await expect(work.history.nth(0)).toContainText(fill(w.dueText, { text: s.due.text }))
    await expect(work.history.nth(0)).toContainText(w.keptYours)
    await expect(work.history.nth(1)).toContainText(fill(w.youSet, { field: w.fields.due }))
    await expect(work.history.nth(1).getByRole('button', { name: w.undo, exact: true })).toBeVisible()
    await work.openItem(owner.title)
    await expect(work.value(w.owner)).toHaveText(owner.value)
    await expect(work.value(data.text.work.handedOverBy)).toHaveText(handedOver)
    await expect(work.history.nth(0)).toContainText(s.reassign.effect)
    await expect(work.history.nth(0)).toContainText(w.keptYours)
    await expect(work.history.nth(1)).toContainText(fill(w.youSet, { field: w.fields.internal_owner }))
  })

  test('4 · With the choice off, Yolo prepares and asks: its send refused, its mail drafted, its task closed as Echo\'s run', covers('tasks/TC-031', 'F-PLUG-35', 'F-M365-08'), async ({ page }) => {
    const a = data.set.yoloAsks
    const c = data.text.claude
    data.model.use(a.script)
    data.model.clear()
    await setActs(page, false)
    const drafted = data.exists(data.dir, c.drafts) ? lines(data.read(data.dir, c.drafts)).length : 0
    // Yolo on T009, as the page sends it.
    await page.evaluate((x) => fetch('ipc', { method: 'POST', body: JSON.stringify({ cmd: 'task_yolo', ask: x.ask, project: x.thread, node: x.task, skill: '', text: '' }) }), a)
    const ask = () => (data.exists(data.home, c.asks) ? data.json(data.home, c.asks) : []).find((x) => x.id === a.ask)
    await expect.poll(() => ask()?.state, { timeout: 60_000, intervals: [1_000] }).toBe('done')
    expect(ask().say).toBe(a.say)
    // The send never reached Echo (Claude Code refused it: nothing allowed it without a prompt); the draft did. Another
    // thread's work was refused: Yolo reads its own task's thread only, the user's choice on or off.
    expect(data.model.toolResults(0).map(Boolean)).toEqual(a.toolResults)
    const drafts = lines(data.read(data.dir, c.drafts))
    expect(drafts).toHaveLength(drafted + 1)
    expect(drafts.at(-1).startsWith(a.draft)).toBe(true)
    // Its record: what it asked, how it ran; what it changed and drafted, no send.
    const dir = path.join(data.env.ECHOCTL_CONFIG_DIR, c.yoloFolder, a.ask)
    expect(data.json(dir, 'permissions.json').used).toContain(a.used)
    const actions = lines(data.read(dir, 'actions.jsonl')).map((l) => JSON.parse(l))
    expect(actions.map((x) => x.kind)).toEqual(a.actions.map(([k]) => k))
    a.actions.forEach(([, cmd], i) => expect(actions[i].command.startsWith(cmd)).toBe(true))
    expect(actions.some((x) => x.kind === 'send')).toBe(false)
    expect(ask().changes.map((x) => x.kind)).toEqual(a.actions.map(([k]) => k))
    // The task closed, by Echo's run.
    expect(data.ledger(a.thread, 'tree').nodes[a.task].state).toBe(a.state)
    const last = lines(data.read(data.home, 'projects', a.thread, data.text.work.ledger)).map((l) => JSON.parse(l)).at(-1)
    expect(last).toMatchObject({ by: 'run', op: 'edit', node: a.task, set: { state: a.state } })
  })
})
