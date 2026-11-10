// S-05 Start and run a thread for a client — every stage (small data): the Threads list by client and its folds, Found
// for you empty, a new thread from + and from a call, a thread's header and menu, its Profile (meetings and sources,
// organisations, call rules, since, notes, colour, AI, stage, client), membership computed from rules and pins, the
// Calls tab (take out, unpin), the Brief, a rename with the CLI and a delete that keeps the calls. Each stage leaves the
// data as it found it.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { ClientsPage } from '../pages/clients.page.mjs'

const data = useData(test, { set: 'small' })

/** A key of the window's shortcuts, by its name in texts.json. */
const key = (name) => data.text.shell.keys[name]
/** `echoctl …` that must succeed, its output read as JSON. */
const cli = (...args) => {
  const r = data.cli(...args)
  if (r.code !== 0) throw new Error(`${args.join(' ')}: ${r.err}`)
  return r.json()
}
const show = (id) => cli('project', 'show', id)
/** The ids of the threads a call belongs to (`echoctl project of`). */
const threadsOf = (suffix) => cli('project', 'of', data.callName(suffix)).projects.map((p) => p.id)
/** A thread's route on a tab. */
const route = (id, tab) => fill(data.text.threads.routes.tab, { id, tab })

/** The rows the left list shows under a client's group, until the next group. */
function under(lines, group, groups) {
  const rest = lines.slice(lines.indexOf(group) + 1)
  const end = rest.findIndex((l) => groups.includes(l))
  return end < 0 ? rest : rest.slice(0, end)
}

scenario(test, 'S-05', 'Start and run a thread for a client: create it, brief, rules, calls, rename, stage, close', () => {
  test('1 · The Threads list groups the open threads under their client; a client folds, stays folded after a reload, and its name opens it', covers('threads/TC-001', 'F-THR-01', 'F-THR-02'), async ({ page }) => {
    const s = data.set.s05
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    try {
      await clients.goto(route(s.openOn.id, s.openOn.tab))
      await expect(clients.listHeading(t.threads)).toBeVisible()
      const lines = await clients.listLines()
      expect(lines.filter((l) => s.groups.includes(l))).toEqual(s.groups)
      for (const g of s.groups) expect([g, under(lines, g, s.groups)]).toEqual([g, s.under[g]])
      await expect(clients.fold(s.own)).toHaveCount(0)
      for (const g of s.groups.filter((x) => x !== s.own)) await expect(clients.fold(g)).toHaveAccessibleName(t.hide)

      await clients.fold(s.fold.group).click()
      await expect(clients.threadRow(s.fold.row)).toHaveCount(0)
      await expect(clients.foldCount(s.fold.group, s.fold.count)).toBeVisible()
      await expect(clients.fold(s.fold.group)).toHaveAccessibleName(t.show)
      await clients.goto(route(s.openOn.id, s.openOn.tab))
      await expect(clients.fold(s.fold.group)).toHaveAccessibleName(t.show)
      await expect(clients.threadRow(s.fold.row)).toHaveCount(0)
      await clients.fold(s.fold.group).click()
      await expect(clients.threadRow(s.fold.row)).toBeVisible()
      await expect(clients.fold(s.fold.group)).toHaveAccessibleName(t.hide)
      await clients.group(s.fold.group).click()
      await expect.poll(() => clients.hash()).toBe(fill(data.text.clients.routes.client, { id: s.fold.client, tab: 'overview' }))
    } finally {
      // Teardown: Acme unfolded (the fold is kept in this browser only).
      if ((await clients.fold(s.fold.group).getAttribute('aria-label').catch(() => t.hide)) === t.show) await clients.fold(s.fold.group).click()
    }
  })

  test('2 · Found for you with nothing found: no count, its explanation, the new thread row, no suggestion', covers('threads/TC-017', 'F-THR-03', 'F-THR-18'), async ({ page }) => {
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    await clients.goto(t.routes.projects)
    await expect(clients.foundForYou).toHaveAccessibleName(t.found)
    await expect(clients.activityCount(t.threads)).toHaveCount(0)
    await clients.foundForYou.click()
    await expect.poll(() => clients.hash()).toBe(t.routes.found)
    await expect(clients.heading(t.found)).toBeVisible()
    await expect(clients.says(t.foundLine)).toBeVisible()
    await expect(clients.newThreadName).toBeVisible()
    await expect(clients.create).toBeVisible()
    await expect(clients.paragraph(t.foundNothing)).toHaveText(t.foundNothing)
    for (const name of t.foundAnswers) await expect(clients.button(name)).toHaveCount(0)
  })

  test('3 · New thread (+) makes a thread with the chosen client and opens its Profile', covers('threads/TC-003', 'F-THR-04'), async ({ page }) => {
    const { hiring: h, clientOptions, groups } = data.set.s05
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    try {
      await clients.goto(t.routes.projects)
      await clients.addThread.click()
      await expect.poll(() => clients.hash()).toBe(t.routes.found)
      await expect(clients.heading(t.found)).toBeVisible()
      await expect(clients.newThreadName).toHaveAttribute('placeholder', t.newThreadPlaceholder)
      await expect(clients.options(clients.threadClient)).toHaveText(clientOptions)
      await expect(clients.create).toBeVisible()
      await clients.newThreadName.fill(h.name)
      await expect(clients.newThreadName).toHaveValue(h.name)
      await clients.threadClient.selectOption({ label: h.client })
      await expect(clients.chosen(clients.threadClient, h.client)).toHaveCount(1)
      await clients.newThreadName.press(key('enter'))
      await expect.poll(() => clients.hash()).toBe(route(h.id, t.tabs.profile))
      await expect(clients.heading(h.name)).toBeVisible()
      await expect(clients.clientTag).toHaveText(h.client)
      await expect(clients.fact(h.name, data.set.s05.acme.stage)).toBeVisible()
      await expect(clients.fact(h.name, h.facts)).toBeVisible()
      expect(under(await clients.listLines(), groups[2], groups)).toEqual([h.row])
      expect(show(h.id)).toMatchObject({ name: h.name, client: h.clientId, stage: h.stage })
    } finally {
      // Teardown: the thread deleted.
      data.cli('project', 'delete', h.id)
    }
    await clients.goto(t.routes.projects)
    await expect(clients.threadRow(h.name)).toHaveCount(0)
    expect(cli('project', 'list').projects.map((p) => p.id)).not.toContain(h.id)
  })

  test("4 · A thread's header shows its client, stage, counts and five tabs; Thread actions lists four items and Escape closes it", covers('threads/TC-002', 'F-THR-08', 'F-THR-09'), async ({ page }) => {
    const a = data.set.s05.acme
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    const at = route(a.id, t.tabs.brief)
    await clients.goto(at)
    await expect(clients.heading(a.name)).toBeVisible()
    await expect(clients.clientTag).toHaveText(a.client)
    await expect(clients.fact(a.name, a.stage)).toBeVisible()
    await expect(clients.fact(a.name, a.facts)).toBeVisible()
    await expect(clients.tabs).toHaveText(a.tabs)
    await expect(clients.tab(t.tabs.briefTab)).toHaveAttribute('aria-selected', 'true')
    await clients.threadActions.click()
    await expect(clients.menuItems).toHaveText(a.menu)
    await page.keyboard.press(key('escape'))
    await expect(clients.menuItems).toHaveCount(0)
    expect(clients.hash()).toBe(at)
  })

  test('5 · Profile → Meetings and Sources say what is and is not read when there is nothing to read', covers('threads/TC-016', 'F-THR-16', 'F-THR-17'), async ({ page }) => {
    const a = data.set.s05.acme
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    await clients.openThread(a.id, t.tabs.profile)
    await expect(clients.rowSays(t.rows.meetings, t.noSeries)).toBeVisible()
    await expect(clients.row(t.rows.sources)).toContainText(t.brainOff)
    await expect(clients.paragraph(t.footStart)).toHaveText(fill(t.foot, { id: a.id }))
  })

  test('6 · Profile → Organisations adds a counterpart organisation and × unlinks it', covers('threads/TC-012', 'F-THR-13'), async ({ page }) => {
    const { acme: a, org: o } = data.set.s05
    const t = data.text.threads
    const r = t.rows
    const clients = new ClientsPage(page, data)
    try {
      await clients.openThread(a.id, t.tabs.profile)
      await expect(clients.rowButtons(r.orgs)).toHaveText([t.addOrg])
      await expect(clients.row(r.orgs)).toContainText(t.orgHint)
      await clients.rowButton(r.orgs, t.addOrg).click()
      await expect(clients.options(clients.rowSelect(r.orgs, t.organisation))).toHaveText(o.options)
      await clients.rowSelect(r.orgs, t.organisation).selectOption({ label: o.option })
      await expect(clients.token(r.orgs, o.token)).toBeVisible()
      await expect(clients.rowButton(r.orgs, t.remove)).toBeVisible()
      await clients.rowButton(r.orgs, t.remove).click()
      await expect(clients.token(r.orgs, o.token)).toHaveCount(0)
      await expect(clients.rowButtons(r.orgs)).toHaveText([t.addOrg])
    } finally {
      // Teardown: Globex unlinked if a check failed with it linked.
      if (await clients.rowButton(r.orgs, t.remove).count()) await clients.rowButton(r.orgs, t.remove).click()
    }
    await expect.poll(() => show(a.id).orgs || []).toEqual([])
  })

  test('7 · Profile → Calls adds a subject rule and removes it; the matching count and the Calls tab follow', covers('threads/TC-013', 'F-THR-14', 'F-THR-19'), async ({ page }) => {
    const { acme: a, rule, pinned, matched } = data.set.s05
    const t = data.text.threads
    const r = t.rows
    const clients = new ClientsPage(page, data)
    try {
      await clients.openThread(a.id, t.tabs.profile)
      await expect(clients.rowButton(r.calls, t.add)).toBeVisible()
      await expect(clients.rowSays(r.calls, rule.before)).toBeVisible()
      await clients.rowButton(r.calls, t.add).click()
      await expect(clients.options(clients.rowSelect(r.calls, t.ruleKind))).toHaveText(rule.kinds)
      await expect(clients.addBox(r.calls)).toHaveAttribute('placeholder', t.rulePlaceholder)
      await clients.rowSelect(r.calls, t.ruleKind).selectOption({ label: rule.kind })
      await clients.addBox(r.calls).fill(rule.words)
      await clients.addBox(r.calls).press(key('enter'))
      await expect(clients.token(r.calls, rule.token)).toBeVisible()
      await expect(clients.rowButton(r.calls, t.remove)).toBeVisible()
      await expect(clients.rowSays(r.calls, rule.after)).toBeVisible()
      await clients.tab(t.tabs.callsTab).click()
      await expect(clients.callRows).toHaveCount(2)
      await expect(clients.callRow(pinned.title)).toContainText(t.why.pinned)
      await expect(clients.callRow(matched.title)).toContainText(t.why.subject)
      await clients.tab(t.tabs.profileTab).click()
      await clients.rowButton(r.calls, t.remove).click()
      await expect(clients.token(r.calls, rule.token)).toHaveCount(0)
      await expect(clients.rowSays(r.calls, rule.before)).toBeVisible()
    } finally {
      // Teardown: the rule removed if a check failed with it there.
      if (show(a.id).rules.length) data.cli('project', 'rule', a.id, '--remove', rule.remove)
    }
    expect(show(a.id)).toMatchObject({ rules: [], call_count: 1 })
  })

  test('8 · Membership is computed from rules and pins: a rule puts a call in and takes it out; no call file names the thread', covers('threads/TC-014', 'F-THR-19'), async () => {
    const { acme: a, rule, pinned, matched, whys, callFiles } = data.set.s05
    try {
      expect(threadsOf(matched.folder)).toEqual([])
      expect(cli('project', 'rule', a.id, '--subject', rule.words).rules).toContainEqual(rule.json)
      expect(threadsOf(matched.folder)).toContain(a.id)
      const p = show(a.id)
      expect(p.call_count).toBe(2)
      expect(p.calls.find((c) => c.title === matched.title).why).toBe(whys.subject)
      expect(p.calls.find((c) => c.title === pinned.title).why).toBe(whys.pinned)
      // The thread is written into no call's files.
      const named = []
      for (const call of fs.readdirSync(path.dirname(data.callDir(matched.folder)))) {
        for (const f of callFiles) {
          const file = path.join(path.dirname(data.callDir(matched.folder)), call, f)
          if (fs.existsSync(file) && fs.readFileSync(file, 'utf8').includes(a.id)) named.push(file)
        }
      }
      expect(named).toEqual([])
      expect(data.cli('project', 'rule', a.id, '--remove', rule.remove).code).toBe(0)
      expect(threadsOf(matched.folder)).not.toContain(a.id)
    } finally {
      // Teardown: the rule removed if a check failed with it there.
      if (show(a.id).rules.length) data.cli('project', 'rule', a.id, '--remove', rule.remove)
    }
    expect(show(a.id).rules).toEqual([])
  })

  test("9 · A call a rule put in is taken out by hand from the Calls tab; the rule doesn't put it back", covers('threads/TC-009', 'F-THR-11', 'F-THR-14'), async ({ page }) => {
    const { acme: a, rule, pinned, matched } = data.set.s05
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    try {
      expect(cli('project', 'rule', a.id, '--subject', rule.words).rules).toContainEqual(rule.json)
      await clients.openThread(a.id, t.tabs.calls)
      await expect(clients.callRows).toHaveCount(2)
      await expect(clients.callRow(pinned.title)).toContainText(t.why.pinned)
      await expect(clients.callButton(pinned.title, t.unpin)).toBeVisible()
      await expect(clients.callRow(matched.title)).toContainText(t.why.subject)
      await clients.callButton(matched.title, t.takeOut).click()
      await expect(clients.callRow(matched.title)).toHaveCount(0)
      await expect(clients.says(fill(t.takenOut, { n: 1 }))).toBeVisible()
      await expect(clients.tab(t.tabs.callsTab)).toHaveText(`${t.tabs.callsTab}1`)
      const p = show(a.id)
      expect(p.exclusions).toHaveLength(1)
      expect(p.calls.map((c) => c.title)).toEqual([pinned.title])
      await clients.openThread(a.id, t.tabs.profile)
      await expect(clients.rowSays(t.rows.calls, rule.takenOut)).toBeVisible()
    } finally {
      // Teardown: the call left to the rules again, the rule removed.
      data.cli('project', 'reset', a.id, data.callName(matched.folder))
      if (show(a.id).rules.length) data.cli('project', 'rule', a.id, '--remove', rule.remove)
    }
    expect(show(a.id)).toMatchObject({ exclusions: [], rules: [], call_count: 1 })
  })

  test('10 · The Calls tab says why a call belongs; Unpin takes it out, and the CLI pins it again', covers('threads/TC-008', 'F-THR-11'), async ({ page }) => {
    const { acme: a, pinned } = data.set.s05
    const t = data.text.threads
    const started = data.json(data.callDir(pinned.folder), data.text.calls.meta).started_at
    const clients = new ClientsPage(page, data)
    try {
      await clients.openThread(a.id, t.tabs.calls)
      await expect(clients.callRows).toHaveCount(1)
      const row = clients.callRow(pinned.title)
      await expect(row).toContainText(fill(t.rowLine, { day: pinned.day, time: await clients.clock(started), why: t.why.pinned }))
      await expect(row).not.toContainText(t.noMinutes)
      await expect(clients.callButton(pinned.title, t.unpin)).toBeVisible()
      await clients.callButton(pinned.title, pinned.title).click()
      await expect.poll(() => clients.hash()).toContain(t.routes.call)
      expect(clients.hash()).toMatch(new RegExp(`${literal(fill(t.routes.minutesEnd, { folder: pinned.folder }))}$`))

      await clients.openThread(a.id, t.tabs.calls)
      await clients.callButton(pinned.title, t.unpin).click()
      await expect(clients.callRows).toHaveCount(0)
      await expect(clients.says(t.noCalls)).toBeVisible()
      await expect(clients.tab(t.tabs.callsTab)).toHaveText(`${t.tabs.callsTab}0`)
      expect(show(a.id)).toMatchObject({ call_count: 0, calls: [] })
      expect(data.cli('project', 'include', a.id, data.callName(pinned.folder)).code).toBe(0)
      await page.reload()
      await clients.tab(t.tabs.callsTab).click()
      await expect(clients.callRows).toHaveCount(1)
      await expect(clients.callRow(pinned.title)).toContainText(t.why.pinned)
      await expect(clients.callButton(pinned.title, t.unpin)).toBeVisible()
    } finally {
      // Teardown: the call pinned again if a check failed with it out.
      if (!show(a.id).calls.length) data.cli('project', 'include', a.id, data.callName(pinned.folder))
    }
    expect(show(a.id).calls.map((c) => [c.title, c.why])).toEqual([[pinned.title, data.set.s05.whys.pinned]])
  })

  test('11 · "+ Thread" → New thread from this call… makes a thread named after the call, the call pinned', covers('threads/TC-004', 'F-THR-05'), async ({ page }) => {
    const { matched, fromCall: n } = data.set.s05
    const t = data.text.threads
    const c = data.text.calls
    const calls = new CallsPage(page, data)
    const clients = new ClientsPage(page, data)
    try {
      await calls.open()
      await calls.openCall(matched.title)
      await expect(calls.headerButton(c.addThread)).toBeVisible()
      for (const name of n.threads) await expect(calls.headerButton(name)).toHaveCount(0)
      await calls.headerButton(c.addThread).click()
      for (const name of n.threads) await expect(calls.threadItem(name)).toBeVisible()
      await expect(calls.menuItems).toHaveText([c.newThreadItem])
      await calls.menuItem(c.newThreadItem).click()
      await expect.poll(() => clients.hash()).toBe(route(n.id, t.tabs.profile))
      await expect(clients.heading(n.name)).toBeVisible()
      await expect(clients.token(t.rows.calls, n.token)).toBeVisible()
      await expect(clients.rowStarting(t.rows.calls, n.line)).toBeVisible()
      await clients.tab(t.tabs.callsTab).click()
      await expect(clients.callRows).toHaveCount(1)
      await expect(clients.callRow(matched.title)).toContainText(t.why.pinned)
      await expect(clients.callButton(matched.title, t.unpin)).toBeVisible()
    } finally {
      // Teardown: the thread deleted.
      data.cli('project', 'delete', n.id)
    }
    expect(cli('project', 'list').projects.map((p) => p.id)).not.toContain(n.id)
  })

  test('12 · The Brief is edited in place: Escape cancels, ⌘S saves it as Markdown; Latest calls lists the call', covers('threads/TC-007', 'F-THR-10'), async ({ page }) => {
    const g = data.set.s05.globex
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    try {
      await clients.openThread(g.id, t.tabs.brief)
      await expect(clients.says(t.whatItIs)).toBeVisible()
      await expect(clients.button(t.edit)).toBeVisible()
      await expect(clients.says(t.briefEmpty)).toBeVisible()
      await expect(clients.subheading(t.latest)).toBeVisible()
      await expect(clients.latestCall(g.latest)).toBeVisible()
      await clients.button(t.edit).click()
      await expect(clients.briefBox).toBeVisible()
      await expect(clients.button(t.cancel)).toBeVisible()
      await expect(clients.button(t.save)).toBeVisible()
      await expect(clients.says(t.briefHint)).toBeVisible()
      await clients.briefBox.fill(g.draft)
      await clients.briefBox.press(key('escape'))
      await expect(clients.briefBox).toHaveCount(0)
      await expect(clients.says(t.briefEmpty)).toBeVisible()
      await clients.button(t.edit).click()
      await clients.briefBox.fill(g.brief)
      await clients.briefBox.press(key('save'))
      await expect(clients.briefBox).toHaveCount(0)
      await expect(clients.paragraph(g.briefShown)).toHaveText(g.briefShown)
      await expect(clients.bold).toHaveText(g.bold)
      await expect.poll(() => show(g.id).brief.trim()).toBe(g.brief)
    } finally {
      // Teardown: the brief emptied (the CLI takes it from a file).
      const empty = path.join(data.dir, data.set.s05.emptyBrief)
      fs.writeFileSync(empty, '')
      data.cli('project', 'set', g.id, '--brief', empty)
    }
    expect(show(g.id).brief).toBe('')
  })

  test('13 · A thread renamed with echoctl project set --name shows its new name, its id unchanged', covers('threads/TC-006', 'F-THR-07'), async ({ page }) => {
    const g = data.set.s05.globex
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    try {
      await clients.openThread(g.id, t.tabs.brief)
      await expect(clients.heading(g.name)).toBeVisible()
      expect(cli('project', 'set', g.id, '--name', g.renamed)).toMatchObject({ name: g.renamed, id: g.id })
      await clients.openThread(g.id, t.tabs.brief)
      await expect(clients.heading(g.renamed)).toBeVisible()
      expect(clients.hash()).toBe(route(g.id, t.tabs.brief))
      const groups = data.set.s05.groups
      expect(under(await clients.listLines(), g.group, groups).map((l) => l.startsWith(`${g.renamed} `))).toEqual([true])
      await expect(clients.threadRow(g.name)).toHaveCount(0)
    } finally {
      // Teardown: the old name back.
      data.cli('project', 'set', g.id, '--name', g.name)
    }
    expect(show(g.id).name).toBe(g.name)
  })

  test('14 · Profile → Since, Notes for Claude and Look are edited in place and saved at once; the AI row offers the profiles', covers('threads/TC-015', 'F-THR-15'), async ({ page }) => {
    const g = data.set.s05.globex
    const t = data.text.threads
    const r = t.rows
    const clients = new ClientsPage(page, data)
    try {
      await clients.openThread(g.id, t.tabs.profile)
      await expect(clients.rowButton(r.since, t.sinceFirst)).toBeVisible()
      await expect(clients.rowButton(r.notes, t.notesEmpty)).toBeVisible()
      await expect(clients.row(r.ai)).toContainText(t.aiDefault)
      await expect(clients.row(r.history)).toHaveCount(0)

      await clients.rowButton(r.since, t.sinceFirst).click()
      await clients.box(r.since).fill(g.since)
      await expect(clients.rowButton(r.since, await clients.sinceDay(g.since))).toBeVisible()
      await expect.poll(() => show(g.id).since).toBe(g.since)

      await clients.rowButton(r.notes, t.notesEmpty).click()
      await clients.box(r.notes).fill(g.notes)
      await clients.box(r.notes).blur()
      await expect(clients.rowButton(r.notes, g.notes)).toBeVisible()
      await expect.poll(() => show(g.id).notes).toBe(g.notes)

      const swatch = clients.rowButton(r.look, fill(t.colour, { c: g.colour }))
      await swatch.click()
      await expect(swatch).toHaveAttribute('aria-pressed', 'true')
      await expect.poll(() => show(g.id).color).toBe(g.colour)

      await expect(clients.options(clients.rowSelect(r.ai, t.aiProfile))).toHaveText(t.aiOptions)
      await expect(clients.rowButton(r.ai, t.model)).toBeVisible()
    } finally {
      // Teardown: since, notes and colour back.
      data.cli('project', 'set', g.id, '--since', '', '--notes', '', '--color', g.colourBack)
    }
    const p = show(g.id)
    expect([p.since || '', p.notes || '', p.color]).toEqual(['', '', g.colourBack])
  })

  test('15 · A closed stage moves the thread under Closed · 1; its history is shown; Active brings it back', covers('threads/TC-010', 'F-THR-12'), async ({ page }) => {
    const g = data.set.s05.globex
    const t = data.text.threads
    const r = t.rows
    const clients = new ClientsPage(page, data)
    try {
      await clients.openThread(g.id, t.tabs.profile)
      for (const name of g.stageGroups) await expect(clients.rowSays(r.stage, name)).toBeVisible()
      await expect(clients.rowButtons(r.stage)).toHaveText(g.stages)
      await expect(clients.rowButton(r.stage, g.active)).toHaveAttribute('aria-pressed', 'true')
      const since = show(g.id).stages.at(-1).at.slice(0, 10)
      await expect(clients.rowSays(r.stage, fill(t.stageSince, { stage: g.active, day: await clients.shortDay(since) }))).toBeVisible()

      await clients.rowButton(r.stage, g.wrapUp).click()
      await expect(clients.rowButton(r.stage, g.wrapUp)).toHaveAttribute('aria-pressed', 'true')
      await expect(clients.fact(g.name, g.wrapUpTag)).toBeVisible()
      const history = clients.rowStarting(r.stage, fill(t.stageSince, { stage: g.wrapUp, day: '' }))
      await expect(history).toContainText(g.active)

      await clients.rowButton(r.stage, g.done).click()
      await expect(clients.rowButton(r.stage, g.done)).toHaveAttribute('aria-pressed', 'true')
      await expect(clients.factEnding(g.name, g.doneTag)).toBeVisible()
      await expect(clients.threadRow(g.name)).toHaveCount(0)
      await expect(clients.closedFold).toHaveAccessibleName(new RegExp(`${literal(fill(t.closed, { n: 1 }))}$`))
      await clients.closedFold.click()
      await expect(clients.threadRow(g.name)).toHaveAccessibleName(g.closedRow)
      expect(cli('project', 'list').projects.find((p) => p.id === g.id)).toMatchObject({ stage: g.stageIds.done, group: g.stageIds.closed })

      await clients.rowButton(r.stage, g.active).click()
      await expect(clients.rowButton(r.stage, g.active)).toHaveAttribute('aria-pressed', 'true')
      await expect(clients.closedFold).toHaveCount(0)
      await expect(clients.threadRow(g.name)).toBeVisible()
    } finally {
      // Teardown: Active again even when a check failed.
      if (show(g.id).stage !== g.stageIds.active) data.cli('project', 'set', g.id, '--stage', g.stageIds.active)
    }
    expect(show(g.id).stage).toBe(g.stageIds.active)
  })

  test('16 · Profile → Client changes the client, makes a new one inline and clears it', covers('threads/TC-011', 'F-THR-13'), async ({ page }) => {
    const { globex: g, northwind: n } = data.set.s05
    const t = data.text.threads
    const r = t.rows
    const clients = new ClientsPage(page, data)
    try {
      await clients.openThread(g.id, t.tabs.profile)
      await expect(clients.rowButton(r.client, g.group)).toBeVisible()
      await expect(clients.rowSays(r.client, fill(t.domains, { d: g.domains }))).toBeVisible()
      await expect(clients.rowSelect(r.client, r.client)).toBeVisible()

      await clients.rowSelect(r.client, r.client).selectOption({ label: t.newClient })
      await expect(clients.box(t.clientName)).toBeVisible()
      await expect(clients.box(t.mailDomains)).toBeVisible()
      await expect(clients.rowButton(r.client, t.addButton)).toBeVisible()
      await expect(clients.rowButton(r.client, t.cancel)).toBeVisible()
      await clients.box(t.clientName).fill(n.name)
      await clients.box(t.mailDomains).fill(n.domain)
      await clients.rowButton(r.client, t.addButton).click()
      await expect(clients.rowButton(r.client, n.name)).toBeVisible()
      await expect(clients.rowSays(r.client, fill(t.domains, { d: n.domain }))).toBeVisible()
      await expect(clients.clientTag).toHaveText(n.name)
      await expect(clients.clientGroup(n.name)).toBeVisible()
      const groups = [...data.set.s05.groups, n.name, t.noClientGroup]
      await expect.poll(async () => under(await clients.listLines(), n.name, groups).map((l) => l.startsWith(`${g.name} `))).toEqual([true])

      await clients.rowSelect(r.client, r.client).selectOption({ label: t.noClientOption })
      await expect(clients.rowSays(r.client, t.noClient)).toBeVisible()
      await expect(clients.clientTag).toHaveCount(0)
      await expect.poll(async () => under(await clients.listLines(), t.noClientGroup, groups).map((l) => l.startsWith(`${g.name} `))).toEqual([true])

      await clients.rowSelect(r.client, r.client).selectOption({ label: g.group })
      await expect(clients.rowButton(r.client, g.group)).toBeVisible()
    } finally {
      // Teardown: Globex its client again, Northwind removed.
      if (show(g.id).client !== g.clientId) data.cli('project', 'set', g.id, '--client', g.clientId)
      data.cli('client', 'remove', n.id)
    }
    expect(show(g.id).client).toBe(g.clientId)
    expect(cli('client', 'list').map((c) => c.id)).not.toContain(n.id)
  })

  test('17 · Thread actions → Delete… asks first; Move to trash removes the thread and leaves its call', covers('threads/TC-005', 'F-THR-06'), async ({ page }) => {
    const { scratch: x, matched } = data.set.s05
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    const calls = new CallsPage(page, data)
    try {
      expect(cli('project', 'create', x.name).id).toBe(x.id)
      expect(data.cli('project', 'include', x.id, data.callName(matched.folder)).code).toBe(0)
      await clients.openThread(x.id, t.tabs.calls)
      await expect(clients.heading(x.name)).toBeVisible()
      await expect(clients.tab(t.tabs.callsTab)).toHaveText(`${t.tabs.callsTab}1`)
      await expect(clients.callRow(matched.title)).toHaveCount(1)
      const ask = clients.says(fill(t.deleteAsk, { name: x.name }))
      await clients.threadActions.click()
      await clients.menuItem(t.deleteItem).click()
      await expect(ask).toBeVisible()
      await expect(clients.button(t.cancel)).toBeVisible()
      await expect(clients.button(t.moveToTrash)).toBeVisible()
      await clients.button(t.cancel).click()
      await expect(ask).toHaveCount(0)
      await expect(clients.heading(x.name)).toBeVisible()
      await clients.threadActions.click()
      await clients.menuItem(t.deleteItem).click()
      await clients.button(t.moveToTrash).click()
      await expect.poll(() => clients.hash()).toBe(t.routes.projects)
      await expect(clients.threadRow(x.name)).toHaveCount(0)
      expect(cli('project', 'list').projects.map((p) => p.id)).not.toContain(x.id)
      await calls.open()
      await expect(calls.call(matched.title)).toBeVisible()
    } finally {
      // Teardown: the thread deleted if a check failed before its delete.
      if (cli('project', 'list').projects.some((p) => p.id === x.id)) data.cli('project', 'delete', x.id)
    }
    expect(cli('project', 'list').projects.map((p) => p.id)).not.toContain(x.id)
  })
})
