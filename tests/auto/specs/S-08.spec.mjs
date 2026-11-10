// S-08 Who owes what — its stages with fixed checks (small data).
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { TasksPage } from '../pages/tasks.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'
import { WorkPage } from '../pages/work.page.mjs'

const data = useData(test, { set: 'small' })

/** A key of the window's shortcuts, by its name in texts.json. */
const key = (name) => data.text.shell.keys[name]
/** A ledger edit of a thread, as the CLI takes it. */
const edit = (thread, op) => data.ledger(thread, 'edit', JSON.stringify(op))
/** `echoctl project tasks`: its piles. */
const piles = () => data.cli('project', 'tasks').json().piles
/** A card's text, by the title it starts with. */
const starts = (title) => new RegExp(`^\\s*${literal(title)}`)
/** The date two days ago, `YYYY-MM-DD` (this computer's day). */
const twoDaysAgo = () => {
  const d = new Date(Date.now() - 2 * 86_400_000)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}
/** What stages leave for later ones: T003 (10 → 11), the close's mid and the tasks marked done (12 → 14). */
const kept = { skillTask: null, closed: null }

/** Tasks, once its piles are drawn. */
async function openTasks(page) {
  const tasks = new TasksPage(page, data)
  await tasks.open()
  await tasks.pileArea(data.text.tasks.iOwe).waitFor()
  return tasks
}

scenario(test, 'S-08', "Who owes what: the piles, the filters, a card, closing a task, Claude's dialog, Asks", () => {
  test('1 · The piles and their counts; Others folded; the CLI holds the same piles', covers('tasks/TC-001', 'F-TASK-01', 'F-TASK-02'), async ({ page }) => {
    const { acme, globex, other, active } = data.set.tasks
    const t = data.text.tasks
    const tasks = await openTasks(page)
    await expect(tasks.heading(t.heading)).toBeVisible()
    await expect(tasks.says(fill(t.across, { n: active }))).toBeVisible()
    await expect(tasks.chosen(t.filters.thread, t.allThreads)).toHaveCount(1)
    await expect(tasks.chosen(t.whoDoesIt, t.anyone)).toHaveCount(1)
    await expect(tasks.chosen(t.filters.order, t.orders.due)).toHaveCount(1)
    const counts = data.set.tasks.counts.start
    for (const [i, name] of t.piles.entries()) {
      await expect(tasks.pile(name, counts[i])).toBeVisible()
      if (i) await expect(tasks.pileArea(name)).toContainText(t.nothing)
    }
    await expect(tasks.cards(t.iOwe)).toHaveCount(counts[0])
    for (const title of [acme.title, globex.title]) await expect(tasks.cards(t.iOwe).filter({ hasText: new RegExp(`^\\s*${literal(title)}`) })).toHaveCount(1)
    await expect(tasks.others(1)).toContainText(t.othersLine)
    await expect(tasks.task(other.title)).toHaveCount(0)
    await tasks.others(1).click()
    for (const text of [other.owner, other.thread]) await expect(tasks.task(other.title)).toContainText(text)
    await tasks.others(1).click()
    await expect(tasks.task(other.title)).toHaveCount(0)
    const p = piles()
    expect(p.mine.map((x) => x.title).sort()).toEqual([acme.title, globex.title].sort())
    expect(p.others.map((x) => x.title)).toEqual([other.title])
    for (const k of data.set.tasks.emptyPiles) expect(p[k]).toEqual([])
  })

  test('2 · Asks with nothing owed: its empty line and its footer, nobody listed', covers('tasks/TC-002', 'F-TASK-08'), async ({ page }) => {
    const a = data.text.tasks.asks
    const tasks = await openTasks(page)
    await tasks.inbox.click()
    await tasks.panelTab(tasks.rightPanel, a.tab).click()
    for (const text of [a.head, a.when, a.empty, a.foot]) await expect(tasks.rightPanel.getByText(text, { exact: true })).toBeVisible()
    await expect(tasks.rightPanel.getByRole('group')).toHaveCount(0)
    await tasks.closePanel(tasks.rightPanel).click()
  })

  test('3 · The Thread filter narrows the piles and Others', covers('tasks/TC-003', 'F-TASK-04'), async ({ page }) => {
    const { acme, globex, threads } = data.set.tasks
    const t = data.text.tasks
    const tasks = await openTasks(page)
    await expect(tasks.pile(t.iOwe, 2)).toBeVisible()
    await expect(tasks.others(1)).toBeVisible()
    await expect(tasks.select(t.filters.thread).getByRole('option')).toHaveText(threads)
    await tasks.select(t.filters.thread).selectOption({ label: globex.option })
    await expect(tasks.chosen(t.filters.thread, globex.option)).toHaveCount(1)
    await expect(tasks.chip(globex.thread)).toBeVisible()
    await expect(tasks.pile(t.iOwe, 1)).toBeVisible()
    await expect(tasks.cards(t.iOwe)).toHaveText([starts(globex.title)])
    await expect(tasks.others()).toHaveCount(0)
    await tasks.select(t.filters.thread).selectOption({ label: acme.option })
    await expect(tasks.pile(t.iOwe, 1)).toBeVisible()
    await expect(tasks.cards(t.iOwe)).toHaveText([starts(acme.title)])
    await expect(tasks.others(1)).toBeVisible()
    await tasks.select(t.filters.thread).selectOption({ label: t.allThreads })
    await expect(tasks.pile(t.iOwe, 2)).toBeVisible()
    await expect(tasks.others(1)).toBeVisible()
  })

  test("4 · A card opens its task's Details beside Tasks, the thread and the id paired", covers('tasks/TC-004', 'F-TASK-07'), async ({ page }) => {
    const { acme, globex } = data.set.tasks
    const t = data.text.tasks
    const w = data.text.work
    const tasks = await openTasks(page)
    const work = new WorkPage(page, data)
    await tasks.task(globex.title).click()
    expect(tasks.hash()).toBe(data.text.shell.routes.tasks)
    await expect(work.panelTab(work.details, w.details)).toHaveAttribute('aria-selected', 'true')
    await expect(tasks.task(globex.title)).toHaveAttribute('aria-current', 'true')
    await expect(work.itemTitle).toHaveText(globex.title)
    await expect(work.detail(globex.path)).toBeVisible()
    await expect(work.value(w.owner)).toHaveText(globex.owner)
    await expect(work.value(w.due)).toContainText(globex.due)
    await expect(work.status).toHaveText(globex.status)
    await tasks.task(acme.title).click()
    await expect(work.itemTitle).toHaveText(acme.title)
    await expect(work.detail(acme.path)).toBeVisible()
    await expect(work.value(w.owner)).toHaveText(acme.owner)
    await expect(work.sendToClaude).toBeVisible()
    await expect(work.edit).toBeVisible()
    await expect(work.detail(t.link)).toBeVisible()
    await expect(work.copyLink).toBeVisible()
    expect(tasks.hash()).toBe(data.text.shell.routes.tasks)
    await work.closePanel(work.details).click()
  })

  test('5 · A card shows its story or owner, its thread, and its due state: late, due, or no date', covers('tasks/TC-005', 'F-TASK-06'), async ({ page }) => {
    const { acme, globex, other } = data.set.tasks
    const t = data.text.tasks
    // Globex pilot's T001 overdue (kept for stage 6).
    edit(globex.project, { op: 'edit', node: globex.id, set: { due: twoDaysAgo() } })
    const tasks = await openTasks(page)
    await expect(tasks.pile(t.iOwe, 2)).toBeVisible()
    const late = tasks.task(globex.title)
    for (const text of [globex.story, globex.thread]) await expect(late).toContainText(text)
    await expect(late.getByTitle(t.overdue, { exact: true })).toBeVisible()
    const due = tasks.task(acme.title)
    for (const text of [acme.story, acme.thread]) await expect(due).toContainText(text)
    await expect(due.getByTitle(t.dueTitle, { exact: true })).toBeVisible()
    await expect(due.getByTitle(t.overdue, { exact: true })).toHaveCount(0)
    await tasks.others(1).click()
    for (const text of [other.owner, other.thread, t.noDate]) await expect(tasks.task(other.title)).toContainText(text)
    await expect(tasks.main).not.toContainText(new RegExp(t.quiet))
    await tasks.others(1).click()
  })

  test('6 · Order: Due puts the late task first, Thread by thread name, Quiet longest keeps the two', covers('tasks/TC-006', 'F-TASK-03'), async ({ page }) => {
    const { acme, globex } = data.set.tasks
    const t = data.text.tasks
    const o = t.orders
    const tasks = await openTasks(page)
    const order = (...titles) => expect(tasks.cards(t.iOwe)).toHaveText(titles.map(starts))
    await expect(tasks.chip(o.due)).toBeVisible()
    await order(globex.title, acme.title)
    await tasks.select(t.filters.order).selectOption({ label: o.thread })
    await expect(tasks.chip(o.thread)).toBeVisible()
    await order(acme.title, globex.title)
    await tasks.select(t.filters.order).selectOption({ label: o.quiet })
    await expect(tasks.chip(o.quiet)).toBeVisible()
    await expect(tasks.pile(t.iOwe, 2)).toBeVisible()
    for (const title of [acme.title, globex.title]) await expect(tasks.task(title)).toBeVisible()
    await tasks.select(t.filters.order).selectOption({ label: o.due })
    await order(globex.title, acme.title)

    // Teardown: T001 back to no date.
    edit(globex.project, { op: 'edit', node: globex.id, set: { due: null } })
  })

  test("7 · A task handed over and late is Owed to me, not Others", covers('tasks/TC-007', 'F-TASK-02'), async ({ page }) => {
    const { other } = data.set.tasks
    const t = data.text.tasks
    // Omar's task handed over by Sam and late (kept for stage 8).
    edit(other.project, { op: 'edit', node: other.id, set: { delegated_by: other.handedBy, due: twoDaysAgo() } })
    const tasks = await openTasks(page)
    await expect(tasks.pile(t.iOwe, 2)).toBeVisible()
    for (const [i, n] of data.set.tasks.counts.handedOver.entries()) await expect(tasks.pile(t.piles[i + 1], n)).toBeVisible()
    await expect(tasks.cards(t.piles[1])).toHaveCount(1)
    const card = tasks.cards(t.piles[1]).first()
    for (const text of [other.title, other.owner, other.thread]) await expect(card).toContainText(text)
    await expect(card.getByTitle(t.overdue, { exact: true })).toBeVisible()
    await expect(tasks.others()).toHaveCount(0)
    const p = piles()
    expect(p.owed).toEqual([expect.objectContaining({ title: other.title, late: true })])
    expect(p.others).toEqual([])
  })

  test('8 · Asks names who owes you, follows the Thread filter, and opens the task', covers('tasks/TC-008', 'F-TASK-08'), async ({ page }) => {
    const { other, globex } = data.set.tasks
    const t = data.text.tasks
    const a = t.asks
    const tasks = await openTasks(page)
    await expect(tasks.pile(t.piles[1], 1)).toBeVisible()
    await tasks.inbox.click()
    await tasks.panelTab(tasks.rightPanel, a.tab).click()
    await expect(tasks.rightPanel.getByText(a.head, { exact: true })).toBeVisible()
    const asker = tasks.asker(other.owner)
    await expect(asker).toContainText(a.noMeeting)
    await expect(asker.getByRole('button')).toHaveCount(1)
    await expect(asker.getByRole('button')).toHaveText(new RegExp(`^${fill(a.wasDue, { title: other.title })}`))
    await expect(tasks.rightPanel.getByText(a.empty)).toHaveCount(0)
    await tasks.select(t.filters.thread).selectOption({ label: globex.option })
    await expect(tasks.asker(other.owner)).toHaveCount(0)
    await expect(tasks.rightPanel.getByText(a.empty)).toBeVisible()
    await tasks.select(t.filters.thread).selectOption({ label: t.allThreads })
    await tasks.ask(fill(a.wasDue, { title: other.title })).click()
    const work = new WorkPage(page, data)
    await expect(work.panelTab(work.details, data.text.work.details)).toHaveAttribute('aria-selected', 'true')
    await expect(work.itemTitle).toHaveText(other.title)
    await expect(work.detail(data.set.tasks.acme.path)).toBeVisible()

    // Teardown: Omar's task back in Others; the panel closed.
    edit(other.project, { op: 'edit', node: other.id, set: { delegated_by: null, due: null } })
    await work.closePanel(work.details).click()
  })

  test('9 · Who does it: By hand set in the Details filters the piles; Claude can do shows none', covers('tasks/TC-009', 'F-TASK-04'), async ({ page }) => {
    const { acme, globex } = data.set.tasks
    const t = data.text.tasks
    const tasks = await openTasks(page)
    await expect(tasks.pile(t.iOwe, 2)).toBeVisible()
    const none = async () => { for (const name of t.piles) await expect(tasks.pileArea(name)).toContainText(t.nothing) }
    await tasks.select(t.whoDoesIt).selectOption({ label: t.byHand })
    await expect(tasks.chip(t.byHand)).toBeVisible()
    await none()
    await tasks.select(t.whoDoesIt).selectOption({ label: t.anyone })
    await tasks.task(globex.title).click()
    const work = new WorkPage(page, data)
    const hand = work.who.getByRole('button', { name: data.text.work.byHand, exact: true })
    await hand.click()
    await expect(hand).toHaveAttribute('aria-pressed', 'true')
    await expect(work.detail(t.how)).toBeVisible()
    await expect(work.value(t.howField)).toContainText(t.notSaid)
    await tasks.select(t.whoDoesIt).selectOption({ label: t.byHand })
    await expect(tasks.pile(t.iOwe, 1)).toBeVisible()
    await expect(tasks.cards(t.iOwe)).toHaveText([starts(globex.title)])
    await expect(tasks.task(globex.title).getByLabel(t.byHandMark, { exact: true })).toBeVisible()
    await expect(tasks.task(acme.title)).toHaveCount(0)
    await tasks.select(t.whoDoesIt).selectOption({ label: t.claudeCan })
    await none()
    await tasks.select(t.whoDoesIt).selectOption({ label: t.anyone })
    await expect(tasks.pile(t.iOwe, 2)).toBeVisible()

    // Teardown: no mode on T001; the panel closed.
    edit(globex.project, { op: 'edit', node: globex.id, set: { mode: null } })
    await work.closePanel(work.details).click()
  })

  test("10 · Send to Claude: the dialog picks the fitting skill; Escape folds the list then closes; Cancel and leaving close it; nothing starts", covers('tasks/TC-010', 'F-TASK-10'), async ({ page }) => {
    const { skillTask: s } = data.set.tasks
    const t = data.text.tasks
    // A task whose words fit the data's one skill (kept for stage 11, which cancels it).
    kept.skillTask = edit(s.project, { op: 'create', kind: 'task', parent: s.parent, title: s.title, internal_owner: s.owner }).id
    const tasks = await openTasks(page)
    await expect(tasks.cards(t.iOwe).filter({ hasText: s.title })).toHaveCount(1)
    await tasks.task(s.title).click()
    const work = new WorkPage(page, data)
    const today = new TodayPage(page, data)
    const claude = today.claudeDialog
    await work.sendToClaude.click()
    await expect(claude.getByRole('heading', { name: data.text.needs.claudeDialog, level: 3 })).toBeVisible()
    await expect(claude).toContainText(fill(t.dialogLine, { thread: data.set.tasks.acme.thread, title: s.title }))
    await expect(today.claudeWorkOnIt).toBeVisible()
    await expect(today.claudeCancel).toBeVisible()
    await expect(today.claudeYolo).toHaveCount(0)
    await expect(claude).toContainText(s.skill)
    await expect(today.noSkill).toBeVisible()
    await today.noSkill.click()
    await expect(today.claudeSkill).toHaveAttribute('placeholder', t.skillPlaceholder)
    if (!(await today.skills.isVisible())) await today.claudeSkill.click()
    await expect(today.skills.getByRole('option')).toHaveCount(1)
    await expect(today.skills.getByRole('option')).toContainText(t.fitsBest)
    await page.keyboard.press(key('escape'))
    await expect(claude).toBeVisible()
    await expect(claude.getByText(t.fitsBest)).toHaveCount(0)
    await today.claudeSkill.click()
    await expect(claude.getByText(t.fitsBest)).toBeVisible()
    await page.keyboard.press(key('escape'))
    await expect(claude.getByText(t.fitsBest)).toHaveCount(0)
    await expect(claude).toBeVisible()
    await page.keyboard.press(key('escape'))
    await expect(claude).toHaveCount(0)
    await expect(tasks.panelTab(tasks.bottomPanel, t.claudePanel)).toHaveCount(0)
    await work.sendToClaude.click()
    await today.claudeCancel.click()
    await expect(claude).toHaveCount(0)
    await work.sendToClaude.click()
    await expect(claude).toBeVisible()
    await tasks.goto(data.text.shell.routes.today)
    await expect(tasks.text(data.text.shell.yourDay)).toBeVisible()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await expect(tasks.panelTab(tasks.bottomPanel, t.claudePanel)).toHaveCount(0)
  })

  test('11 · A pile of seven shows six and "+ 1"; it unfolds and folds again', covers('tasks/TC-011', 'F-TASK-05'), async ({ page }) => {
    const { fold: f } = data.set.tasks
    const t = data.text.tasks
    const made = []
    for (let n = 1; piles().mine.length < f.total; n++) made.push(edit(f.project, { op: 'create', kind: 'task', parent: f.parent, title: fill(f.title, { n }), internal_owner: f.owner }).id)
    const tasks = await openTasks(page)
    const more = fill(t.more, { n: f.total - f.shown })
    await expect(tasks.pile(t.iOwe, f.total)).toBeVisible()
    await expect(tasks.cards(t.iOwe)).toHaveCount(f.shown)
    await expect(tasks.fold(t.iOwe, more)).toBeVisible()
    await tasks.fold(t.iOwe, more).click()
    await expect(tasks.cards(t.iOwe)).toHaveCount(f.total)
    await expect(tasks.fold(t.iOwe, t.showFewer)).toBeVisible()
    await tasks.fold(t.iOwe, t.showFewer).click()
    await expect(tasks.cards(t.iOwe)).toHaveCount(f.shown)
    await expect(tasks.fold(t.iOwe, more)).toBeVisible()

    // Teardown: the tasks made here and stage 10's T003 cancelled.
    for (const id of [...made, kept.skillTask].filter(Boolean)) edit(f.project, { op: 'edit', node: id, set: { state: data.set.tasks.states.cancelled } })
  })

  test("12 · Close a task from Today: Cancel keeps it; Close it with a comment takes it out of Today and Tasks", covers('tasks/TC-012', 'F-TASK-13'), async ({ page }) => {
    const { acme, globex, close } = data.set.tasks
    const t = data.text.tasks
    const today = new TodayPage(page, data)
    await today.goto(data.text.shell.routes.today)
    await expect(today.closeFor(acme.title)).toBeVisible()
    await today.closeFor(acme.title).click()
    await expect(today.closeDialog.getByText(acme.title, { exact: true })).toBeVisible()
    await expect(today.closeNote).toBeVisible()
    await expect(today.closeCancel).toBeVisible()
    await expect(today.closeIt).toBeVisible()
    await today.closeCancel.click()
    await expect(today.closeDialog).toHaveCount(0)
    await expect(today.closeFor(acme.title)).toBeVisible()
    await today.closeFor(acme.title).click()
    await today.closeNote.fill(close.note)
    await today.closeIt.click()
    await expect(today.closeDialog).toHaveCount(0)
    await expect(today.closeFor(acme.title)).toHaveCount(0)
    const node = data.ledger(acme.project, 'tree').nodes[acme.id]
    expect(node.state).toBe(data.set.tasks.states.done)
    expect(node.history.at(-1)).toMatchObject({ op: 'edit', set: { state: data.set.tasks.states.done }, note: close.note })
    kept.closed = node.history.at(-1).mid
    const tasks = await openTasks(page)
    await expect(tasks.pile(t.iOwe, 1)).toBeVisible()
    await expect(tasks.cards(t.iOwe)).toHaveText([starts(globex.title)])
  })

  test('13 · No active threads: Tasks says so and opens Threads', covers('tasks/TC-013', 'F-TASK-09'), async ({ page }) => {
    const t = data.text.tasks
    const threads = [data.set.tasks.acme.project, data.set.tasks.globex.project]
    for (const id of threads) expect(data.cli('project', 'set', id, '--stage', data.set.tasks.stages.closed).code).toBe(0)
    const tasks = new TasksPage(page, data)
    await tasks.open()
    await expect(tasks.says(fill(t.across, { n: 0 }))).toBeVisible()
    for (const text of [t.noThreads, t.noThreadsLine]) await expect(tasks.says(text)).toBeVisible()
    await tasks.main.getByRole('button', { name: t.openThreads, exact: true }).click()
    await expect.poll(() => tasks.hash()).toMatch(new RegExp(`^(${data.text.clients.routes.projects}|${data.text.shell.routes.project}/)`))

    // Teardown: both threads active again.
    for (const id of threads) expect(data.cli('project', 'set', id, '--stage', data.set.tasks.stages.active).code).toBe(0)
  })

  test('14 · No open tasks: Tasks says so, with no pile', covers('tasks/TC-014', 'F-TASK-09'), async ({ page }) => {
    const t = data.text.tasks
    const { acme, states } = data.set.tasks
    // Every open task marked done (stage 12's T001 is already: its teardown undoes the close).
    const done = Object.values(piles()).flat().map((x) => [x.project, x.id])
    for (const [project, id] of done) edit(project, { op: 'edit', node: id, set: { state: states.done } })
    expect(Object.values(piles()).flat()).toEqual([])
    const tasks = new TasksPage(page, data)
    await tasks.open()
    await expect(tasks.heading(t.heading)).toBeVisible()
    for (const name of Object.values(t.filters)) await expect(tasks.select(name)).toBeVisible()
    for (const text of [t.noTasks, t.noTasksLine]) await expect(tasks.says(text)).toBeVisible()
    await expect(tasks.main.getByRole('heading', { name: new RegExp(`^${t.iOwe} `) })).toHaveCount(0)

    // Teardown: each reopened; stage 12's T001 by undoing its close (its case's teardown, with the mid it noted).
    for (const [project, id] of done) edit(project, { op: 'edit', node: id, set: { state: states.open } })
    if (kept.closed) edit(acme.project, { op: 'undo', target: kept.closed })
    expect(data.ledger(acme.project, 'tree').nodes[acme.id].state).toBe(states.open)
  })
})
