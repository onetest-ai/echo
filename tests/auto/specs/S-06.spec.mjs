// S-06 Shape the work — its stages with fixed checks (small data).
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { WorkPage } from '../pages/work.page.mjs'
import { TasksPage } from '../pages/tasks.page.mjs'
import { CallsPage } from '../pages/calls.page.mjs'

const data = useData(test, { set: 'small' })

/** A key of the window's shortcuts, by its name in texts.json. */
const key = (name) => data.text.shell.keys[name]
/** A ledger edit of a thread, as the CLI takes it. */
const edit = (thread, op) => data.ledger(thread, 'edit', JSON.stringify(op))
/** A task as `echoctl project tasks` gives it, from any pile. */
const task = (thread, id) => Object.values(data.cli('project', 'tasks').json().piles).flat().find((t) => t.project === thread && t.id === id)
/** Proposals applied through the agent's path, from a call's evidence (the stamp of its folder). */
const apply = (thread, call, proposals) => {
  const file = path.join(data.dir, `proposals-${thread}.json`)
  fs.writeFileSync(file, JSON.stringify(proposals))
  const evidence = data.json(data.dir, 'manifest.json').calls.find((c) => c.title === call).evidence
  return data.ledger(thread, 'apply', '--evidence', evidence, '--proposals', file)
}
/** The records stages 9–11 make (decision, risk, question), by kind: their ids. */
const records = {}

scenario(test, 'S-06', 'Shape the work', () => {
  test("1 · The board's columns and cards; Fields hides and shows the due chips; the board's menus close on Escape", covers('work/TC-004', 'F-WORK-04'), async ({ page }) => {
    const b = data.set.board
    const w = data.text.work
    edit(b.thread, { op: 'edit', node: b.t2.id, set: { due: b.t2.pastDue } })
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await expect(work.columnHeads).toHaveCount(b.columns.length)
    for (const [i, [name, n]] of b.columns.entries()) {
      await expect(work.columnHeads.nth(i)).toHaveAccessibleName(name)
      await expect(work.count(name)).toHaveText(String(n))
    }
    await expect(work.addColumn).toBeVisible()
    for (const t of [b.t1, b.t2]) {
      const card = work.card(t.title)
      await expect(card.getByText(`${t.id} · ${b.story.title}`, { exact: true })).toBeVisible()
      await expect(card.getByTitle(t.owner, { exact: true })).toHaveText(t.initials)
    }
    await expect(work.card(b.t1.title).getByTitle(w.dueTitle, { exact: true })).toHaveText(await work.day(task(b.thread, b.t1.id).due))
    await expect(work.card(b.t2.title).getByTitle(w.overdue, { exact: true })).toHaveText(b.t2.pastDueShown)
    const chips = work.main.getByTitle(w.dueTitle, { exact: true }).or(work.main.getByTitle(w.overdue, { exact: true }))

    await work.fields.click()
    for (const [name, on] of b.fields) await expect(work.field(name)).toBeChecked({ checked: on })
    await work.field(b.fields[3][0]).uncheck()
    await expect(chips).toHaveCount(0)
    await work.field(b.fields[3][0]).check()
    await expect(chips).toHaveCount(2)
    await page.keyboard.press(key('escape'))
    await expect(work.menu).toHaveCount(0)
    for (const opener of [work.columnMenu(b.columns[0][0]), work.viewOptions]) {
      await opener.click()
      await expect(work.menu).toBeVisible()
      await page.keyboard.press(key('escape'))
      await expect(work.menu).toHaveCount(0)
    }
    edit(b.thread, { op: 'edit', node: b.t2.id, set: { due: null } })
  })

  test('2 · Saved views: Board and Tree, a new view renamed, duplicated and deleted', covers('work/TC-001', 'F-WORK-01'), async ({ page }) => {
    const b = data.set.board
    const v = data.text.work.views
    const work = new WorkPage(page, data)
    const shown = async (...names) => {
      await expect.poll(() => work.viewCount()).toBe(names.length)
      for (const name of names) await expect(work.view(name)).toBeVisible()
    }
    await work.open(b.thread)
    await shown(v.board, v.tree)
    await expect(work.newView).toBeVisible()
    await expect(work.column(b.columns[0][0])).toBeVisible()
    await work.view(v.tree).click()
    await expect(work.text(fill(data.text.work.workstream, { thread: b.threadName }))).toBeVisible()
    await expect(work.view(v.tree)).toHaveAttribute('aria-current', 'true')
    await work.goto()
    await work.open(b.thread)
    await expect(work.view(v.tree)).toHaveAttribute('aria-current', 'true')

    await work.newView.click()
    await expect(work.view(v.third)).toHaveAttribute('aria-current', 'true')
    await expect(work.viewName).toBeVisible()
    for (const item of [v.duplicate, v.delete]) await expect(work.menuItem(item)).toBeVisible()
    await work.viewName.fill(b.newView)
    await work.viewName.press(key('enter'))
    await expect(work.view(b.newView)).toBeVisible()
    await work.view(b.newView).click()
    await work.menuItem(v.duplicate).click()
    const copy = fill(v.copy, { name: b.newView })
    await expect(work.view(copy)).toHaveAttribute('aria-current', 'true')
    await expect(work.viewName).toHaveValue(copy)
    await shown(v.board, v.tree, b.newView, copy)
    await work.menuItem(v.delete).click()
    await shown(v.board, v.tree, b.newView)
    await expect(work.view(v.board)).toHaveAttribute('aria-current', 'true')
    await expect(work.menu).toHaveCount(0)
    await work.view(b.newView).click()
    await expect(work.view(b.newView)).toHaveAttribute('aria-current', 'true')
    await work.view(b.newView).click()
    await work.menuItem(v.delete).click()
    await shown(v.board, v.tree)
    await expect(work.view(v.board)).toHaveAttribute('aria-current', 'true')
    await expect(work.column(b.columns[0][0])).toBeVisible()
  })

  test('3 · Dragging a card to a column sets its state, undone from History; Waiting takes no drop', covers('work/TC-005', 'F-WORK-05'), async ({ page }) => {
    const b = data.set.board
    const { counts: n, states } = b
    const w = data.text.work
    const [[todo], [doing], [waiting]] = b.columns
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await expect(work.count(todo)).toHaveText(n.both)
    for (const t of [b.t1, b.t2]) await expect(work.column(todo).getByText(t.title, { exact: true })).toBeVisible()
    await work.card(b.t2.title).dragTo(work.column(doing))
    await expect(work.count(todo)).toHaveText(n.one)
    await expect(work.count(doing)).toHaveText(n.one)
    await expect(work.column(doing).getByText(b.t2.title, { exact: true })).toBeVisible()
    await work.openItem(b.t2.title)
    await expect(work.history.nth(0)).toContainText(w.setColumnState)
    expect(task(b.thread, b.t2.id).state).toBe(states.inProgress)
    await work.history.nth(0).getByRole('button', { name: w.undo, exact: true }).click()
    await expect(work.column(todo).getByText(b.t2.title, { exact: true })).toBeVisible()
    await expect(work.count(doing)).toHaveText(n.none)
    // Waiting takes no drop: T001 stays to do.
    await work.card(b.t1.title).dragTo(work.column(waiting))
    await expect(work.count(todo)).toHaveText(n.both)
    await expect(work.count(waiting)).toHaveText(n.none)
    expect(task(b.thread, b.t1.id).state).toBe(states.open)
  })

  test("4 · Details edits Owner, Handed over by and Due; a person's box suggests Me and the thread's people, and takes any name", covers('work/TC-012', 'F-WORK-11'), async ({ page }) => {
    const p = data.set.work
    const w = data.text.work
    for (const person of p.people) expect(data.cli('project', 'person', p.thread, person).code).toBe(0)
    const work = new WorkPage(page, data)
    await work.open(p.thread)
    await work.openItem(p.item)
    // 1: as the data has them.
    await expect(work.value(w.owner)).toHaveText(p.before.owner)
    await expect(work.value(w.handedOverBy)).toHaveText(p.before.handedOver)
    await expect(work.value(w.due)).toHaveText(p.before.due)
    // 2: Owner.
    const owner = await work.editField(w.owner)
    await owner.fill(p.owner)
    await owner.press(key('enter'))
    await expect(work.value(w.owner)).toHaveText(p.owner)
    await expect(work.card(p.item).getByTitle(p.owner, { exact: true })).toHaveText(p.initials)
    // 3–4: Handed over by suggests Me then the thread's people, and takes a name it doesn't suggest.
    const handed = await work.editField(w.handedOverBy)
    expect(await work.suggestions()).toEqual([w.me, ...p.people])
    await handed.fill(p.otherName)
    await handed.press(key('enter'))
    await expect(work.value(w.handedOverBy)).toHaveText(p.otherName)
    // 5: Due.
    const due = await work.editField(w.due)
    await due.fill(p.due)
    await expect(work.value(w.due)).toHaveText(p.dueShown)
    await expect(work.card(p.item).getByTitle(w.dueTitle, { exact: true })).toHaveText(p.chip)
    // 6: History, newest first, each with undo.
    const fields = [w.fields.due, w.fields.delegated_by, w.fields.internal_owner]
    for (const [k, field] of fields.entries()) {
      await expect(work.history.nth(k)).toContainText(fill(w.youSet, { field }))
      await expect(work.history.nth(k).getByRole('button', { name: w.undo, exact: true })).toBeVisible()
    }

    // Teardown: the three undone, newest first; the people removed.
    for (const field of fields) await work.history.filter({ hasText: fill(w.youSet, { field }) }).first().getByRole('button', { name: w.undo, exact: true }).click()
    await expect(work.value(w.owner)).toHaveText(p.before.owner)
    await expect(work.value(w.due)).toHaveText(p.before.due)
    for (const person of p.people) expect(data.cli('project', 'person', p.thread, person, p.remove).code).toBe(0)
  })

  test('5 · Waits for and Holds up link the two tasks both ways; Copy link copies its echo:// link', covers('work/TC-016', 'F-WORK-14'), async ({ page }) => {
    const b = data.set.board
    const w = data.text.work
    edit(b.thread, { op: 'link', node: b.t2.id, depends_on: b.t1.id })
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await work.openItem(b.t2.title)
    await expect(work.detail(w.waitsFor)).toBeVisible()
    await expect(work.link(`${b.t1.id} ${b.t1.title}`)).toBeVisible()
    await expect(work.card(b.t1.title).getByText(fill(w.holdsUp, { n: 1 }), { exact: true })).toBeVisible()
    await work.openItem(b.t1.title)
    await expect(work.detail(w.holdsUpRow)).toBeVisible()
    await work.link(`${b.t2.id} ${b.t2.title}`).click()
    await expect(work.itemTitle).toHaveText(b.t2.title)
    await expect(work.copyLink).toHaveAccessibleDescription(b.link)
    edit(b.thread, { op: 'unlink', node: b.t2.id, depends_on: b.t1.id })
  })

  test('6 · Status changed in the details moves the card, shows in History and the CLI, and is undone', covers('work/TC-011', 'F-WORK-10'), async ({ page }) => {
    const b = data.set.board
    const w = data.text.work
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await work.openItem(b.t1.title)
    await expect(work.status).toHaveText(fill(w.status, { state: w.states.open }))
    await work.status.click()
    await work.statusBox.selectOption({ label: w.states.in_progress })
    await expect(work.status).toHaveText(fill(w.status, { state: w.states.in_progress }))
    await expect(work.column(b.columns[1][0]).getByText(b.t1.title, { exact: true })).toBeVisible()
    await expect(work.history.nth(0)).toContainText(fill(w.youSet, { field: w.fields.state }))
    expect(task(b.thread, b.t1.id).state).toBe(b.states.inProgress)
    await work.history.nth(0).getByRole('button', { name: w.undo, exact: true }).click()
    await expect(work.status).toHaveText(fill(w.status, { state: w.states.open }))
    expect(task(b.thread, b.t1.id).state).toBe(b.states.open)
  })

  test('7 · Details: open a task, add its description, rename it, and Escape changes nothing', covers('work/TC-010', 'F-WORK-09'), async ({ page }) => {
    const b = data.set.board
    const w = data.text.work
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await work.openItem(b.t1.title)
    await expect(work.panelTab(work.details, w.details)).toHaveAttribute('aria-selected', 'true')
    await expect(work.detail(b.t1.id)).toBeVisible()
    await expect(work.detail(w.task)).toBeVisible()
    await expect(work.sendToClaude).toBeVisible()
    await expect(work.edit).toBeVisible()
    await expect(work.detail(b.path)).toBeVisible()
    await expect(work.description).toBeVisible()
    await work.description.click()
    await work.descriptionBox.fill(b.description)
    await work.descriptionBox.press(key('save'))
    await expect(work.detail(b.description)).toBeVisible()
    await expect(work.history.nth(0)).toContainText(fill(w.youSet, { field: w.description }))
    await work.edit.click()
    await work.titleBox.fill(b.titleV2)
    await work.titleBox.press(key('enter'))
    await expect(work.itemTitle).toHaveText(b.titleV2)
    await expect(work.card(b.titleV2)).toBeVisible()
    await work.detail(b.description).click()
    await work.descriptionBox.press(key('escape'))
    await expect(work.detail(b.description)).toBeVisible()

    // Teardown: the title back, the description cleared.
    await work.edit.click()
    await work.titleBox.fill(b.t1.title)
    await work.titleBox.press(key('enter'))
    await expect(work.itemTitle).toHaveText(b.t1.title)
    await work.detail(b.description).click()
    await work.descriptionBox.fill('')
    await work.descriptionBox.press(key('save'))
    await expect(work.description).toBeVisible()
  })

  test('8 · Who does it: two icon buttons; By hand is saved and filters Tasks', covers('work/TC-014', 'F-WORK-12'), async ({ page }) => {
    const b = data.set.board
    const w = data.text.work
    const t = data.text.tasks
    expect(task(b.thread, b.t1.id).mode).toBeNull()
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await work.openItem(b.t1.title)
    const [claude, hand] = [w.claudeCan, w.byHand].map((name) => work.who.getByRole('button', { name, exact: true }))
    for (const button of [claude, hand]) await expect(button).toHaveText('')
    expect((await claude.boundingBox()).y).toBe((await hand.boundingBox()).y)
    await hand.click()
    await expect(hand).toHaveAttribute('aria-pressed', 'true')
    expect(task(b.thread, b.t1.id).mode).toMatchObject({ mode: b.modes.byHand })

    const tasks = new TasksPage(page, data)
    await tasks.open()
    await tasks.select(t.whoDoesIt).selectOption({ label: t.byHand })
    await expect(tasks.task(b.t1.title)).toBeVisible()
    await tasks.select(t.whoDoesIt).selectOption({ label: t.claudeCan })
    await expect(tasks.pile(t.iOwe, 0)).toBeVisible()
    await expect(tasks.task(b.t1.title)).toHaveCount(0)

    // Teardown: Anyone on Tasks; no one chosen for T001.
    await tasks.select(t.whoDoesIt).selectOption({ label: t.anyone })
    edit(b.thread, { op: 'edit', node: b.t1.id, set: { mode: null } })
    expect(task(b.thread, b.t1.id).mode).toEqual(b.modes.cleared)
  })

  test('9 · Filters: Kind and Owner list what the work holds; Owner and Group by narrow and group the board; Kind on the Tree', covers('work/TC-002', 'F-WORK-02'), async ({ page }) => {
    const b = data.set.board
    const w = data.text.work
    records.decision = edit(b.thread, { op: 'create', kind: 'decision', parent: b.story.id, title: b.decision }).id
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    for (const name of Object.values(w.selects)) await expect(work.select(name)).toBeVisible()
    await expect(work.openOnly).toHaveAttribute('aria-pressed', 'true')
    await expect(work.fields).toBeVisible()
    await expect(work.select(w.selects.kind).getByRole('option')).toHaveText(b.kinds)
    await expect(work.select(w.selects.owner).getByRole('option')).toHaveText(b.owners)
    await work.select(w.selects.owner).selectOption({ label: b.t2.owner })
    await expect(work.main.getByRole('listitem')).toHaveCount(1)
    await expect(work.card(b.t2.title)).toBeVisible()
    await expect(work.count(b.columns[0][0])).toHaveText(b.counts.one)
    await work.select(w.selects.owner).selectOption({ label: w.anyone })
    await work.select(w.selects.group).selectOption({ label: w.groupBy.story })
    await expect(work.main.getByRole('button', { name: `${b.story.id} ${b.story.title}`, exact: true })).toBeVisible()
    await expect(work.text(fill(w.progress, { met: 0, all: 1 }))).toBeVisible()
    for (const t of [b.t1, b.t2]) await expect(work.card(t.title)).toBeVisible()
    await work.select(w.selects.group).selectOption({ label: w.groupBy.owner })
    // A lane an owner, in order, each with its one card.
    for (const [i, t] of [b.t2, b.t1].entries()) {
      await expect(work.text(t.owner).filter({ visible: true })).toBeVisible()
      await expect(work.column(b.columns[0][0]).nth(i).getByRole('listitem')).toHaveCount(1)
      await expect(work.column(b.columns[0][0]).nth(i).getByText(t.title, { exact: true })).toBeVisible()
    }
    await work.select(w.selects.group).selectOption({ label: w.groupBy.none })
    await work.select(w.selects.layout).selectOption({ label: w.layouts.tree })
    await work.select(w.selects.kind).selectOption({ label: b.kinds[3] })
    await expect(work.treeRow(records.decision)).toBeVisible()
    await expect(work.treeRow(b.story.id)).toBeVisible()
    for (const t of [b.t1, b.t2]) await expect(work.treeRow(t.id)).toHaveCount(0)
    expect((await work.treeRow(b.story.id).boundingBox()).y).toBeLessThan((await work.treeRow(records.decision).boundingBox()).y)
    await work.select(w.selects.kind).selectOption({ label: b.kinds[0] })
    await work.select(w.selects.layout).selectOption({ label: w.layouts.board })
    for (const t of [b.t1, b.t2]) await expect(work.card(t.title)).toBeVisible()
  })

  test("10 · A story's criterion ticked on the Tree is done; a risk's likelihood and impact", covers('work/TC-013', 'F-WORK-11'), async ({ page }) => {
    const b = data.set.board
    const w = data.text.work
    records.risk = edit(b.thread, { op: 'create', kind: 'risk', parent: b.story.id, title: b.risk }).id
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await work.view(w.views.tree).click()
    await work.treeRow(b.story.id).click()
    await expect(work.detail(fill(w.doneWhen, { met: 0, all: 1 }))).toBeVisible()
    await expect(work.criterion(b.story.criterion)).not.toBeChecked()
    await work.criterion(b.story.criterion).check()
    await expect(work.detail(fill(w.doneWhen, { met: 1, all: 1 }))).toBeVisible()
    await expect(work.treeRow(b.story.id)).toContainText(fill(w.criteria, { met: 1, all: 1 }))
    await expect(work.status).toHaveText(fill(w.status, { state: w.states.done }))
    await expect(work.history.nth(0)).toContainText(fill(w.ticked, { n: 1 }))
    await work.treeRow(records.risk).click()
    await expect(work.detail(w.risk)).toBeVisible()
    for (const f of [w.likelihood, w.impact]) await expect(work.value(f)).toHaveText(w.notRated)
    await expect(work.link(new RegExp(`^${w.noResponse}`))).toBeVisible()
    for (const [f, level] of [[w.likelihood, b.likelihood], [w.impact, b.impact]]) {
      const box = await work.editField(f)
      await box.selectOption({ label: level })
      await expect(work.value(f)).toHaveText(level)
    }

    // Teardown (the risk and the Tree are kept for stage 11): the criterion unticked.
    await work.treeRow(b.story.id).click()
    await work.criterion(b.story.criterion).uncheck()
    await expect(work.status).toHaveText(fill(w.status, { state: w.states.open }))
  })

  test('11 · The Tree lists the story, its tasks and question indented, the decision and the risk at its level', covers('work/TC-007', 'F-WORK-07'), async ({ page }) => {
    const b = data.set.board
    const w = data.text.work
    records.question = edit(b.thread, { op: 'create', kind: 'question', parent: b.story.id, title: b.question }).id
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await work.view(w.views.tree).click()
    await expect(work.text(fill(w.workstream, { thread: b.threadName }))).toBeVisible()
    const row = work.treeRow.bind(work)
    await expect(row(records.question)).toBeVisible()
    for (const text of [b.story.title, fill(w.criteria, { met: 0, all: 1 }), fill(w.status, { state: w.states.open })]) await expect(row(b.story.id)).toContainText(text)
    for (const t of [b.t1, b.t2]) for (const text of [t.owner, fill(w.status, { state: w.states.open })]) await expect(row(t.id)).toContainText(text)
    for (const text of [w.question, b.question, w.openState]) await expect(row(records.question)).toContainText(text)
    for (const text of [w.decision, b.decision]) await expect(row(records.decision)).toContainText(text)
    await expect(row(records.decision)).not.toContainText(w.statusMark)
    for (const text of [w.risk, b.risk, w.noResponseState]) await expect(row(records.risk)).toContainText(text)
    const story = await work.indent(b.story.id)
    for (const id of [b.t1.id, b.t2.id, records.question]) expect(await work.indent(id)).toBeGreaterThan(story)
    for (const id of [records.decision, records.risk]) expect(await work.indent(id)).toBe(story)

    // Teardown: the three records deleted, the Board back.
    for (const id of Object.values(records)) edit(b.thread, { op: 'delete', node: id })
    await work.view(w.views.board).click()
    await expect(work.view(w.views.board)).toHaveAttribute('aria-current', 'true')
  })

  test("12 · A column's menu: a new column named and limited, Hide and back, Delete; Done can't be deleted", covers('work/TC-006', 'F-WORK-06'), async ({ page }) => {
    const g = data.set.globex
    const b = data.set.board
    const w = data.text.work
    const done = b.columns[4][0]
    const work = new WorkPage(page, data)
    await work.open(g.thread)
    await expect(work.columnHeads).toHaveCount(5)
    await expect(work.count(b.columns[0][0])).toHaveText(b.counts.one)
    await expect(work.card(g.t1.title)).toBeVisible()
    await work.addColumn.click()
    await expect(work.columnHeads).toHaveCount(6)
    const col = w.newColumn
    for (const box of [work.columnName(col), work.columnState(col), work.columnLimit(col)]) await expect(box).toBeVisible()
    for (const item of w.columnItems) await expect(work.columnItem(col, item)).toBeVisible()
    await work.columnName(col).fill(g.column.name)
    await page.keyboard.press(key('enter'))
    await expect(work.menu).toHaveCount(0)
    await work.columnMenu(g.column.name).click()
    await work.columnState(g.column.name).selectOption({ label: g.column.state })
    await work.columnLimit(g.column.name).fill(g.column.limit)
    await page.keyboard.press(key('enter'))
    await expect(work.count(g.column.name)).toHaveText(g.column.shown)
    await expect(work.menu).toHaveCount(0)

    const [, hide, del] = w.columnItems
    await work.columnMenu(done).click()
    // Both items take a click where they are drawn: the whole menu shows over the short board (#109).
    for (const item of [hide, del]) {
      await expect(work.columnItem(done, item)).toBeInViewport({ ratio: 1 })
      await work.columnItem(done, item).click({ trial: true, timeout: 2_000 })
    }
    await work.columnItem(done, hide).click()
    await expect(work.columnHead(done)).toHaveCount(0)
    await expect(work.unhide).toHaveText(fill(w.hidden, { n: 1 }))
    await work.unhide.click()
    await expect(work.columnHead(done)).toBeVisible()
    await work.columnMenu(g.column.name).click()
    await work.columnItem(g.column.name, del).click()
    await expect(work.columnHead(g.column.name)).toHaveCount(0)
    await expect(work.columnHeads).toHaveCount(5)
    await work.columnMenu(done).click()
    await work.columnItem(done, del).click()
    await expect(work.text(w.keepDone)).toBeVisible()
    await expect(work.columnHead(done)).toBeVisible()
  })

  test('13 · Link to an item in another thread: found by its words, linked as the same work, unlinked', covers('work/TC-017', 'F-WORK-15'), async ({ page }) => {
    const g = data.set.globex
    const w = data.text.work
    const work = new WorkPage(page, data)
    await work.open(g.thread)
    await work.openItem(g.t1.title)
    await work.linkOther.click()
    await expect(work.findOther).toHaveAttribute('placeholder', w.findPlaceholder)
    await expect(work.cancelLink).toBeVisible()
    await work.findOther.fill(g.search)
    await expect(work.details.getByRole('group').filter({ has: page.getByRole('button', { name: w.linkKinds[0] }) })).toHaveCount(g.results.length)
    for (const r of g.results) for (const kind of w.linkKinds) await expect(work.result(r).getByRole('button', { name: kind, exact: true })).toBeVisible()
    await expect(work.result(g.results[1])).toContainText(`${g.results[1]} · ${g.resultOwner}`)
    await work.result(g.results[1]).getByRole('button', { name: w.linkKinds[0], exact: true }).click()
    await expect(work.detail(w.sameAs)).toBeVisible()
    await expect(work.link(g.linked)).toBeVisible()
    await expect(work.link(w.unlink)).toBeVisible()
    await expect(work.history.filter({ hasText: g.linkedHistory })).toHaveCount(1)
    await work.link(w.unlink).click()
    await expect(work.detail(w.sameAs)).toHaveCount(0)
    await expect(work.history.filter({ hasText: g.unlinkedHistory })).toHaveCount(1)
  })

  test("14 · A synced task's History: created, verified, its quote and its source, which opens the call", covers('work/TC-015', 'F-WORK-13'), async ({ page }) => {
    const g = data.set.globex
    const w = data.text.work
    data.ledger(g.thread, 'sync')
    const applied = apply(g.thread, data.set.kickoff, g.proposal)
    const work = new WorkPage(page, data)
    await work.open(g.thread)
    await work.openItem(g.proposal[0].title)
    await expect(work.detail(g.path)).toBeVisible()
    const created = work.history.filter({ hasText: w.created })
    for (const text of [w.yesterday, data.text.work.me, w.verified, `“${g.proposal[0].quote}”`]) await expect(created).toContainText(text)
    const source = created.getByRole('button', { name: g.source, exact: true })
    await expect(source).toHaveAccessibleDescription(data.set.kickoff)
    await source.click()
    await expect.poll(() => work.hash()).toMatch(new RegExp(`-${data.set.calls.other.folder}/(minutes|transcript)$`))
    await expect(new CallsPage(page, data).heading(data.set.kickoff)).toBeVisible()

    // Teardown: the task deleted.
    for (const { id } of applied.created) edit(g.thread, { op: 'delete', node: id })
  })

  test("15 · The sync line: not synced yet, one call to read", covers('work/TC-003', 'F-WORK-03'), async ({ page }) => {
    const b = data.set.board
    const w = data.text.work
    data.ledger(b.thread, 'sync')
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await expect(work.syncLine(fill(w.notSynced, { n: 1 }))).toBeVisible()
    await expect(work.syncNow).toBeVisible()
    expect(data.ledger(b.thread, 'status')).toBeNull()
  })

  test('16 · To review: a near-duplicate merged with Yes', covers('work/TC-008', 'F-WORK-08'), async ({ page }) => {
    const b = data.set.board
    const r = data.set.review
    const w = data.text.work
    expect(apply(b.thread, data.set.calls.withMinutes.title, r.merge).by_op).toMatchObject({ suggest_merge: 1 })
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await expect(work.text(fill(w.nToReview, { n: 1 }))).toBeVisible()
    await expect(work.text(w.toReview)).toBeVisible()
    await expect(work.text(r.mergeBanner)).toBeVisible()
    await expect(work.no).toBeVisible()
    await expect(work.main.getByRole('listitem')).toHaveCount(3)
    await work.yes.click()
    await expect(work.text(w.toReview)).toHaveCount(0)
    await expect(work.text(fill(w.nToReview, { n: 1 }))).toHaveCount(0)
    await expect(work.main.getByRole('listitem')).toHaveCount(2)
    await expect(work.card(r.merge[0].title)).toHaveCount(0)
    const nodes = Object.values(data.ledger(b.thread, 'tree').nodes)
    expect(nodes.find((n) => n.title === r.merge[0].title).state).toBe(w.merged)
  })

  test('17 · To review: a near-duplicate kept with No', covers('work/TC-009', 'F-WORK-08'), async ({ page }) => {
    const b = data.set.board
    const r = data.set.review
    const w = data.text.work
    apply(b.thread, data.set.calls.withMinutes.title, r.reject)
    const work = new WorkPage(page, data)
    await work.open(b.thread)
    await expect(work.textStarting(r.rejectBanner)).toBeVisible()
    await expect(work.yes).toBeVisible()
    await work.no.click()
    await expect(work.text(w.toReview)).toHaveCount(0)
    await expect(work.card(r.reject[0].title)).toBeVisible()
    await expect(work.card(b.t2.title)).toBeVisible()
  })

  test('18 · A Due date typed in Details is saved once, as typed; Enter on an empty box saves nothing', covers('work/TC-030', 'F-WORK-11'), async ({ page }) => {
    const p = data.set.work
    const t = p.typed
    const w = data.text.work
    const events = () => data.read(data.home, 'projects', p.thread, 'ledger', 'events.jsonl').trim().split('\n')
    const work = new WorkPage(page, data)
    await work.open(p.thread)
    await work.openItem(p.item)
    await expect(work.value(w.due)).toHaveText(p.before.due)
    // History already has stage 1's and stage 4's "you set due": this stage adds one.
    const set = work.history.filter({ hasText: fill(w.youSet, { field: w.fields.due }) })
    const sets = await set.count()
    // 2: Enter on the empty box, then Escape: nothing.
    let before = events().length
    await work.editField(w.due)
    await page.keyboard.press(key('enter'))
    await page.keyboard.press(key('escape'))
    await expect(work.value(w.due)).toHaveText(p.before.due)
    expect(events().length).toBe(before)
    // 3: typed key by key into the box's first part: nothing written while typing.
    const box = await work.editField(w.due)
    await work.typeDate(box, t.keys)
    await expect(box).toHaveValue(t.due)
    expect(events().length).toBe(before)
    // 4: Enter: one edit, the date typed.
    await page.keyboard.press(key('enter'))
    await expect(work.value(w.due)).toHaveText(t.shown)
    const added = events().slice(before).map((l) => JSON.parse(l))
    expect(added.map((e) => [e.op, e.node, e.set])).toEqual([['edit', data.set.board.t2.id, { due: t.due }]])
    // 5: one more "you set due" in History, the newest.
    await expect(set).toHaveCount(sets + 1)
    // A year half typed (0002) is refused: nothing written, the date kept.
    before = events().length
    const again = await work.editField(w.due)
    await work.typeDate(again, t.halfYear)
    await page.keyboard.press(key('enter'))
    await expect(work.value(w.due)).toHaveText(t.shown)
    expect(events().length).toBe(before)
    // Teardown: undone.
    await set.first().getByRole('button', { name: w.undo, exact: true }).click()
    await expect(work.value(w.due)).toHaveText(p.before.due)
  })
})
