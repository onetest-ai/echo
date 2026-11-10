// S-26 Ten threads in every stage: membership, people and clients — every stage (standard data): the Threads list by
// client with Found for you and the closed fold, why each call is in a thread, a call in two threads, suggestions that
// never assign, calls taken out, the rules and their counts, counterparts, Found for you's six candidates, closed
// threads; the People Map, List, new tags, a client's People tab, a person's panel, each person's side; then the
// clients: account against organisation, names and domains, About and logo, a client with only closed threads, one
// with none, the user's company, the Overview cards, the Calls and Tasks tabs; last, Acme rollout's Since moved back.
// Only the last stage changes data (as the scenario says, its teardown is skipped). Each test has its own browser, so
// what a stage left in the browser (the closed fold, the People view) is set again where a stage reads it.
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { ClientsPage } from '../pages/clients.page.mjs'
import { PeoplePage } from '../pages/people.page.mjs'
import { TasksPage } from '../pages/tasks.page.mjs'
import { WorkPage } from '../pages/work.page.mjs'

const data = useData(test, { set: 'standard', ai: 'fake' })

/** `echoctl …` that must succeed, its output read as JSON. */
const cli = (...args) => {
  const r = data.cli(...args)
  if (r.code !== 0) throw new Error(`${args.join(' ')}: ${r.err}`)
  return r.json()
}
const show = (id) => cli('project', 'show', id)
/** `echoctl project of` a call, by the end of its folder's name; its threads and suggestions as ids. */
const of = (slug) => {
  const r = cli('project', 'of', data.callName(slug))
  return { projects: r.projects.map((p) => ({ id: p.id, why: p.why })), suggested: r.suggested.map((p) => p.id) }
}
/** A thread's calls, newest first (`project show`). */
const callsOf = (id) => [...show(id).calls].sort((a, b) => b.started_at.localeCompare(a.started_at))
/** A thread's route on a tab; a client's. */
const threadRoute = (id, tab) => fill(data.text.threads.routes.tab, { id, tab })
const clientRoute = (id, tab) => fill(data.text.clients.routes.client, { id, tab })

/** What the Threads list reads from the top: its heading, New thread, Found for you, each client group and its rows,
 *  the closed fold (and its rows while open). */
async function listLines(clients, open) {
  const l = data.set.s26.list
  const t = data.text.threads
  const last = Object.fromEntries(cli('project', 'list').projects.map((p) => [p.id, p.last_call]))
  const row = async ([name, calls, stage, id]) => clients.rowName({ name, calls, day: last[id] && await clients.dayLabel(last[id]), stage, logo: l.logo.includes(name) })
  const lines = [t.threads, t.newThread, fill(t.foundCount, { n: l.found })]
  for (const g of l.groups) {
    lines.push(g)
    for (const r of l.rows[g]) lines.push(await row(r))
  }
  lines.push(fill(t.closed, { n: l.closed }))
  if (open) for (const r of l.closedRows) lines.push(await row(r))
  return lines
}

/** A Calls tab's row as it reads: its title, its day and time with why it is in (and "no minutes yet"), its button. */
async function callRow(page, call, row) {
  const t = data.text.threads
  const at = new Date(call.started_at)
  const vars = { day: await page.dayLabel(call.started_at), time: await page.clock(at.toISOString()), why: row.why, minutes: t.noMinutes }
  return [row.title, fill(row.noMinutes ? t.rowLineMinutes : t.rowLine, vars), row.button]
}

/** A client's Calls row as it reads: its title, its day and time (and "no minutes yet") with its thread chip; a chip
 *  with a picture reads apart, its picture first. */
async function clientCallRow(page, call, title, chip, noMinutes) {
  const c = data.text.clients
  const vars = { day: await page.dayLabel(call.started_at), time: await page.clock(call.started_at), minutes: c.noMinutes }
  const line = fill(noMinutes ? c.callLineMinutes : c.callLine, vars)
  return data.set.s26.list.logo.includes(chip) ? [title, line, chip, chip] : [title, `${line} ${chip}`]
}

/** "1 call", "3 calls", as a card writes a count of calls. */
const callCount = (n) => fill(data.text.threads.callCount[n === 1 ? 'one' : 'many'], { n })

/** A Profile row's tokens as it reads them, each followed by its ×. */
const tokens = (xs, remove) => xs.flatMap((x) => [x, typeof remove === 'function' ? remove(x) : remove])

/** The YYYY-MM-DD of n days ago, on this computer's calendar. */
const daysAgo = (n) => {
  const d = new Date(Date.now() - n * 86_400_000)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

scenario(test, 'S-26', 'Ten threads in every stage: membership, people and clients', () => {
  test('1 · The Threads list on the full set: Found for you 6, the groups in order with their rows, Closed · 3 folded, a client fold, a client\'s name', covers('threads/TC-018', 'F-THR-01', 'F-THR-02', 'F-THR-03'), async ({ page }) => {
    const l = data.set.s26.list
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    await clients.goto(t.routes.projects)
    await expect(clients.listHeading(t.threads)).toBeVisible()
    const shut = await listLines(clients, false)
    await expect.poll(() => clients.listLines()).toEqual(shut)
    for (const g of l.noFold) await expect(clients.fold(g)).toHaveCount(0)
    await expect(clients.activityCount(t.threads)).toHaveText(String(l.found))

    await clients.closedFold.click()
    await expect(clients.closedFold).toHaveAttribute('aria-expanded', 'true')
    await expect.poll(() => clients.listLines()).toEqual(await listLines(clients, true))

    await clients.fold(l.fold.group).click()
    await expect(clients.threadRow(l.fold.row)).toHaveCount(0)
    await expect(clients.foldCount(l.fold.group, l.fold.count)).toBeVisible()
    await clients.fold(l.fold.group).click()
    await expect(clients.threadRow(l.fold.row)).toBeVisible()

    await clients.group(l.client.group).click()
    await expect.poll(() => clients.hash()).toBe(clientRoute(l.client.id, t.tabs.overview))
  })

  test('2 · Each thread\'s Calls tab says why each call is in: its series, the client\'s people, its subject, added by hand', covers('threads/TC-019', 'F-THR-11', 'F-THR-14'), async ({ page }) => {
    const c = data.set.s26.calls
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    for (const id of c.order) {
      const rows = c.rows[id]
      const calls = callsOf(id)
      expect([id, calls.map((x) => x.title)]).toEqual([id, rows.map((r) => r.title)])
      await clients.openThread(id, t.tabs.calls)
      await expect(clients.tab(t.tabs.callsTab)).toHaveAccessibleName(fill(t.tabCount, { tab: t.tabs.callsTab, n: rows.length }))
      const want = []
      for (const [i, r] of rows.entries()) want.push(await callRow(clients, calls[i], r))
      await expect.poll(() => clients.callRowsRead()).toEqual(want)
      await expect(clients.callRowsSaying(t.teamsChat)).toHaveCount(0)
    }
  })

  test('3 · One call in two threads, with its own reason in each; the threads aren\'t written into the call', covers('threads/TC-020', 'F-THR-11', 'F-THR-19', 'F-CLIENT-03'), async ({ page }) => {
    const x = data.set.s26.twoThreads
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    for (const [id, title, why, button] of x.in) {
      await clients.openThread(id, t.tabs.calls)
      await expect(clients.callRow(title)).toContainText(why)
      await expect(clients.callButton(title, button)).toBeVisible()
    }
    const call = callsOf(x.of[0].id).find((c) => c.title === x.title)
    for (const c of x.clients) {
      await clients.openClient(c.id, t.tabs.calls)
      await expect(clients.callRows).toHaveCount(c.calls)
      const rows = await clients.callRowsRead()
      expect(rows.find((r) => r[0] === x.title)).toEqual(await clientCallRow(clients, call, x.title, c.chips[0], false))
    }
    expect(of(x.call).projects).toEqual(x.of)
    for (const file of x.files) {
      const text = data.read(data.callDir(x.call), file)
      for (const id of x.notIn) expect([file, text.includes(id)]).toEqual([file, false])
    }
  })

  test('4 · A suggestion never assigns a call: project of suggests, the call\'s bar asks, no thread holds it', covers('threads/TC-024', 'F-THR-19', 'F-THR-11'), async ({ page }) => {
    const s = data.set.s26.suggested
    const t = data.text.threads
    for (const c of s.calls) expect([c.slug, of(c.slug)]).toEqual([c.slug, { projects: [], suggested: c.suggested }])
    const calls = new CallsPage(page, data)
    for (const c of s.calls) {
      const why = cli('project', 'of', data.callName(c.slug)).suggested[0].why
      await calls.goto(`${t.routes.call}${data.callName(c.slug)}`)
      await expect(calls.headerButton(data.text.calls.addThread)).toBeVisible()
      await expect(calls.says(fill(t.partOf, { thread: c.bar, why }))).toBeVisible()
      await expect(calls.says(why)).toBeVisible()
      for (const b of [t.suggestAdd, t.notThis]) await expect(calls.headerButton(b)).toBeVisible()
      await expect(calls.headerButton(c.bar)).toHaveCount(0)
    }
    const clients = new ClientsPage(page, data)
    for (const [id, n] of s.threads) {
      await clients.openThread(id, t.tabs.calls)
      await expect(clients.callRows).toHaveCount(n)
      for (const title of s.titles) await expect(clients.callRow(title)).toHaveCount(0)
    }
    for (const c of s.calls) expect([c.slug, of(c.slug).projects]).toEqual([c.slug, []])
  })

  test('5 · Calls taken out by hand stay out and are counted, on the Calls tab and in the Profile', covers('threads/TC-022', 'F-THR-11', 'F-THR-14', 'F-THR-19'), async ({ page }) => {
    const x = data.set.s26.takenOut
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    for (const [id, title] of x.threads) {
      await clients.openThread(id, t.tabs.calls)
      await expect(clients.paragraph(x.footer)).toHaveText(x.footer)
      await expect(clients.callRow(title)).toHaveCount(0)
    }
    await clients.openThread(x.none[0], t.tabs.calls)
    await expect(clients.callRows.first()).toBeVisible()
    await expect(clients.paragraph(x.footer)).toHaveCount(0)
    for (const [id, , line] of [...x.threads, [x.none[0], null, x.none[1]]]) {
      await clients.openThread(id, t.tabs.profile)
      await expect(clients.rowSays(t.rows.calls, line)).toBeVisible()
    }
    expect(of(x.call)).toEqual({ projects: [], suggested: [] })
  })

  test('6 · The Profile\'s rules and match counts on the ten threads agree with the CLI', covers('threads/TC-026', 'F-THR-14', 'F-THR-19'), async ({ page }) => {
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    const n = (re, line) => Number((new RegExp(re).exec(line) || [])[1] || 0)
    for (const r of data.set.s26.rules) {
      await clients.openThread(r.id, t.tabs.profile)
      await expect.poll(() => clients.rowReads(t.rows.calls)).toEqual([...tokens(r.tokens, t.remove), t.add, r.line])
      const p = show(r.id)
      expect([r.id, p.rules.length, p.call_count, p.pins.length, p.exclusions.length])
        .toEqual([r.id, r.tokens.length, n(t.counts.match, r.line), n(t.counts.pinned, r.line), n(t.counts.takenOut, r.line)])
    }
  })

  test('7 · Counterparts: two, one or none; an organisation that isn\'t a client has no group', covers('threads/TC-027', 'F-THR-13', 'F-CLIENT-08'), async ({ page }) => {
    const o = data.set.s26.orgs
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    for (const [id, orgs] of o.threads) {
      await clients.openThread(id, t.tabs.profile)
      await expect.poll(() => clients.rowReads(t.rows.orgs)).toEqual([...tokens(orgs.map(([name]) => name), t.remove), t.addOrg, t.orgHintFull])
      for (const [name, domain] of orgs) await expect(clients.titled(t.rows.orgs, domain)).toContainText(name)
    }
    await clients.goto(t.routes.projects)
    await expect(clients.listHeading(t.threads)).toBeVisible()
    for (const name of o.notGroups) await expect(clients.clientGroup(name)).toHaveCount(0)
    const orgs = cli('org', 'list')
    const list = Array.isArray(orgs) ? orgs : orgs.orgs || orgs.clients
    for (const id of o.notAccounts) expect(list.find((x) => x.id === id)).toMatchObject({ account: false })
  })

  test('8 · Found for you: six candidates, each with its why, organisation, client choice and calls; nothing accepted', covers('threads/TC-025', 'F-THR-18', 'F-THR-03'), async ({ page }) => {
    const f = data.set.s26.found
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    await clients.goto(t.routes.projects)
    await expect(clients.foundForYou).toHaveAccessibleName(fill(t.foundCount, { n: f.cards.length }))
    await clients.foundForYou.click()
    await expect.poll(() => clients.hash()).toBe(t.routes.found)
    await expect(clients.heading(t.found)).toBeVisible()
    await expect(clients.foundCards).toHaveCount(f.cards.length)
    for (const c of f.cards) {
      await expect(clients.foundBox(c.name)).toHaveValue(c.name)
      for (const b of t.foundAnswers) await expect(clients.foundButton(c.name, b)).toBeVisible()
      await expect(clients.foundSays(c.name, c.why)).toBeVisible()
      if (c.org) await expect(clients.foundSays(c.name, c.org)).toBeVisible()
      await expect(clients.chosen(clients.foundSelect(c.name, t.rows.client), c.client)).toHaveCount(1)
      if (c.newClient) await expect(clients.foundOption(c.name, t.rows.client, c.newClient)).toHaveCount(1)
      await expect(clients.foundCalls(c.name)).toHaveCount(c.calls.length)
      for (const title of c.calls) await expect(clients.foundCall(c.name, title).first()).toBeVisible()
    }
    await expect(clients.options(clients.foundSelect(f.cards[0].name, t.addTo))).toHaveText(f.threads)
    await expect(clients.newThreadName).toBeVisible()
    await expect(clients.newThreadClient).toBeVisible()
    await expect(clients.create).toBeVisible()
    expect(cli('project', 'list').projects).toHaveLength(f.all)
  })

  test('9 · A closed thread keeps its calls and takes no new one, even a matching one', covers('threads/TC-023', 'F-THR-12', 'F-THR-19'), async ({ page }) => {
    const x = data.set.s26.closedThreads
    const t = data.text.threads
    for (const slug of x.notTaken) expect([slug, of(slug)]).toEqual([slug, { projects: [], suggested: [] }])
    const p = show(x.tyrell.id)
    expect(p).toMatchObject({ call_count: x.tyrell.calls, group: x.tyrell.group })
    expect(p.calls.map((c) => ({ title: c.title, why: c.why }))).toEqual([{ title: x.tyrell.call, why: x.tyrell.why }])
    expect(p.stages.at(-1).stage).toBe(x.tyrell.last)
    const clients = new ClientsPage(page, data)
    for (const [id, title, why, button, notTaken] of x.rows) {
      await clients.openThread(id, t.tabs.calls)
      await expect(clients.callRows).toHaveCount(1)
      await expect(clients.callRow(title)).toContainText(why)
      await expect(clients.callButton(title, button)).toBeVisible()
      await expect(clients.callRow(notTaken)).toHaveCount(0)
    }
    await clients.openClient(x.client.id, t.tabs.calls)
    await expect(clients.callRows).toHaveCount(1)
    await expect(clients.callRow(x.client.call)).toContainText(x.client.chip)
  })

  test('10 · The People Map places people and organisations by influence and interest; the same person sits apart in each thread', covers('people/TC-009', 'F-PPL-02', 'F-PPL-01'), async ({ page }) => {
    const m = data.set.s26.map
    const pt = data.text.people
    const people = new PeoplePage(page, data)
    for (const th of m.threads) {
      await people.open(th.id)
      await expect(people.view(pt.map)).toHaveAttribute('aria-pressed', 'true')
      if (th.count) await expect(people.peopleTab).toHaveAccessibleName(fill(pt.tabCount, { n: th.count }))
      for (const s of m.stances) await expect(people.legend(s)).toBeVisible()
      await expect(people.addPerson).toBeVisible()
      for (const [q, chips] of Object.entries(th.quadrants)) {
        await expect.poll(() => people.areaReads(people.quadrant(q))).toEqual([fill(pt.quadrantHead, { name: q, sub: m.subtitles[q] }), ...chips])
      }
      await expect.poll(() => people.areaReads(people.tray)).toEqual([pt.notPlaced, ...th.tray])
      for (const [name, stance] of th.stances) expect([name, await people.avatarColour(people.chip(name))]).toEqual([name, await people.legendColour(stance)])
      await expect(people.paragraph(m.line.split(' — ')[0])).toHaveText(m.line)
    }
  })

  test('11 · The List splits the thread\'s people from those only seen in its calls', covers('people/TC-010', 'F-PPL-04', 'F-PPL-01'), async ({ page }) => {
    const l = data.set.s26.peopleList
    const pt = data.text.people
    const people = new PeoplePage(page, data)
    await people.open(l.thread)
    await people.view(pt.list).click()
    await expect(people.columns(people.inThread)).toHaveText(l.columns)
    await expect(people.bodyRows(people.inThread)).toHaveCount(l.in.length)
    for (const [i, [name, org, role, stance, calls, isNew]] of l.in.entries()) {
      await expect(people.bodyRows(people.inThread).nth(i)).toHaveAccessibleName(new RegExp(`^${name} `))
      expect([name, await people.cellText(people.inThread, name, pt.col.organisation), await people.cellText(people.inThread, name, pt.col.role), await people.cellText(people.inThread, name, pt.col.stance), await people.cellText(people.inThread, name, pt.col.calls)])
        .toEqual([name, org, role, stance, calls])
      await expect(people.rowNew(people.inThread, name)).toHaveCount(isNew ? 1 : 0)
    }
    await expect(people.says(pt.seenCap)).toBeVisible()
    await expect(people.says(l.seenLine)).toBeVisible()
    await expect(people.button(pt.addAll)).toBeVisible()
    await expect(people.bodyRows(people.seen)).toHaveCount(l.seen.length)
    for (const [name, org, calls] of l.seen) {
      expect([name, await people.cellText(people.seen, name, pt.col.organisation), await people.cellText(people.seen, name, pt.col.calls)]).toEqual([name, org, calls])
      await expect(people.rowButton(people.seen, name, pt.add)).toBeVisible()
    }
    const listed = cli('project', 'people', l.thread)
    const inThread = (listed.people || listed).filter((p) => p.in_thread).map((p) => p.name).sort()
    expect(inThread).toEqual(l.in.map(([name]) => name).sort())
    await people.view(pt.map).click()
    await expect(people.view(pt.map)).toHaveAttribute('aria-pressed', 'true')
  })

  test('12 · The tag "new" on people first seen within two weeks, on the Map, the List and a client\'s People; none is quiet', covers('people/TC-012', 'F-PPL-04'), async ({ page }) => {
    const n = data.set.s26.newTag
    const pt = data.text.people
    const people = new PeoplePage(page, data)
    await people.open(n.thread)
    await expect(people.view(pt.map)).toHaveAttribute('aria-pressed', 'true')
    await expect(people.tray).toBeVisible()
    await expect(people.newTags).toHaveCount(n.new.length)
    for (const name of n.new) await expect(people.chip(name, people.tray)).toContainText(pt.newTag)
    await people.view(pt.list).click()
    await expect(people.inThread).toBeVisible()
    await expect(people.newTags).toHaveCount(n.new.length)
    for (const name of n.new) await expect(people.rowNew(people.inThread, name)).toHaveCount(1)
    await expect(people.quietTags).toHaveCount(0)
    await people.open(n.globex.id)
    await expect(people.inThread).toBeVisible()
    for (const name of n.globex.new) await expect(people.rowNew(people.inThread, name)).toHaveCount(1)
    for (const name of n.globex.not) await expect(people.rowNew(people.inThread, name)).toHaveCount(0)
    const listed = cli('project', 'people', n.thread)
    const byName = Object.fromEntries((listed.people || listed).map((p) => [p.name, p]))
    for (const [name, isNew] of n.cli) expect([name, byName[name].new, byName[name].quiet]).toEqual([name, isNew, false])
    await people.goto(clientRoute(n.client.id, data.text.clients.tabIds.people))
    await expect(people.inClient).toBeVisible()
    for (const name of n.client.new) await expect(people.rowNew(people.inClient, name)).toHaveCount(1)
    for (const name of n.client.not) await expect(people.rowNew(people.inClient, name)).toHaveCount(0)
  })

  test('13 · A client\'s People tab: a Threads column, the people seen only in calls, the Map by the strongest place; the user\'s company', covers('people/TC-013', 'F-PPL-01', 'F-PPL-04', 'F-CLIENT-07'), async ({ page }) => {
    const c = data.set.s26.clientPeople
    const pt = data.text.people
    const people = new PeoplePage(page, data)
    await people.goto(clientRoute(c.id, data.text.clients.tabIds.people))
    await people.view(pt.list).click()
    await expect(people.peopleTab).toHaveAccessibleName(fill(pt.tabCount, { n: c.count }))
    await expect(people.columns(people.inClient)).toHaveText(c.columns)
    await expect(people.addPerson).toHaveCount(0)
    await expect(people.bodyRows(people.inClient)).toHaveCount(c.in.length)
    for (const [name, threads, org] of c.in) {
      expect([name, await people.cellText(people.inClient, name, pt.col.threads), await people.cellText(people.inClient, name, pt.col.organisation)]).toEqual([name, threads.replaceAll(', ', ''), org])
    }
    await expect(people.bodyRows(people.seen)).toHaveCount(c.seen.length)
    for (const [i, [name, org]] of c.seen.entries()) {
      const row = people.bodyRows(people.seen).nth(i)
      await expect(row).toHaveAccessibleName(new RegExp(`^${name} `))
      await expect(row).toContainText(org)
    }
    await expect(people.button(pt.add)).toHaveCount(0)
    await expect(people.button(pt.addAll)).toHaveCount(0)
    await people.view(pt.map).click()
    for (const [q, names] of Object.entries(c.map)) for (const name of names) await expect(people.chip(name, people.quadrant(q))).toBeVisible()
    await expect(people.paragraph(c.mapLine.split(' — ')[0])).toHaveText(c.mapLine)
    await expect(people.chip(c.map[Object.keys(c.map)[0]][0])).not.toHaveAttribute('draggable', 'true')
    await people.goto(clientRoute(c.own.id, data.text.clients.tabIds.people))
    await people.view(pt.list).click()
    await expect(people.says(c.own.nobody)).toBeVisible()
    await expect(people.bodyRows(people.seen)).toHaveCount(c.own.seen.length)
    for (const [name, org] of c.own.seen) expect([name, await people.cellText(people.seen, name, pt.col.organisation)]).toEqual([name, org])
  })

  test('14 · A person\'s panel: place in the thread, who set what (you, Microsoft 365, Echo, a question), who they are and the facts', covers('people/TC-014', 'F-PPL-05', 'F-PPL-06'), async ({ page }) => {
    const p = data.set.s26.panel
    const pt = data.text.people
    const people = new PeoplePage(page, data)
    await people.open(p.thread)
    await people.view(pt.list).click()
    try {
      const d = p.dana
      await people.row(people.inThread, d.name).click()
      await expect(people.panelButton(d.name)).toBeVisible()
      for (const text of [d.sub, d.aboutNote]) await expect(people.detail(text)).toBeVisible()
      await expect(people.panelButton(d.about)).toBeVisible()
      await expect(people.value(pt.panel.role)).toHaveText(d.role)
      await expect(people.detailEnding(d.roleBy)).toBeVisible()
      await expect(people.chosen(people.stance, d.stance)).toHaveCount(1)
      expect([await people.level(pt.panel.influence), await people.level(pt.panel.interest)]).toEqual([d.influence, d.interest])
      await expect(people.value(pt.panel.workstream)).toHaveText(d.workstream)
      await expect(people.chosen(people.panelSelect(pt.panel.governance), d.governance)).toHaveCount(1)
      await expect(people.value(pt.panel.title)).toHaveText(d.title)
      await expect(people.value(pt.panel.phone)).toHaveText(d.phone)
      await expect(people.value(pt.panel.office)).toHaveText(d.office)
      await expect(people.detail(d.source)).toHaveCount(3)
      await expect(people.facts).toHaveCount(d.facts[0])
      await people.panelButton(d.all).click()
      await expect(people.facts).toHaveCount(d.facts[1])
      await expect(people.panelButton(d.fewer)).toBeVisible()

      const o = p.omar
      await people.row(people.inThread, o.name).click()
      await expect(people.panelButton(o.name)).toBeVisible()
      await expect(people.value(pt.panel.role)).toHaveText(o.role)
      await expect(people.detailEnding(o.roleBy)).toBeVisible()
      await expect(people.chosen(people.stance, o.stance)).toHaveCount(1)
      await expect(people.chosen(people.panelSelect(pt.panel.governance), o.governance)).toHaveCount(1)
      await expect(people.panelButton(o.describe)).toBeVisible()
      await expect(people.detail(o.question)).toBeVisible()
      for (const b of o.answers) await expect(people.panelButton(b)).toBeVisible()

      const r = p.rita
      await people.row(people.inThread, r.name).click()
      await expect(people.panelButton(r.name)).toBeVisible()
      await expect(people.detail(r.note)).toBeVisible()
      await expect(people.value(pt.panel.workstream)).toHaveText(r.workstream)
      await expect(people.chosen(people.panelSelect(pt.panel.worksFor), r.worksFor)).toHaveCount(1)
      await expect(people.value(pt.panel.role)).toHaveText(r.role)
      expect([await people.level(pt.panel.influence), await people.level(pt.panel.interest)]).toEqual([r.influence, r.interest])
    } finally {
      // Teardown: the panel closed.
      if (await people.closePanel(people.panel).count()) await people.closePanel(people.panel).click()
    }
    await expect(people.panel).toHaveCount(0)
  })

  test('15 · Each person\'s side in the List: your company, an organisation, a bare domain, or outside', covers('people/TC-011', 'F-PPL-04', 'F-PPL-07', 'F-CLIENT-07'), async ({ page }) => {
    const s = data.set.s26.sides
    const pt = data.text.people
    const people = new PeoplePage(page, data)
    await people.open(s.thread)
    await people.view(pt.list).click()
    for (const [table, rows] of [[people.inThread, s.in], [people.seen, s.seen]]) {
      for (const [name, org] of rows) expect([name, await people.cellText(table, name, pt.col.organisation)]).toEqual([name, org])
    }
    const i = s.initech
    await people.open(i.id)
    await expect(people.says(i.nobody)).toBeVisible()
    await expect(people.bodyRows(people.seen)).toHaveCount(i.seen)
    for (const [name, org] of i.guests) expect([name, await people.cellText(people.seen, name, pt.col.organisation)]).toEqual([name, org])
    await people.row(people.seen, i.guests[0][0]).click()
    await expect(people.nameOrg).toHaveAccessibleName(i.nameOrg)
    await people.closePanel(people.panel).click()
    await people.view(pt.map).click()
    await expect(people.says(i.mapNobody)).toBeVisible()
    await expect(people.paragraph(i.mapLine.split(' — ')[0])).toHaveText(i.mapLine)
  })

  test('16 · An account is a client, an organisation is not: the CLI\'s lists, the Threads groups, a client\'s page and one that isn\'t', covers('clients/TC-010', 'F-CLIENT-08', 'F-CLIENT-01', 'F-THR-13'), async ({ page }) => {
    const a = data.set.s26.accounts
    const c = data.text.clients
    const ids = (r) => (Array.isArray(r) ? r : r.clients || r.orgs).map((x) => x.id)
    expect(ids(cli('client', 'list'))).toEqual(a.clients)
    const orgs = cli('org', 'list')
    const all = Array.isArray(orgs) ? orgs : orgs.orgs || orgs.clients
    expect(all.map((x) => x.id).sort()).toEqual([...a.clients, ...a.orgs].sort())
    for (const id of a.orgs) expect(all.find((x) => x.id === id)).toMatchObject({ account: false })
    expect(all.find((x) => x.id === a.noDomains).domains).toEqual([])
    const clients = new ClientsPage(page, data)
    await clients.goto(data.text.threads.routes.projects)
    await expect(clients.listHeading(data.text.threads.threads)).toBeVisible()
    for (const g of data.set.s26.list.groups) await expect(clients.clientGroup(g)).toBeVisible()
    for (const id of a.orgs) await expect(clients.clientGroup(all.find((x) => x.id === id).name)).toHaveCount(0)
    await clients.openClient(a.acme.id, c.tabIds.overview)
    await expect(clients.heading(a.acme.name)).toBeVisible()
    for (const text of [c.client, a.acme.line]) await expect(clients.says(text)).toBeVisible()
    for (const id of a.gone) {
      await clients.openClient(id, c.tabIds.overview)
      await expect(clients.subheading(c.gone)).toBeVisible()
      await expect(clients.link(c.backToThreads)).toBeVisible()
    }
    await clients.openThread(a.counterpart.thread, data.text.threads.tabs.profile)
    await expect(clients.row(data.text.threads.rows.orgs)).toContainText(a.counterpart.org)
  })

  test('17 · The Profile\'s other names, mail domains and kind of five clients, as the CLI has them', covers('clients/TC-011', 'F-CLIENT-05', 'F-CLIENT-07'), async ({ page }) => {
    const n = data.set.s26.names
    const c = data.text.clients
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    const listed = Object.fromEntries(cli('client', 'list').map((x) => [x.id, x]))
    for (const [id, aliases, domains, own] of n.clients) {
      await clients.openClient(id, c.tabIds.profile)
      for (const label of n.rows) await expect(clients.row(label)).toBeVisible()
      const remove = (v) => `${t.remove} ${v}`
      await expect.poll(() => clients.rowReads(c.rows.aliases)).toEqual([...tokens(aliases, remove), t.add])
      await expect.poll(() => clients.rowReads(c.rows.domains)).toEqual([...tokens(domains, remove), t.add, own ? n.ownLine : n.outsideLine])
      await expect(clients.switch(c.ownSwitch)).toHaveAttribute('aria-checked', String(own))
      await expect(clients.rowSays(c.rows.kind, own ? n.kinds[1] : n.kinds[0])).toBeVisible()
      expect([id, listed[id].aliases, listed[id].domains]).toEqual([id, aliases, domains])
    }
  })

  test('18 · About and logo, present and absent: Acme\'s Markdown and picture, Globex\'s empty About and letter', covers('clients/TC-012', 'F-CLIENT-02', 'F-CLIENT-05'), async ({ page }) => {
    const a = data.set.s26.about
    const c = data.text.clients
    const clients = new ClientsPage(page, data)
    await clients.openClient(a.acme.id, c.tabIds.overview)
    await expect(clients.heading(a.acme.heading)).toBeVisible()
    await expect(clients.paragraph(a.acme.paragraph)).toHaveText(a.acme.paragraph)
    await expect(clients.bold).toHaveText(a.acme.bold)
    await expect(clients.bullets).toHaveText(a.acme.items)
    await expect(clients.link(a.acme.link)).toHaveAttribute('href', a.acme.href)
    await expect(clients.editAbout).toBeVisible()
    await expect(clients.headImg(data.set.s26.list.groups[0])).toHaveAttribute('src', new RegExp(`^${literal(a.acme.src)}\\d+$`))
    await expect(clients.says(c.client)).toBeVisible()
    for (const [x, want] of [[a.acme, `200 ${a.acme.png}`], [a.globex, String(a.globex.status)]]) {
      const r = await page.request.get(data.base + x.logo)
      expect([x.id, r.status() === 200 ? `200 ${r.headers()['content-type']}` : String(r.status())]).toEqual([x.id, want])
    }
    await clients.openClient(a.acme.id, c.tabIds.profile)
    await expect(clients.rowPicture(c.rows.logo)).toBeVisible()
    for (const b of [a.acme.change, a.acme.remove]) await expect(clients.rowButton(c.rows.logo, b)).toBeVisible()
    await expect(clients.rowSays(c.rows.logo, a.acme.logoLine)).toBeVisible()
    await clients.openClient(a.globex.id, c.tabIds.overview)
    await expect(clients.says(a.globex.empty)).toBeVisible()
    await expect(clients.says(a.globex.letter)).toBeVisible()
    await clients.openClient(a.globex.id, c.tabIds.profile)
    await expect(clients.rowSays(c.rows.logo, a.globex.logoBox)).toBeVisible()
    await expect(clients.rowButton(c.rows.logo, a.globex.add)).toBeVisible()
    await expect(clients.rowButton(c.rows.logo, a.acme.remove)).toHaveCount(0)
    await clients.goto(data.text.threads.routes.projects)
    await clients.closedFold.click()
    const logo = new RegExp(`^${literal(a.acme.src)}\\d+$`)
    await expect(clients.groupImg(a.list.group)).toHaveAttribute('src', logo)
    await expect(clients.rowImg(a.list.closedRow)).toHaveAttribute('src', logo)
    await expect(clients.rowImg(a.list.own)).toBeVisible()
  })

  test('19 · A client with only closed threads: no row in the list, its page, one closed card, its tabs, kept on Profile', covers('clients/TC-013', 'F-CLIENT-01', 'F-CLIENT-02', 'F-CLIENT-04', 'F-CLIENT-06'), async ({ page }) => {
    const x = data.set.s26.onlyClosed
    const c = data.text.clients
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    await clients.goto(t.routes.projects)
    await expect(clients.clientGroup(x.name)).toBeVisible()
    await expect(clients.fold(x.name)).toHaveCount(0)
    await expect(clients.threadRow(x.thread)).toHaveCount(0)
    await clients.closedFold.click()
    await expect(clients.threadRow(x.thread)).toBeVisible()
    await clients.group(x.name).click()
    await expect.poll(() => clients.hash()).toBe(clientRoute(x.id, t.tabs.overview))
    await expect(clients.heading(x.name)).toBeVisible()
    for (const text of [c.client, x.line]) await expect(clients.says(text)).toBeVisible()
    await expect(clients.tabs).toHaveText(x.tabs)
    const p = show(x.threadId)
    const day = await clients.cardDay(p.last_call)
    await expect.poll(() => clients.cardReads(x.thread)).toEqual([`${x.thread} ${x.stage}`, `${fill(x.card[0], { day })} ${x.card[1]}`])
    expect(await clients.opacityOf(clients.overviewCard(x.thread))).toBeLessThan(1)
    await clients.tab(c.tabs.calls).click()
    await expect(clients.callRows).toHaveCount(1)
    await expect(clients.callRow(x.call)).toContainText(x.thread)
    await clients.tab(c.tabs.tasks).click()
    const tasks = new TasksPage(page, data)
    for (const pile of data.text.tasks.piles) {
      await expect(clients.pile(pile, 0)).toBeVisible()
      await expect(tasks.pileArea(pile)).toContainText(data.text.tasks.nothing)
    }
    await expect(tasks.others()).toHaveCount(0)
    await clients.tab(c.tabs.people).click()
    const people = new PeoplePage(page, data)
    await expect(people.says(x.nobody)).toBeVisible()
    for (const line of x.peopleLines) await expect(people.paragraph(line.split(' — ')[0])).toHaveText(line)
    await clients.tab(c.tabs.profile).click()
    await expect.poll(() => clients.rowReads(c.rows.threads)).toEqual(x.threadsRow)
    await expect(clients.says(x.stays)).toBeVisible()
    await expect(clients.removeClient).toHaveCount(0)
  })

  test('20 · A client with no threads: its page, empty tabs, Remove offered on Profile, its calls a Found for you card', covers('clients/TC-014', 'F-CLIENT-01', 'F-CLIENT-04', 'F-CLIENT-06', 'F-THR-18'), async ({ page }) => {
    const x = data.set.s26.noThreads
    const c = data.text.clients
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    await clients.goto(t.routes.projects)
    await expect(clients.clientGroup(x.name)).toBeVisible()
    await expect(clients.fold(x.name)).toHaveCount(0)
    await clients.group(x.name).click()
    await expect.poll(() => clients.hash()).toBe(clientRoute(x.id, t.tabs.overview))
    await expect(clients.heading(x.name)).toBeVisible()
    for (const text of [c.client, x.line, x.about, x.empty, x.noThreads]) await expect(clients.says(text)).toBeVisible()
    await clients.tab(c.tabs.calls).click()
    await expect(clients.says(x.noCalls)).toBeVisible()
    await expect(clients.tab(c.tabs.calls)).toHaveAccessibleName(c.tabs.calls)
    await clients.tab(c.tabs.tasks).click()
    const tasks = new TasksPage(page, data)
    for (const pile of data.text.tasks.piles) {
      await expect(clients.pile(pile, 0)).toBeVisible()
      await expect(tasks.pileArea(pile)).toContainText(data.text.tasks.nothing)
    }
    await expect(tasks.others()).toHaveCount(0)
    await clients.tab(c.tabs.people).click()
    const people = new PeoplePage(page, data)
    await expect(people.says(x.nobody)).toBeVisible()
    await expect(people.paragraph(x.join)).toHaveText(x.join)
    await expect(people.addPerson).toHaveCount(0)
    await clients.tab(c.tabs.profile).click()
    await expect.poll(() => clients.rowReads(c.rows.threads)).toEqual(x.threadsRow)
    await expect(clients.removeClient).toBeVisible()
    await clients.goto(t.routes.found)
    const f = x.found
    await expect(clients.foundSays(f.card, f.why)).toBeVisible()
    await expect(clients.chosen(clients.foundSelect(f.card, t.rows.client), f.client)).toHaveCount(1)
    for (const title of f.calls) await expect(clients.foundCall(f.card, title)).toBeVisible()
  })

  test('21 · The user\'s company with a thread: its place in the list, "your company", its card, its Profile and People', covers('clients/TC-017', 'F-CLIENT-07', 'F-CLIENT-02', 'F-THR-01'), async ({ page }) => {
    const x = data.set.s26.own
    const c = data.text.clients
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    await clients.goto(t.routes.projects)
    await expect(clients.listHeading(t.threads)).toBeVisible()
    const lines = await clients.listLines()
    expect(lines.indexOf(x.group)).toBe(lines.indexOf(x.after) + 1)
    expect(lines.indexOf(x.before)).toBe(lines.indexOf(x.group) + 2)
    const p = show(x.row[3])
    const day = await clients.dayLabel(p.last_call)
    expect(lines[lines.indexOf(x.group) + 1]).toBe(clients.rowName({ name: x.row[0], calls: x.row[1], day, stage: x.row[2] }))
    await clients.group(x.group).click()
    await expect.poll(() => clients.hash()).toBe(clientRoute(x.id, t.tabs.overview))
    for (const text of [x.tag, x.line]) await expect(clients.says(text)).toBeVisible()
    await expect(clients.says(c.client)).toHaveCount(0)
    const k = x.card
    await expect(clients.cardButton(k.name, `${k.name} ${k.stage}`)).toBeVisible()
    await expect(clients.cardSays(k.name, fill(k.lines[0], { calls: callCount(x.row[1]), day: await clients.cardDay(p.last_call) }))).toBeVisible()
    for (const line of k.lines.slice(1)) await expect(clients.cardSays(k.name, line)).toBeVisible()
    for (const d of k.decided) await expect(clients.cardButton(k.name, d)).toBeVisible()
    // The risk without a response first, then the one with its response.
    const reads = await clients.cardReads(k.name)
    const at = k.risks.map((r) => reads.indexOf(r))
    expect(at.every((i, n) => i >= 0 && (n === 0 || i > at[n - 1]))).toBe(true)
    await clients.tab(c.tabs.profile).click()
    await expect(clients.switch(c.ownSwitch)).toHaveAttribute('aria-checked', 'true')
    for (const text of [x.kind, c.ownHint, c.keeps]) await expect(clients.says(text)).toBeVisible()
    await expect.poll(() => clients.rowReads(c.rows.threads)).toEqual(x.threadsRow)
    await clients.tab(c.tabs.people).click()
    const people = new PeoplePage(page, data)
    await people.view(data.text.people.list).click()
    await expect(people.says(data.set.s26.clientPeople.own.nobody)).toBeVisible()
    await expect(people.bodyRows(people.seen)).toHaveCount(x.seen)
  })

  test('22 · A client\'s Overview cards: open threads first with their counts and blocks, the closed one last and dimmed; a risk and a card open their place', covers('clients/TC-015', 'F-CLIENT-02'), async ({ page }) => {
    const o = data.set.s26.overview
    const c = data.text.clients
    const clients = new ClientsPage(page, data)
    await clients.openClient(o.id, c.tabIds.overview)
    const names = o.cards.map((k) => k.name)
    await expect.poll(() => clients.cardOrder(names)).toEqual(names)
    const open = await clients.opacityOf(clients.overviewCard(names[0]))
    const red = await clients.themeColour(data.text.tasks.colours.late)
    for (const k of o.cards) {
      const logo = data.set.s26.list.logo.includes(k.name)
      await expect(clients.cardButton(k.name, [k.name, logo && k.name, k.stage].filter(Boolean).join(' '))).toBeVisible()
      const p = show(k.id)
      const calls = callCount(p.call_count)
      await expect(clients.cardSays(k.name, fill(c.cardCalls, { calls, day: await clients.cardDay(p.last_call) }))).toBeVisible()
      if (k.stories) await expect(clients.cardSays(k.name, k.stories)).toBeVisible()
      for (const line of k.tasks) await expect(clients.cardSays(k.name, line)).toBeVisible()
      if (!k.tasks.length) await expect(clients.cardSays(k.name, o.noTasks)).toBeVisible()
      if (k.late) expect(await clients.colourOf(clients.cardSays(k.name, k.late))).toBe(red)
      for (const b of [...k.decided, ...k.risks, ...k.questions]) await expect(clients.cardButton(k.name, b)).toBeVisible()
      const has = [k.decided, k.risks, k.questions]
      for (const [i, label] of o.blocks.entries()) await expect(clients.cardSays(k.name, label)).toHaveCount(has[i].length ? 1 : 0)
      expect([k.name, (await clients.opacityOf(clients.overviewCard(k.name))) < open]).toEqual([k.name, !!k.closed])
    }
    // 4: the risk opens its thread's Work tab (its Details: "after" below, the last test; it fails while they don't open).
    await clients.cardButton(names[1], o.risk.button).click()
    await expect.poll(() => clients.hash()).toBe(threadRoute(o.risk.thread, data.text.threads.tabs.work))
    await clients.openClient(o.id, c.tabIds.overview)
    const last = o.cards.at(-1)
    await clients.cardButton(last.name, [last.name, last.name, last.stage].join(' ')).click()
    await expect.poll(() => clients.hash()).toMatch(new RegExp(`^${literal(fill(c.routes.thread, { id: last.id }))}`))
  })

  test('23 · A client\'s Calls and Tasks tabs over its three threads: every call with its thread, the piles of its open threads', covers('clients/TC-016', 'F-CLIENT-03', 'F-CLIENT-04'), async ({ page }) => {
    const x = data.set.s26.clientTabs
    const c = data.text.clients
    const clients = new ClientsPage(page, data)
    await clients.openClient(x.id, c.tabIds.calls)
    await expect(clients.tab(c.tabs.calls)).toHaveAccessibleName(fill(data.text.threads.tabCount, { tab: c.tabs.calls, n: x.calls.length }))
    const all = [...new Map(x.threadIds.flatMap((id) => show(id).calls).map((k) => [k.name, k])).values()]
      .sort((a, b) => b.started_at.localeCompare(a.started_at))
    expect(all.map((k) => k.title)).toEqual(x.calls.map(([title]) => title))
    const want = []
    for (const [i, [title, chip, noMinutes]] of x.calls.entries()) want.push(await clientCallRow(clients, all[i], title, chip, noMinutes))
    await expect.poll(() => clients.callRowsRead()).toEqual(want)
    await clients.callButton(x.open, x.open).click()
    await expect.poll(() => clients.hash()).toMatch(new RegExp(`^${literal(data.text.threads.routes.call)}`))

    await clients.openClient(x.id, c.tabIds.tasks)
    await expect(clients.tab(c.tabs.tasks)).toHaveAccessibleName(fill(data.text.threads.tabCount, { tab: c.tabs.tasks, n: x.tasks }))
    const t = data.text.tasks
    const tasks = new TasksPage(page, data)
    for (const [i, pile] of t.piles.entries()) await expect(clients.pile(pile, x.piles[i])).toBeVisible()
    await expect(tasks.others(x.others)).toBeVisible()
    await expect(tasks.cards(t.piles[2])).toHaveCount(x.waiting.length)
    for (const [title, thread] of x.waiting) await expect(tasks.card(t.piles[2], title, thread)).toHaveCount(1)
    await tasks.others(x.others).click()
    await expect(tasks.cards(t.others)).toHaveCount(x.others)
    for (const [thread, n] of Object.entries(x.othersBy)) await expect(tasks.cardsOf(t.others, thread)).toHaveCount(n)

    // 6: Tasks filtered by each thread gives the same cards.
    await tasks.open()
    await tasks.pileArea(t.iOwe).waitFor()
    const sums = [0, 0, 0, 0]
    let others = 0
    for (const option of x.threads) {
      await tasks.select(t.filters.thread).selectOption({ label: option })
      await expect(tasks.chosen(t.filters.thread, option)).toHaveCount(1)
      for (const [i, pile] of t.piles.entries()) sums[i] += await tasks.pileCount(pile)
      others += await tasks.othersCount()
    }
    expect([...sums, others]).toEqual([...x.piles, x.others])
  })

  test('24 · Since keeps older calls out of a thread; moved back, the older call comes in by its subject', covers('threads/TC-021', 'F-THR-14', 'F-THR-15', 'F-THR-19'), async ({ page }) => {
    const x = data.set.s26.since
    const t = data.text.threads
    expect(of(x.call)).toEqual({ projects: [], suggested: x.suggested })
    const clients = new ClientsPage(page, data)
    const p = show(x.thread)
    await clients.openThread(x.thread, t.tabs.calls)
    await expect(clients.factStarting(p.name, fill(t.headCalls, { n: x.calls, since: p.since }))).toBeVisible()
    await expect(clients.tab(t.tabs.callsTab)).toHaveAccessibleName(fill(t.tabCount, { tab: t.tabs.callsTab, n: x.calls }))
    await expect(clients.callRow(x.title)).toHaveCount(0)
    await clients.openThread(x.thread, t.tabs.profile)
    expect(p.since).toBe(`${new Date().getFullYear()}-01-01`)
    await expect(clients.rowButton(t.rows.since, await clients.sinceDay(p.since))).toBeVisible()
    await expect(clients.rowSays(t.rows.since, x.olderNot)).toBeVisible()
    await clients.openThread(x.firstCall.id, t.tabs.profile)
    await expect(clients.rowButton(t.rows.since, x.firstCall.says)).toBeVisible()
    for (const [id, days] of x.rows) {
      const since = show(id).since
      expect([id, since]).toEqual([id, daysAgo(days)])
      await clients.openThread(id, t.tabs.profile)
      await expect(clients.rowButton(t.rows.since, await clients.sinceDay(since))).toBeVisible()
      await expect(clients.rowSays(t.rows.since, x.olderNot)).toBeVisible()
    }
    // 6–8: the Since date moved back a year (kept: the last stage, its teardown skipped).
    cli('project', 'set', x.thread, '--since', fill(x.moved, { lastYear: new Date().getFullYear() - 1 }))
    await clients.openThread(x.thread, t.tabs.calls)
    await expect(clients.tab(t.tabs.callsTab)).toHaveAccessibleName(fill(t.tabCount, { tab: t.tabs.callsTab, n: x.after.calls }))
    await expect(clients.callRow(x.title)).toContainText(x.after.why)
    await expect(clients.callButton(x.title, x.after.button)).toBeVisible()
    expect(of(x.call)).toEqual({ projects: [{ id: x.thread, why: x.after.of }], suggested: x.after.suggested })
  })

  // clients/TC-015's step 4, its Details, run last: the other stages go on while it fails.
  test("after · A risk on a client's Overview card opens its Details in the right panel (clients/TC-015 step 4, #314)", covers('clients/TC-015', 'F-CLIENT-02'), async ({ page }) => {
    const o = data.set.s26.overview
    const clients = new ClientsPage(page, data)
    await clients.openClient(o.id, data.text.clients.tabIds.overview)
    await clients.cardButton(o.cards[1].name, o.risk.button).click()
    const work = new WorkPage(page, data)
    await expect.poll(() => work.hash()).toBe(threadRoute(o.risk.thread, data.text.threads.tabs.work))
    await expect(work.panelTab(work.details, data.text.work.details)).toHaveAttribute('aria-selected', 'true')
    await expect(work.itemTitle).toHaveText(o.risk.title)
  })
})
