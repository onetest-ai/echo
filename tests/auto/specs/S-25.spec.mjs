// S-25 Who owes what on a full board — every stage (standard data): Tasks' piles on 57 open tasks (I owe with the
// blocked ones marked, Owed to me and Others, Waiting for others, Nobody named, no closed task anywhere), the due
// chips, the quiet marks, the order, the Who does it and Thread filters, Asks by person; then the threads' Work tabs: a
// board with custom columns and its filters, the sync line, where each card lands and what it shows, links across
// threads, column counts, the Tree's status words, what is shown nowhere, and the Owner filter that lists the user once.
// Stages only read, or change a view setting; as the scenario says, stages 19–21 leave "Open only" off in the Trees.
import { test, expect } from '@playwright/test'
import { useData, fill } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { TasksPage } from '../pages/tasks.page.mjs'
import { WorkPage } from '../pages/work.page.mjs'

const data = useData(test, { set: 'standard', ai: 'fake' })

/** `echoctl project tasks` as the data was built (stages 1–11 change nothing): read once, by the first stage. */
let built = null
const tasksCli = () => {
  const r = data.cli('project', 'tasks')
  if (r.code !== 0) throw new Error(`project tasks: ${r.err}`)
  return r.json()
}
/** A card's key, `<thread name> <id>`, as the data names them; and an entry's of the CLI's piles. */
const key = (c) => `${c.thread} ${c.id}`
const entryKey = (e) => `${Object.entries(data.set.s25.threadIds).find(([, id]) => id === e.project)[0]} ${e.id}`
/** The CLI's entry of a card, in the pile the window shows it in. */
const entry = (pile, c) => built.piles[data.text.tasks.cliPiles[pile]].find((e) => entryKey(e) === key(c))

/** A due chip's text: the date the CLI gives, with its weekday when it is today or in the coming days. */
async function dueText(tasks, pile, c) {
  if (!c.due) return data.text.tasks.noDate
  const due = entry(pile, c).due
  return ['today', 'tomorrow'].includes(c.due) ? tasks.day(due) : tasks.shortDay(due)
}

/** What a card of the data reads in a pile. */
async function reads(tasks, pile, c) {
  const t = data.text.tasks
  const s = data.set.s25
  const mode = c.mode && fill(t.modes[c.mode], { how: s.how })
  const quiet = c.quiet && fill(t.quietMark, { n: c.quiet })
  return tasks.reads({ title: c.title, line: c.line, mode, thread: c.thread, logo: s.logo.includes(c.thread), quiet, due: await dueText(tasks, pile, c) })
}

/** A card of I owe whose line says it is blocked. */
const isBlocked = (c) => c.line.split(' · ').some((part) => part.startsWith(data.text.tasks.blocked))

/** Tasks, once its piles are drawn. */
async function openTasks(page) {
  const tasks = new TasksPage(page, data)
  await tasks.open()
  await tasks.pileArea(data.text.tasks.iOwe).waitFor()
  return tasks
}

/** Every pile and Others unfolded (stages 1, 2 and 4 do it one by one in the manual scenario; each test here has its
 *  own page). */
async function unfoldAll(tasks) {
  const t = data.text.tasks
  const s = data.set.s25
  for (const [pile, n] of Object.entries(s.folds)) await tasks.fold(pile, fill(t.more, { n })).click()
  await tasks.others(s.others).click()
  for (const pile of Object.keys(s.folds)) await expect(tasks.fold(pile, t.showFewer)).toBeVisible()
  await expect(tasks.cards(t.others)).toHaveCount(s.others)
}

/** A list of names, cut in the groups given (in any order inside a group, group after group), each group sorted. */
const grouped = (names, groups) => {
  let i = 0
  const out = groups.map((g) => { const part = names.slice(i, i + g.length); i += g.length; return part.sort() })
  return i === names.length ? out : [...out, names.slice(i)]
}

/** The names of I owe's cards by their keys, for the order checks. */
async function iOweNames(tasks) {
  const t = data.text.tasks
  const names = {}
  for (const c of data.set.s25.cards[t.iOwe]) names[key(c)] = await reads(tasks, t.iOwe, c)
  return names
}

/** The Work tab of a thread on one of its views (a click on a view that isn't the active one). */
async function openWork(page, thread, view) {
  const work = new WorkPage(page, data)
  await work.open(thread)
  await work.views.waitFor()
  if ((await work.view(view).getAttribute('aria-current')) !== 'true') await work.view(view).click()
  await expect(work.view(view)).toHaveAttribute('aria-current', 'true')
  return work
}

/** "Open only" off in the view shown (a saved view setting). */
async function allItems(work) {
  if ((await work.openOnly.getAttribute('aria-pressed')) === 'true') await work.openOnly.click()
  await expect(work.openOnly).toHaveAttribute('aria-pressed', 'false')
}

/** A board's column heads, in order, and the count each shows. */
async function heads(work, expected) {
  await expect.poll(() => work.columnNames()).toEqual(expected.map(([name]) => name))
  for (const [name, n] of expected) await expect(work.count(name)).toHaveText(n)
}

/** The date a ledger item is due: its due as the board shows it (the weekday in the coming days). */
async function boardDue(work, thread, id, week) {
  const due = data.ledger(thread, 'tree').nodes[id].due
  return week ? work.day(due) : work.shortDay(due)
}

scenario(test, 'S-25', 'Who owes what on a full board: Tasks, piles, filters, badges, the board and its columns', () => {
  test('1 · I owe holds the user\'s 14 open tasks; the 7 blocked ones stay there, marked in red, saying by whom', covers('tasks/TC-015', 'F-TASK-01', 'F-TASK-02', 'F-TASK-06'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    built = tasksCli()
    const tasks = await openTasks(page)
    for (const [i, pile] of t.piles.entries()) await expect(tasks.pile(pile, s.counts[i])).toBeVisible()
    await expect(tasks.others(s.others)).toBeVisible()
    await expect(tasks.chip(t.allThreads)).toBeVisible()
    await expect(tasks.says(fill(t.across, { n: s.active }))).toBeVisible()

    await tasks.fold(t.iOwe, fill(t.more, { n: s.folds[t.iOwe] })).click()
    await expect(tasks.fold(t.iOwe, t.showFewer)).toBeVisible()
    await expect(tasks.cards(t.iOwe)).toHaveCount(s.counts[0])
    const cards = s.cards[t.iOwe]
    expect(cards).toHaveLength(s.counts[0])
    for (const c of cards) await expect(tasks.card(t.iOwe, c.title, c.thread)).toHaveAccessibleName(await reads(tasks, t.iOwe, c))

    // 4: the word "blocked" on 7 cards; the other 7 show only their story (their names above). Its red: "after"
    // below, the last test (it fails while the Tasks view draws the word grey).
    const blocked = cards.filter(isBlocked)
    expect(blocked).toHaveLength(s.blocked)
    await expect(tasks.blockedWords(t.iOwe)).toHaveCount(s.blocked)

    // 5: the CLI's piles.mine.
    const mine = built.piles[t.cliPiles[t.iOwe]]
    expect(mine.map(entryKey).sort()).toEqual(cards.map(key).sort())
    expect(mine.filter((e) => e.blocked).map(entryKey).sort()).toEqual(blocked.map(key).sort())
    expect(mine.every((e) => e.mine === true)).toBe(true)
  })

  test('2 · Another person\'s task is Owed to me when the user handed it over, else in Others (24, by thread)', covers('tasks/TC-018', 'F-TASK-01', 'F-TASK-02', 'F-TASK-06'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    const [, owedPile] = t.piles
    const tasks = await openTasks(page)
    await expect(tasks.pile(owedPile, s.counts[1])).toBeVisible()
    await expect(tasks.cards(owedPile)).toHaveCount(s.counts[1])
    await expect(tasks.more(owedPile)).toHaveCount(0)
    for (const c of s.cards[owedPile]) await expect(tasks.card(owedPile, c.title, c.thread)).toHaveAccessibleName(await reads(tasks, owedPile, c))

    // 3–5: Others, opened: its note, the 11 read, the count by thread.
    await expect(tasks.others(s.others)).toContainText(t.othersLine)
    await tasks.others(s.others).click()
    await expect(tasks.cards(t.others)).toHaveCount(s.others)
    for (const c of s.cards[t.others]) await expect(tasks.card(t.others, c.title, c.thread)).toHaveAccessibleName(await reads(tasks, t.others, c))
    for (const [thread, n] of Object.entries(s.othersPerThread)) await expect(tasks.cardsOf(t.others, thread)).toHaveCount(n)

    // 6: Omar's task, handed over by Dana Lee, isn't Owed to me.
    await expect(tasks.card(owedPile, s.notOwed.title, s.notOwed.thread)).toHaveCount(0)
    await expect(tasks.card(t.others, s.notOwed.title, s.notOwed.thread)).toHaveCount(1)

    // 7: the CLI.
    const { owed, others } = tasksCli().piles
    expect(owed).toHaveLength(s.counts[1])
    for (const e of owed) {
      expect(e.delegated_by).toBe(s.me[1])
      expect(s.me).not.toContain(e.owner)
    }
    expect(others).toHaveLength(s.others)
    expect(others.filter((e) => e.delegated_by === s.me[1])).toEqual([])
  })

  test('3 · Waiting for others holds the work the user\'s tasks wait for and a dashed card for each person they wait on', covers('tasks/TC-016', 'F-TASK-01', 'F-TASK-02', 'F-TASK-06'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    const waitPile = t.piles[2]
    const tasks = await openTasks(page)
    await expect(tasks.pile(waitPile, s.counts[2])).toBeVisible()
    await expect(tasks.cards(waitPile)).toHaveCount(s.counts[2])
    await expect(tasks.more(waitPile)).toHaveCount(0)
    for (const c of s.cards[waitPile]) {
      const card = tasks.card(waitPile, c.title, c.thread)
      await expect(card).toHaveAccessibleName(await reads(tasks, waitPile, c))
      // A person's card is drawn dashed; a task's is not.
      expect([c.title, await tasks.borderOf(card)]).toEqual([c.title, c.wait ? t.waitBorder : t.cardBorder])
    }

    // 3: the other "Unblock: …" tasks are in Others, none in Waiting for others.
    await tasks.others(s.others).click()
    for (const c of s.unblock) {
      await expect(tasks.card(t.others, c.title, c.thread)).toHaveCount(1)
      await expect(tasks.card(waitPile, c.title, c.thread)).toHaveCount(0)
    }
    // 4: no card for Kim Park.
    await expect(tasks.pileArea(waitPile)).not.toContainText(s.notWaiting[0])

    // 5: the CLI: two wait cards, each stopping the user's task.
    const waiting = tasksCli().piles[t.cliPiles[waitPile]]
    expect(waiting).toHaveLength(s.counts[2])
    expect(waiting.filter((e) => e.wait).map((e) => ({ who: e.who, thread: e.project, id: e.id, stops: e.blocks.map((b) => b.id) })))
      .toEqual(s.waitWho.map((w) => ({ ...w, stops: [w.id] })))
  })

  test('4 · Nobody named takes every open task without an owner, blocked or handed over', covers('tasks/TC-017', 'F-TASK-01', 'F-TASK-02', 'F-TASK-06'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    const nobody = t.piles[3]
    const tasks = await openTasks(page)
    await expect(tasks.pile(nobody, s.counts[3])).toBeVisible()
    await tasks.fold(nobody, fill(t.more, { n: s.folds[nobody] })).click()
    await expect(tasks.cards(nobody)).toHaveCount(s.counts[3])
    await expect(tasks.fold(nobody, t.showFewer)).toBeVisible()
    for (const c of s.cards[nobody]) await expect(tasks.card(nobody, c.title, c.thread)).toHaveAccessibleName(await reads(tasks, nobody, c))

    // 4: the blocked one late in red ("Overdue"), the waiting one due today, not red.
    const red = await tasks.themeColour(t.colours.late)
    const late = tasks.due(tasks.card(nobody, s.lateNobody.title, s.lateNobody.thread), t.overdue)
    await expect(late).toBeVisible()
    expect(await tasks.colourOf(late)).toBe(red)
    const today = tasks.due(tasks.card(nobody, s.todayNobody.title, s.todayNobody.thread), t.dueTitle)
    await expect(today).toBeVisible()
    expect(await tasks.colourOf(today)).not.toBe(red)

    // 5: the CLI.
    const pile = tasksCli().piles[t.cliPiles[nobody]]
    expect(pile).toHaveLength(s.counts[3])
    expect(pile.every((e) => e.owner === null)).toBe(true)
    expect(Object.fromEntries(pile.map((e) => [entryKey(e), e.delegated_by]))).toEqual(s.nobodyBy)
  })

  test('5 · Closed tasks (done, cancelled, merged, deleted) are in no pile; the Thread filter counts only open ones', covers('tasks/TC-019', 'F-TASK-01', 'F-TASK-02'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    const tasks = await openTasks(page)
    await unfoldAll(tasks)
    const shown = [...t.piles.map((_, i) => s.counts[i]), s.others]
    for (const [i, pile] of [...t.piles, t.others].entries()) await expect(tasks.cards(pile)).toHaveCount(shown[i])
    expect(shown.reduce((a, b) => a + b)).toBe(s.allCards)
    for (const c of s.closed) await expect(tasks.anyCard(c.title, c.thread)).toHaveCount(0)
    await expect(tasks.options(t.filters.thread)).toHaveText(s.threadOptions)
    for (const who of s.notWaiting) await expect(tasks.pileArea(t.piles[2])).not.toContainText(who)
    const inPiles = Object.values(tasksCli().piles).flat().map(entryKey)
    expect(s.closed.map(key).filter((k) => inPiles.includes(k))).toEqual([])
  })

  test('6 · The due chip: late in red (Overdue), a weekday for today and the coming days, the date later on, or "no date"', covers('tasks/TC-020', 'F-TASK-06', 'F-TASK-02'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    const tasks = await openTasks(page)
    await unfoldAll(tasks)
    const red = await tasks.themeColour(t.colours.late)
    for (const c of s.chips) {
      const card = tasks.card(c.pile, c.title, c.thread)
      await expect(card).toHaveCount(1)
      if (!c.due) {
        await expect(tasks.undated(card)).toBeVisible()
        await expect(tasks.dueOf(card)).toHaveCount(0)
        continue
      }
      const chip = tasks.due(card, c.due === 'late' ? t.overdue : t.dueTitle)
      await expect(chip).toHaveText(await dueText(tasks, c.pile, c))
      expect([c.title, (await tasks.colourOf(chip)) === red]).toEqual([c.title, c.due === 'late'])
    }
    await expect(tasks.lateChips).toHaveCount(s.lateChips)
    for (const chip of await tasks.lateChips.all()) expect(await tasks.colourOf(chip)).toBe(red)
    await expect(tasks.dueChips).toHaveCount(s.dated)
    await expect(tasks.noDates).toHaveCount(s.undated)
  })

  test('7 · The quiet mark only on undated tasks five days quiet, all of them Umbrella proposal\'s', covers('tasks/TC-021', 'F-TASK-06', 'F-TASK-03'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    const tasks = await openTasks(page)
    await unfoldAll(tasks)
    const wait = s.cards[t.piles[2]].find((c) => c.wait && c.quiet)
    for (const c of [...s.quiet, { ...wait, pile: t.piles[2] }]) {
      const mark = tasks.quiet(tasks.card(c.pile, c.title, c.thread))
      if (!c.quiet) {
        await expect(mark).toHaveCount(0)
        continue
      }
      await expect(mark).toHaveText(fill(t.quietMark, { n: c.quiet }))
      await expect(mark).toHaveAttribute('title', fill(t.quietTitle, { n: c.quiet }))
    }
    await expect(tasks.quietMarks).toHaveCount(s.quietMarks)
    await expect(tasks.quietIn(s.quietThread)).toHaveCount(s.quietMarks)
  })

  test('8 · Order on a full pile: Due (late, then by date, then the quietest), Quiet longest, Thread, and Due again', covers('tasks/TC-022', 'F-TASK-03', 'F-TASK-05'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    const o = t.orders
    const tasks = await openTasks(page)
    await tasks.fold(t.iOwe, fill(t.more, { n: s.folds[t.iOwe] })).click()
    await expect(tasks.chip(o.due)).toBeVisible()
    await expect(tasks.cards(t.iOwe)).toHaveCount(s.counts[0])
    const names = await iOweNames(tasks)
    const inOrder = async (groups) => {
      const want = groups.map((g) => g.map((k) => names[k]))
      await expect.poll(async () => grouped(await tasks.names(t.iOwe), want)).toEqual(want.map((g) => [...g].sort()))
    }
    await inOrder(s.orders.due)
    for (const [order, groups] of [[o.quiet, s.orders.quiet], [o.thread, s.orders.thread], [o.due, s.orders.due]]) {
      await tasks.select(t.filters.order).selectOption({ label: order })
      await expect(tasks.chip(order)).toBeVisible()
      await inOrder(groups)
    }
  })

  test('9 · Who does it on the full board: six marked by the user, Claude can do and By hand narrow I owe, with the Thread filter too', covers('tasks/TC-023', 'F-TASK-04', 'F-TASK-06'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    const tasks = await openTasks(page)
    await tasks.fold(t.iOwe, fill(t.more, { n: s.folds[t.iOwe] })).click()
    await expect(tasks.chip(t.anyone)).toBeVisible()
    await expect(tasks.cards(t.iOwe)).toHaveCount(s.counts[0])
    const byKey = Object.fromEntries(s.cards[t.iOwe].map((c) => [key(c), c]))
    await expect(tasks.modes(t.iOwe)).toHaveCount(s.modes.length)
    for (const k of s.modes.map(key)) {
      const c = byKey[k]
      await expect(tasks.mode(tasks.card(t.iOwe, c.title, c.thread))).toHaveAccessibleName(fill(t.modes[c.mode], { how: s.how }))
    }
    const names = await iOweNames(tasks)
    const only = async (keys, counts) => {
      for (const [i, pile] of t.piles.entries()) await expect(tasks.pile(pile, counts[i])).toBeVisible()
      await expect.poll(async () => (await tasks.names(t.iOwe)).sort()).toEqual(keys.map((k) => names[k]).sort())
      await expect(tasks.others()).toHaveCount(0)
    }
    await tasks.select(t.whoDoesIt).selectOption({ label: t.claudeCan })
    await expect(tasks.chip(t.claudeCan)).toBeVisible()
    await only(s.claudeCan, [s.claudeCan.length, 0, 0, 0])
    await tasks.select(t.whoDoesIt).selectOption({ label: t.byHand })
    await only(s.byHand, [s.byHand.length, 0, 0, 0])
    for (const pile of t.piles.slice(1)) await expect(tasks.pileArea(pile)).toContainText(t.nothing)
    await tasks.select(t.whoDoesIt).selectOption({ label: t.claudeCan })
    await tasks.select(t.filters.thread).selectOption({ label: s.both.thread })
    await only(s.both.cards, [s.both.cards.length, 0, 0, 0])
    await tasks.select(t.whoDoesIt).selectOption({ label: t.anyone })
    await tasks.select(t.filters.thread).selectOption({ label: t.allThreads })
    for (const [i, pile] of t.piles.entries()) await expect(tasks.pile(pile, s.counts[i])).toBeVisible()
    await expect(tasks.others(s.others)).toBeVisible()
  })

  test('10 · The Thread filter: each option counts its open tasks, the piles hold its cards; empty threads show four empty piles', covers('tasks/TC-024', 'F-TASK-04', 'F-TASK-01'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    const tasks = await openTasks(page)
    await unfoldAll(tasks)
    await expect(tasks.options(t.filters.thread)).toHaveText(s.threadOptions)
    for (const r of s.threadFilter.filter((x) => s.filterSteps.includes(x.option))) {
      await tasks.select(t.filters.thread).selectOption({ label: r.option })
      await expect(tasks.chip(r.name)).toBeVisible()
      for (const [i, pile] of t.piles.entries()) {
        await expect(tasks.pile(pile, r.counts[i])).toBeVisible()
        await expect(tasks.cards(pile)).toHaveCount(r.counts[i])
        if (!r.counts[i]) await expect(tasks.pileArea(pile)).toContainText(t.nothing)
      }
      if (r.others) await expect(tasks.cards(t.others)).toHaveCount(r.others)
      else await expect(tasks.others()).toHaveCount(0)
      expect([r.option, r.counts.reduce((a, b) => a + b) + r.others]).toEqual([r.option, r.cards])
      await expect(tasks.says(t.noTasks)).toHaveCount(0)
    }
    await tasks.select(t.filters.thread).selectOption({ label: t.allThreads })
    for (const [i, pile] of t.piles.entries()) await expect(tasks.cards(pile)).toHaveCount(s.counts[i])
    await expect(tasks.cards(t.others)).toHaveCount(s.others)
  })

  test('11 · Asks by person on the full board: who, when you meet, what to ask and why; it follows the Thread filter and opens a task', covers('tasks/TC-025', 'F-TASK-08', 'F-TASK-04'), async ({ page }) => {
    const s = data.set.s25
    const t = data.text.tasks
    const a = t.asks
    const tasks = await openTasks(page)
    try {
      await tasks.inbox.click()
      await tasks.panelTab(tasks.rightPanel, a.tab).click()
      for (const text of [a.head, a.when]) await expect(tasks.panelSays(text)).toBeVisible()
      await expect.poll(() => tasks.askerNames()).toEqual(s.askers)
      await expect(tasks.panelSays(a.empty)).toHaveCount(0)
      const said = {}
      for (const x of s.asks) {
        const why = x.due ? fill(t.askWhy.due, { due: await tasks.day(entry(t.piles[1], x).due) }) : x.why
        said[x.text] = fill(t.askWhy.item, { title: x.text, why })
      }
      const red = await tasks.themeColour(t.colours.late)
      for (const who of s.askers) {
        await expect(tasks.askWhen(who)).toHaveCount(1)
        await expect(tasks.asksOf(who)).toHaveText(s.asks.filter((x) => x.who === who).map((x) => said[x.text]))
        for (const reason of await tasks.reasons(who).all()) expect(await tasks.colourOf(reason)).not.toBe(red)
      }
      // 4–5: the Thread filter.
      const n = s.askNarrow
      await tasks.select(t.filters.thread).selectOption({ label: n.thread })
      await expect(tasks.asksOf(n.who)).toHaveText(n.asks.map((x) => said[x]))
      await expect(tasks.asker(n.gone)).toHaveCount(0)
      await tasks.select(t.filters.thread).selectOption({ label: s.askEmpty })
      await expect(tasks.panelSays(a.empty)).toBeVisible()
      await expect(tasks.askers).toHaveCount(0)
      // 6: a thing to ask opens its task's Details.
      await tasks.select(t.filters.thread).selectOption({ label: t.allThreads })
      const work = new WorkPage(page, data)
      await tasks.ask(s.askOpen.title).click()
      await expect(work.panelTab(work.details, data.text.work.details)).toHaveAttribute('aria-selected', 'true')
      await expect(work.itemTitle).toHaveText(s.askOpen.title)
      await expect(work.detail(s.askOpen.path)).toBeVisible()
    } finally {
      // Teardown: the panel closed.
      if (await tasks.closePanel(tasks.rightPanel).count()) await tasks.closePanel(tasks.rightPanel).click()
    }
    await expect(tasks.rightPanel).toHaveCount(0)
  })

  test('12 · A board of custom columns: a limit shown over, a hidden column, a card in the first column of its own state', covers('work/TC-019', 'F-WORK-04', 'F-WORK-06'), async ({ page }) => {
    const c = data.set.s25.work.custom
    const w = data.text.work
    const work = await openWork(page, c.thread, w.views.board)
    await heads(work, c.heads)
    await expect(work.addColumn).toBeVisible()
    await expect(work.unhide).toHaveText(fill(w.hidden, { n: c.hidden }))
    await expect(work.columnHead(c.notShown)).toHaveCount(0)
    expect(await work.colourOf(work.count(c.over))).toBe(await work.themeColour(w.colours.late))
    for (const [id, column] of c.cards) await expect(work.cardIn(column, id)).toHaveCount(1)
    await expect(work.cardIn(c.stored.column, c.stored.id)).toHaveCount(0)
    await expect(work.cardIn(c.stored.shown, c.stored.id)).toHaveCount(1)
  })

  test('13 · The Owner and Kind filters narrow every column, Done included', covers('work/TC-024', 'F-WORK-02', 'F-WORK-04'), async ({ page }) => {
    const { custom: c, filters: f } = data.set.s25.work
    const w = data.text.work
    const work = await openWork(page, c.thread, w.views.board)
    try {
      await heads(work, c.heads)
      await work.select(w.selects.owner).selectOption({ label: f.owner })
      await heads(work, f.ownerHeads)
      for (const [id, column] of f.ownerCards) await expect(work.cardIn(column, id)).toHaveCount(1)
      await work.select(w.selects.owner).selectOption({ label: w.anyone })
      await work.select(w.selects.kind).selectOption({ value: f.kind })
      await expect(work.cardsIn(f.done)).toHaveCount(f.doneTasks.length)
      for (const id of f.doneTasks) await expect(work.cardIn(f.done, id)).toHaveCount(1)
      for (const id of f.questions) await expect(work.cardById(id)).toHaveCount(0)
      await work.select(w.selects.kind).selectOption({ value: f.all })
      await heads(work, c.heads)
    } finally {
      // Teardown: no filter on the board.
      await work.select(w.selects.owner).selectOption({ label: w.anyone })
      await work.select(w.selects.kind).selectOption({ value: f.all })
    }
    await heads(work, c.heads)
  })

  test('14 · The sync line on four threads: synced with calls to read and To review, never synced, failed', covers('work/TC-027', 'F-WORK-03', 'F-WORK-08'), async ({ page }) => {
    const y = data.set.s25.work.sync
    const w = data.text.work
    const work = await openWork(page, y.synced.thread, w.views.board)
    const st = data.ledger(y.synced.thread, 'status')
    await expect(work.syncLine(fill(w.syncedRead, { time: await work.clock(st.finished_at || st.at), calls: y.synced.calls, n: y.synced.review }))).toBeVisible()
    const toReview = work.syncPart(fill(w.nToReview, { n: y.synced.review }))
    expect(await work.colourOf(toReview)).toBe(await work.themeColour(w.colours.toReview))
    await expect(work.syncNow).toBeEnabled()
    await expect(work.toReview).toHaveCount(y.review.length)
    for (const [i, r] of y.review.entries()) {
      await expect(work.toReview.nth(i)).toContainText(r.says)
      await expect(work.reviewButtons(i)).toHaveText(r.buttons)
    }
    for (const thread of y.never) {
      await work.open(thread)
      await expect(work.syncLine(w.notSyncedYet)).toBeVisible()
    }
    await work.open(y.failed.thread)
    const failed = work.syncFailed(y.failed.why)
    await expect(failed).toHaveText(w.syncFailed)
    expect(await work.colourOf(failed)).toBe(await work.themeColour(w.colours.late))
    await expect(work.main).not.toContainText(new RegExp(w.callsToRead))
  })

  test('15 · The default board: where each card lands; a blocked task stays in Blocked though it waits; nothing closed is a card', covers('work/TC-018', 'F-WORK-04', 'F-WORK-02'), async ({ page }) => {
    const g = data.set.s25.work.globex
    const w = data.text.work
    const work = await openWork(page, g.thread, w.views.board)
    await heads(work, g.heads)
    await expect(work.addColumn).toBeVisible()
    await expect(work.unhide).toHaveCount(0)
    for (const [id, column] of g.cards) await expect(work.cardIn(column, id)).toHaveCount(1)
    const q = data.set.s25.work.chips.cards.find((c) => c.id === g.question)
    expect(await work.reads(q.id)).toBe(q.reads)
    await expect(work.cardIn(g.doneWaiting.column, g.doneWaiting.id)).toHaveCount(1)
    await expect(work.chip(g.doneWaiting.id, g.doneWaiting.chip)).toHaveCount(0)
    for (const id of g.absent) await expect(work.cardById(id)).toHaveCount(0)
  })

  test('16 · A card shows its id and story, its owner\'s initials (or ?), its due (late in red), whom it waits on and what it holds up', covers('work/TC-025', 'F-WORK-04'), async ({ page }) => {
    const { chips: k } = data.set.s25.work
    const w = data.text.work
    const work = await openWork(page, k.thread, w.views.board)
    for (const c of k.cards) {
      await expect(work.cardIn(c.column, c.id)).toHaveCount(1)
      await expect(work.top(c.id, c.top)).toBeVisible()
      const due = c.reads.includes('{due}') ? await boardDue(work, k.thread, c.id, k.week.includes(c.id)) : ''
      expect(await work.reads(c.id)).toBe(fill(c.reads, { due }))
      if (c.avatar) await expect(work.avatar(c.id, c.avatar[1])).toHaveText(c.avatar[0])
    }
    const red = await work.themeColour(w.colours.late)
    const [late, today] = [k.cards.find((c) => c.late), k.cards.find((c) => c.id === k.today)]
    expect(await work.colourOf(work.dueChip(late.id, w.overdue))).toBe(red)
    await expect(work.dueChip(today.id, w.dueTitle)).toBeVisible()
    expect(await work.colourOf(work.dueChip(today.id, w.dueTitle))).not.toBe(red)
  })

  test('17 · Details show the links across threads: waits for, holds up, also in, same as; nothing is unlinked', covers('work/TC-026', 'F-WORK-15', 'F-WORK-14'), async ({ page }) => {
    const l = data.set.s25.work.links
    const w = data.text.work
    const route = (id) => fill(data.text.threads.routes.tab, { id, tab: data.text.threads.tabs.work })
    let work = await openWork(page, l.waits.thread, w.views.board)
    await work.openItem(l.waits.title)
    await expect(work.linkIn(l.waits.row, l.waits.link)).toBeVisible()
    await expect(work.linkRow(l.waits.row)).toContainText(l.waits.owner)
    await expect(work.linkIn(l.waits.row, w.unlink)).toBeVisible()
    await expect(work.detail(l.waits.waitingOn)).toBeVisible()
    await expect(work.detail(l.waits.waitingOnText)).toBeVisible()

    await work.linkIn(l.waits.row, l.waits.link).click()
    await expect.poll(() => work.hash()).toBe(route(l.holds.thread))
    await expect(work.itemTitle).toHaveText(l.holds.title)
    await expect(work.linkIn(l.holds.row, l.holds.link, l.holds.says)).toBeVisible()
    await expect(work.linkIn(l.holds.row, w.unlink, l.holds.says)).toBeVisible()

    work = await openWork(page, l.also.thread, w.views.board)
    await expect(work.cardIn(l.also.column, l.also.id)).toHaveCount(1)
    await work.openItem(l.also.title)
    await expect(work.linkIn(l.also.row, l.also.link)).toBeVisible()
    await expect(work.linkIn(l.also.row, w.unlink)).toBeVisible()
    await work.linkIn(l.also.row, l.also.link).click()
    await expect.poll(() => work.hash()).toBe(route(l.same.thread))
    await expect(work.itemTitle).toHaveText(l.same.title)
    await expect(work.linkIn(l.same.row, l.same.link)).toBeVisible()
    await expect(work.linkRow(l.same.row)).toContainText(l.same.says)
    await expect(work.linkIn(l.same.row, w.unlink)).toBeVisible()
    expect(data.ledger(l.same.thread, 'tree').nodes[l.same.id].mirror_of).toMatchObject(l.same.master)
  })

  test('18 · The column counts of four threads, the cards outside To do, and no closed task on any board', covers('work/TC-020', 'F-WORK-04'), async ({ page }) => {
    const n = data.set.s25.work.counts
    const w = data.text.work
    for (const thread of n.order) {
      const work = await openWork(page, thread, w.views.board)
      await heads(work, n.heads[thread])
      for (const [column, ids] of Object.entries(n.cards[thread] || {})) {
        await expect(work.cardsIn(column)).toHaveCount(ids.length)
        for (const id of ids) await expect(work.cardIn(column, id)).toHaveCount(1)
      }
      for (const id of n.absent[thread] || []) await expect(work.cardById(id)).toHaveCount(0)
      if (thread === n.todo.thread) await expect(work.cardIn(n.todo.column, n.todo.id)).toHaveCount(1)
    }
  })

  test('19 · The Tree\'s words for questions and risks: answered, open, handled, no response, cancelled, done; a decision has none', covers('work/TC-021', 'F-WORK-07'), async ({ page }) => {
    const r = data.set.s25.work.records
    const w = data.text.work
    for (const thread of r.order) {
      const work = await openWork(page, thread, w.views.tree)
      await allItems(work)
      for (const [id, word, cls] of r.rows[thread]) {
        await expect(work.treeStatus(id)).toHaveText(fill(w.statusWord, { word }))
        expect([id, await work.colourOf(work.treeStatus(id))]).toEqual([id, await work.themeColour(w.colours[cls])])
      }
      if (thread === r.decisions.thread) {
        for (const id of r.decisions.ids) {
          await expect(work.treeRow(id)).toBeVisible()
          await expect(work.treeStatus(id)).toHaveCount(0)
        }
      }
    }
  })

  test('20 · Merged and deleted items show nowhere: not in the Kind counts, the Tree or the board; the Owner filter', covers('work/TC-023', 'F-WORK-02', 'F-WORK-07'), async ({ page }) => {
    const x = data.set.s25.work.nowhere
    const w = data.text.work
    let work = await openWork(page, x.thread, w.views.tree)
    await allItems(work)
    await expect(work.options(w.selects.kind)).toHaveText(x.kinds)
    await expect(work.treeRows.first()).toBeVisible()
    for (const id of x.ids) await expect(work.treeRow(id)).toHaveCount(0)
    work = await openWork(page, x.thread, w.views.board)
    await expect(work.columnHeads.first()).toBeVisible()
    for (const id of x.ids) await expect(work.cardById(id)).toHaveCount(0)
    await expect(work.options(w.selects.owner)).toHaveText(x.owners)
  })

  test('21 · The Tree\'s words for tasks and stories (waiting, blocked, held, cancelled…), their tags, and no header for an empty workstream', covers('work/TC-022', 'F-WORK-07'), async ({ page }) => {
    const it = data.set.s25.work.items
    const w = data.text.work
    for (const thread of it.order) {
      const work = await openWork(page, thread, w.views.tree)
      await allItems(work)
      await expect(work.text(fill(w.workstream, { thread: it.stream }))).toBeVisible()
      for (const title of it.emptyStreams[thread] || []) await expect(work.text(fill(w.workstream, { thread: title }))).toHaveCount(0)
      for (const row of it.rows[thread]) {
        if (!row.shown) {
          await expect(work.treeRow(row.id)).toHaveCount(0)
          continue
        }
        await expect(work.treeRow(row.id)).toBeVisible()
        for (const tag of w.tags) await expect(work.treeTag(row.id, tag)).toHaveCount(tag === row.tag ? 1 : 0)
        if (!row.word) {
          await expect(work.treeStatus(row.id)).toHaveCount(0)
          continue
        }
        await expect(work.treeStatus(row.id)).toHaveText(fill(w.statusWord, { word: row.word }))
        expect([row.id, await work.colourOf(work.treeStatus(row.id))]).toEqual([row.id, await work.themeColour(w.colours[row.class])])
      }
      if (thread === it.holdsUp.thread) await expect(work.treeTag(it.holdsUp.id, fill(w.holdsUp, { n: it.holdsUp.n }))).toBeVisible()
    }
  })

  test('22 · The Owner filter lists the user once, as "Me (Sam Rivera)", and keeps every item of theirs on the board and the Tree', covers('work/TC-029', 'F-WORK-02'), async ({ page }) => {
    const m = data.set.s25.work.mine
    const w = data.text.work
    let work = await openWork(page, m.thread, w.views.board)
    try {
      await expect(work.options(w.selects.owner)).toHaveText(data.set.s25.work.nowhere.owners)
      for (const name of m.notOptions) await expect(work.option(w.selects.owner, name)).toHaveCount(0)
      await work.select(w.selects.owner).selectOption({ label: m.me })
      await expect(work.chipReads(m.me)).toBeVisible()
      await expect(work.allCards).toHaveCount(m.open.length + m.done.length)
      for (const id of m.open) {
        await expect(work.cardById(id)).toHaveCount(1)
        await expect(work.cardIn(m.doneColumn, id)).toHaveCount(0)
      }
      for (const id of m.done) await expect(work.cardIn(m.doneColumn, id)).toHaveCount(1)
      for (const id of [...m.open, ...m.done]) await expect(work.avatar(id, m.owner[id])).toHaveText(m.initials[m.owner[id]])

      work = await openWork(page, m.thread, w.views.tree)
      await allItems(work)
      await work.select(w.selects.owner).selectOption({ label: m.me })
      await expect(work.treeTasks).toHaveCount(m.tree.length)
      for (const id of m.tree) await expect(work.treeRow(id)).toContainText(m.owner[id])
      for (const id of m.stories) await expect(work.treeRow(id)).toBeVisible()
    } finally {
      // Teardown: the Tree's Owner on Anyone and "Open only" on again, the Board's Owner on Anyone.
      work = await openWork(page, m.thread, w.views.tree)
      await work.select(w.selects.owner).selectOption({ label: w.anyone })
      if ((await work.openOnly.getAttribute('aria-pressed')) !== 'true') await work.openOnly.click()
      work = await openWork(page, m.thread, w.views.board)
      await work.select(w.selects.owner).selectOption({ label: w.anyone })
    }
    await expect(work.chipReads(w.owner)).toBeVisible()
    work = await openWork(page, m.thread, w.views.tree)
    await expect(work.openOnly).toHaveAttribute('aria-pressed', 'true')
    await expect(work.chipReads(w.owner)).toBeVisible()
  })

  // tasks/TC-015's step 4, its last check, run last: the other stages only read, and they go on while it fails.
  test('after · On I owe\'s cards the word "blocked" is red (tasks/TC-015 step 4, #313)', covers('tasks/TC-015', 'F-TASK-01', 'F-TASK-02', 'F-TASK-06'), async ({ page }) => {
    const t = data.text.tasks
    const tasks = await openTasks(page)
    await tasks.fold(t.iOwe, fill(t.more, { n: data.set.s25.folds[t.iOwe] })).click()
    await expect(tasks.blockedWords(t.iOwe)).toHaveCount(data.set.s25.blocked)
    const red = await tasks.themeColour(t.colours.late)
    // All of them at once, retried: a poll's redraw may replace the cards between a lookup and a read.
    await expect.poll(() => tasks.blockedWords(t.iOwe).evaluateAll((els) => els.map((e) => getComputedStyle(e).color))).toEqual(Array(data.set.s25.blocked).fill(red))
  })
})
