// S-07 Clients and people — its stages with fixed checks (small data).
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { ClientsPage } from '../pages/clients.page.mjs'
import { PeoplePage } from '../pages/people.page.mjs'

const data = useData(test, { set: 'small' })

/** A key of the window's shortcuts, by its name in texts.json. */
const key = (name) => data.text.shell.keys[name]
/** `echoctl …` that must succeed, its output read as JSON. */
const cli = (...args) => {
  const r = data.cli(...args)
  if (r.code !== 0) throw new Error(`${args.join(' ')}: ${r.err}`)
  return r.json()
}
/** A person of people-lab as `echoctl project people` gives them, and their place in the thread. */
const person = (name) => cli('project', 'people', data.set.people.thread.id).people.find((p) => p.name === name)
const place = (name) => (person(name).places || []).find((p) => p.thread === data.set.people.thread.id) || {}

scenario(test, 'S-07', "Clients and people: a client's page and tabs, a new thread and domains, a thread's people, a person's details", () => {
  test('1 · Initech is your own company: its group, its tag, no threads, the switch on', covers('clients/TC-008', 'F-CLIENT-07'), async ({ page }) => {
    const { own, groups } = data.set.clients
    const t = data.text.clients
    const clients = new ClientsPage(page, data)
    await clients.goto(t.routes.projects)
    await expect(clients.group(own.group)).toBeVisible()
    for (const name of groups) await expect(clients.group(name)).toBeVisible()
    await clients.group(own.group).click()
    await expect.poll(() => clients.hash()).toBe(fill(t.routes.client, { id: own.id, tab: 'overview' }))
    await expect(clients.says(t.yourCompany)).toBeVisible()
    await expect(clients.says(own.facts)).toBeVisible()
    await clients.tab(t.tabs.overview).click()
    await expect(clients.says(t.noThreads)).toBeVisible()
    await clients.tab(t.tabs.profile).click()
    await expect(clients.main.getByRole('switch', { name: t.ownSwitch })).toBeChecked()
    await expect(clients.says(t.ownLine)).toBeVisible()
    await expect(clients.says(t.ownHint)).toBeVisible()
  })

  test("2 · Open Acme from its thread's menu: its header and tabs; Globex from the left list", covers('clients/TC-001', 'F-CLIENT-01'), async ({ page }) => {
    const { acme, globex } = data.set.clients
    const t = data.text.clients
    const clients = new ClientsPage(page, data)
    await clients.openThread(acme.thread.id, 'brief')
    await clients.threadActions.click()
    await clients.menuItem(fill(t.openClient, { name: acme.name })).click()
    await expect.poll(() => clients.hash()).toBe(fill(t.routes.client, { id: acme.id, tab: 'overview' }))
    await expect(clients.heading(acme.name)).toBeVisible()
    await expect(clients.says(t.client)).toBeVisible()
    await expect(clients.says(acme.facts)).toBeVisible()
    await expect(clients.claudeAbout).toHaveAccessibleName(`Claude about ${acme.name}`)
    await expect(clients.tabs).toHaveText(acme.tabs)
    await expect(clients.tab(t.tabs.overview)).toHaveAttribute('aria-selected', 'true')
    await clients.group(globex.name).click()
    await expect.poll(() => clients.hash()).toBe(fill(t.routes.client, { id: globex.id, tab: 'overview' }))
    await expect(clients.heading(globex.name)).toBeVisible()
    await expect(clients.says(globex.facts)).toBeVisible()
  })

  test("3 · Globex's Profile: its rows; another name and a domain added and removed", covers('clients/TC-004', 'F-CLIENT-05'), async ({ page }) => {
    const { globex: g } = data.set.clients
    const t = data.text.clients
    const r = t.rows
    const clients = new ClientsPage(page, data)
    await clients.openClient(g.id, 'profile')
    await expect(clients.row(r.name).getByRole('button', { name: g.name, exact: true })).toBeVisible()
    await expect(clients.addIn(r.aliases)).toBeVisible()
    await expect(clients.token(r.domains, g.domain)).toBeVisible()
    await expect(clients.removeToken(g.domain)).toBeVisible()
    await expect(clients.row(r.kind)).toContainText(t.notOwn)
    await expect(clients.row(r.logo)).toBeVisible()
    await expect(clients.card(g.thread)).toBeVisible()
    await expect(clients.newThread).toBeVisible()
    await expect(clients.says(t.domainsHint)).toBeVisible()

    await clients.addIn(r.aliases).click()
    await clients.addBox(r.aliases).fill(g.alias)
    await clients.addBox(r.aliases).press(key('enter'))
    await expect(clients.token(r.aliases, g.alias)).toBeVisible()
    await expect(clients.removeToken(g.alias)).toBeVisible()
    await clients.addIn(r.domains).click()
    await clients.addBox(r.domains).fill(g.newDomain)
    await clients.addBox(r.domains).press(key('enter'))
    for (const d of [g.domain, g.newDomain]) await expect(clients.token(r.domains, d)).toBeVisible()
    await expect.poll(() => cli('client', 'list').find((c) => c.id === g.id).domains).toEqual([g.domain, g.newDomain])

    await clients.removeToken(g.alias).click()
    await expect(clients.token(r.aliases, g.alias)).toHaveCount(0)
    await clients.removeToken(g.newDomain).click()
    await expect(clients.token(r.domains, g.newDomain)).toHaveCount(0)
    await expect(clients.token(r.domains, g.domain)).toBeVisible()
    await expect.poll(() => cli('client', 'list').find((c) => c.id === g.id)).toMatchObject({ aliases: [], domains: [g.domain] })
  })

  test('4 · + New thread on a client makes a thread under it', covers('clients/TC-005', 'F-CLIENT-05'), async ({ page }) => {
    const { globex: g } = data.set.clients
    const t = data.text.clients
    const clients = new ClientsPage(page, data)
    await clients.openClient(g.id, 'profile')
    await clients.newThread.click()
    await expect(clients.newThreadName).toHaveAttribute('placeholder', g.newThread.placeholder)
    await clients.newThreadName.fill(g.newThread.name)
    await clients.newThreadName.press(key('enter'))
    await expect.poll(() => clients.hash()).toBe(`${fill(t.routes.thread, { id: g.newThread.id })}profile`)
    await expect(clients.heading(g.newThread.name)).toBeVisible()
    await expect(clients.clientTag).toHaveText(g.name)
    expect(cli('project', 'show', g.newThread.id).client).toBe(g.id)

    // Teardown: the thread deleted.
    expect(data.cli('project', 'delete', g.newThread.id).code).toBe(0)
  })

  test("5 · A client's Overview: About edited and saved, Escape keeps it; the thread card opens the thread", covers('clients/TC-002', 'F-CLIENT-02'), async ({ page }) => {
    const { acme: a } = data.set.clients
    const t = data.text.clients
    const clients = new ClientsPage(page, data)
    await clients.openClient(a.id, 'overview')
    await expect(clients.says(fill(t.aboutHead, { name: a.name }))).toBeVisible()
    await expect(clients.editAbout).toBeVisible()
    await expect(clients.says(t.aboutEmpty)).toBeVisible()
    await expect(clients.card(a.card)).toBeVisible()
    for (const line of a.cardLines) await expect(clients.says(line)).toBeVisible()
    await clients.editAbout.click()
    await clients.aboutBox.fill(a.about)
    await clients.aboutBox.press(key('save'))
    await expect(clients.says(a.about)).toBeVisible()
    await clients.editAbout.click()
    await clients.aboutBox.press('End')
    await clients.aboutBox.pressSequentially(a.typed)
    await clients.aboutBox.press(key('escape'))
    await expect(clients.aboutBox).toHaveCount(0)
    await expect(clients.says(a.about)).toBeVisible()
    await clients.card(a.card).click()
    await expect.poll(() => clients.hash()).toMatch(new RegExp(`^${fill(t.routes.thread, { id: a.thread.id })}\\w+$`))

    // Teardown: About cleared.
    await clients.openClient(a.id, 'overview')
    await clients.editAbout.click()
    await clients.aboutBox.fill('')
    await clients.aboutBox.press(key('save'))
    await expect(clients.says(t.aboutEmpty)).toBeVisible()
  })

  test("6 · A client's Calls and Tasks tabs: its call with its thread; its piles; a card opens its Details", covers('clients/TC-003', 'F-CLIENT-03', 'F-CLIENT-04'), async ({ page }) => {
    const { acme: a } = data.set.clients
    const t = data.text.clients
    const clients = new ClientsPage(page, data)
    await clients.openClient(a.id, 'calls')
    await expect(clients.callRows).toHaveCount(1)
    const row = clients.callRows.first()
    await expect(row.getByRole('button', { name: a.call, exact: true })).toBeVisible()
    await expect(row).toContainText(a.callDay)
    await expect(row).toContainText(a.thread.name)
    await expect(row).not.toContainText(t.noMinutes)
    await expect(clients.main.getByRole('button', { name: a.notCall })).toHaveCount(0)

    await clients.tab(t.tabs.tasks).click()
    for (const [name, n] of a.piles) await expect(clients.pile(name, n)).toBeVisible()
    await expect(clients.task(a.mine)).toBeVisible()
    await expect(clients.task(a.other)).toHaveCount(0)
    await clients.others(a.others).click()
    await expect(clients.task(a.other)).toContainText(a.otherOwner)
    await clients.task(a.mine).click()
    await expect(clients.panelTab(clients.rightPanel, data.text.work.details)).toHaveAttribute('aria-selected', 'true')
    await expect(clients.rightPanel.getByRole('heading', { name: a.mine, level: 3 })).toBeVisible()
    await expect(clients.rightPanel.getByText(a.detailsPath, { exact: true })).toBeVisible()

    // Teardown: Others folded again.
    await clients.others(a.others).click()
    await expect(clients.task(a.other)).toHaveCount(0)
  })

  test('7 · A client with threads cannot be removed, in the window or the CLI', covers('clients/TC-007', 'F-CLIENT-06'), async ({ page }) => {
    const { acme: a } = data.set.clients
    const t = data.text.clients
    const clients = new ClientsPage(page, data)
    await clients.openClient(a.id, 'profile')
    await expect(clients.says(t.keeps)).toBeVisible()
    await expect(clients.removeClient).toHaveCount(0)
    const r = data.cli('client', 'remove', a.id)
    expect(r.code).not.toBe(0)
    expect(r.err).toContain(fill(t.stillHas, { id: a.id }))
    expect(cli('client', 'list').map((c) => c.id)).toContain(a.id)
  })

  test('8 · A client without threads: Remove asks, Cancel keeps it, Remove takes it away', covers('clients/TC-006', 'F-CLIENT-06'), async ({ page }) => {
    const { hooli: h } = data.set.clients
    const t = data.text.clients
    expect(data.cli('client', 'add', h.name, '--domains', h.domain).code).toBe(0)
    const clients = new ClientsPage(page, data)
    await clients.openClient(h.id, 'profile')
    await expect(clients.heading(h.name)).toBeVisible()
    await expect(clients.newThread).toBeVisible()
    await expect(clients.removeClient).toBeVisible()
    await clients.removeClient.click()
    await expect(clients.says(fill(t.removeAsk, { name: h.name }))).toBeVisible()
    await expect(clients.cancel).toBeVisible()
    await expect(clients.confirmRemove).toBeVisible()
    await clients.cancel.click()
    await expect(clients.says(fill(t.removeAsk, { name: h.name }))).toHaveCount(0)
    await expect(clients.removeClient).toBeVisible()
    await clients.removeClient.click()
    await clients.confirmRemove.click()
    await expect.poll(() => clients.hash()).toBe(t.routes.projects)
    await expect(clients.group(h.group)).toHaveCount(0)
    await clients.goto(fill(t.routes.client, { id: h.id, tab: 'overview' }))
    await expect(clients.main.getByRole('heading', { name: t.gone })).toBeVisible()
    await expect(clients.main.getByRole('link', { name: t.backToThreads })).toBeVisible()
    expect(cli('client', 'list').map((c) => c.id)).not.toContain(h.id)
  })

  test("9 · An organisation that isn't a client: added and listed by the CLI, not a client group, offered to a thread, removed", covers('clients/TC-009', 'F-CLIENT-08'), async ({ page }) => {
    const { org: o, acme: a } = data.set.clients
    const t = data.text.clients
    expect(cli('org', 'add', o.name, '--domains', o.domain)).toMatchObject({ account: false, id: o.id, domains: [o.domain] })
    const orgs = cli('org', 'list')
    expect(orgs.find((x) => x.id === o.id)).toMatchObject({ account: false })
    expect(orgs.find((x) => x.id === a.id)).not.toHaveProperty('account')
    const clients = new ClientsPage(page, data)
    await clients.goto(t.routes.projects)
    await expect(clients.group(data.set.clients.groups[0])).toBeVisible()
    await expect(clients.group(o.name)).toHaveCount(0)
    await clients.openThread(a.thread.id, 'profile')
    await clients.addOrganisation.click()
    for (const option of o.options) await expect(clients.organisation.getByRole('option', { name: option, exact: true })).toHaveCount(1)
    const r = data.cli('org', 'remove', a.id)
    expect(r.code).not.toBe(0)
    expect(r.err).toContain(fill(t.stillHas, { id: a.id }))
    const removed = data.cli('org', 'remove', o.id)
    expect(removed.code).toBe(0)
    expect(removed.out).toContain(fill(t.orgRemoved, { id: o.id }))
    await clients.organisation.press(key('escape'))
  })

  test("10 · An empty thread offers the people from its calls; the view choice is kept", covers('people/TC-001', 'F-PPL-01', 'F-PPL-03'), async ({ page }) => {
    const p = data.set.people
    const t = data.text.people
    // The scenario's thread, with the Acme call (kept for the stages after this one).
    expect(data.cli('project', 'create', p.thread.name).code).toBe(0)
    expect(data.cli('project', 'include', p.thread.id, data.callName(p.call)).code).toBe(0)
    const people = new PeoplePage(page, data)
    await people.open(p.thread.id)
    await expect(people.views.getByRole('button')).toHaveText(t.views)
    await expect(people.view(t.map)).toHaveAttribute('aria-pressed', 'true')
    for (const s of t.legend) await expect(people.says(s)).toBeVisible()
    await expect(people.addPerson).toBeVisible()
    await expect(people.says(t.nobody)).toBeVisible()
    await expect(people.says(fill(t.seenLine, { n: p.seen.length }))).toBeVisible()
    await people.view(t.list).click()
    await expect(people.view(t.list)).toHaveAttribute('aria-pressed', 'true')
    await expect(people.says(t.nobodyList)).toBeVisible()
    await expect(people.says(t.seenCap)).toBeVisible()
    await expect(people.says(fill(t.notIn, { n: p.seen.length }))).toBeVisible()
    for (const x of p.seen) await expect(people.row(people.seen, x.name)).toBeVisible()
    await people.open(p.thread.id)
    await expect(people.view(t.list)).toHaveAttribute('aria-pressed', 'true')
    await people.view(t.org).click()
    await expect(people.view(t.org)).toHaveAttribute('aria-pressed', 'true')
    await expect(people.main.getByRole('table')).toHaveCount(0)
    await people.view(t.map).click()
    await expect(people.view(t.map)).toHaveAttribute('aria-pressed', 'true')
  })

  test("11 · The List: who was seen in the calls, with their facts; Add puts Dana Lee in the thread", covers('people/TC-003', 'F-PPL-04'), async ({ page }) => {
    const p = data.set.people
    const t = data.text.people
    const [dana, omar] = p.seen
    const people = new PeoplePage(page, data)
    await people.open(p.thread.id)
    await people.view(t.list).click()
    await expect(people.columns(people.seen)).toHaveText(t.columns)
    const row = people.row(people.seen, dana.name)
    for (const text of [dana.email, dana.org, dana.calls, dana.last]) await expect(row).toContainText(text)
    await expect(row.getByRole('button', { name: t.add, exact: true })).toBeVisible()
    const other = people.row(people.seen, omar.name)
    for (const text of [omar.org, omar.calls]) await expect(other).toContainText(text)
    await expect(other.getByRole('button', { name: t.add, exact: true })).toBeVisible()
    // Owes: empty (people-lab has no tasks).
    await expect(other.getByRole('cell').nth(6)).toHaveText('')
    await row.getByRole('button', { name: t.add, exact: true }).click()
    await expect(people.row(people.inThread, dana.name)).toBeVisible()
    await expect(people.says(fill(t.notInOne, { n: 1 }))).toBeVisible()
    await expect(people.seen.getByRole('row')).toHaveCount(2)
    await expect(people.row(people.seen, omar.name)).toBeVisible()
  })

  test('12 · The Map: Dana Lee dragged from the tray to Manage closely, then to Monitor', covers('people/TC-004', 'F-PPL-02'), async ({ page }) => {
    const p = data.set.people
    const t = data.text.people
    const people = new PeoplePage(page, data)
    await people.open(p.thread.id)
    await people.view(t.map).click()
    for (const q of t.quadrants) await expect(people.quadrant(q)).toBeVisible()
    await expect(people.tray.getByRole('button', { name: new RegExp(`^${p.dana.name} `) })).toBeVisible()
    await people.chip(p.dana.name).dragTo(people.quadrant(p.place.to))
    await expect(people.quadrant(p.place.to).getByRole('button', { name: new RegExp(`^${p.dana.name} `) })).toBeVisible()
    await expect(people.tray).toHaveCount(0)
    await expect.poll(() => place(p.dana.name)).toMatchObject({ influence: p.place.influence, interest: p.place.interest })
    await people.chip(p.dana.name).dragTo(people.quadrant(p.place.then))
    await expect(people.quadrant(p.place.then).getByRole('button', { name: new RegExp(`^${p.dana.name} `) })).toBeVisible()
    await expect(people.quadrant(p.place.to).getByRole('button')).toHaveCount(0)
  })

  test("13 · Suggestions from the calls: Yes applies one, No turns one down, Change… saves your own version", covers('people/TC-006', 'F-PPL-06'), async ({ page }) => {
    const p = data.set.people
    const t = data.text.people
    const [evidence] = cli('project', 'ledger', p.thread.id, 'sync').pending
    const file = path.join(data.dir, 'people-suggestions.json')
    fs.writeFileSync(file, JSON.stringify(p.suggestions))
    expect(cli('project', 'ledger', p.thread.id, 'people-apply', '--evidence', evidence.id, '--file', file)).toMatchObject({ added: p.added })
    const people = new PeoplePage(page, data)
    await people.open(p.thread.id)
    await expect(people.suggestions).toHaveCount(3)
    await expect(people.suggestions.first()).toContainText(p.firstRow)
    for (const [i, ask] of p.asks.entries()) {
      await expect(people.suggestions.nth(i)).toContainText(ask)
      for (const name of t.answers) await expect(people.suggestions.nth(i).getByRole('button', { name, exact: true })).toBeVisible()
    }

    await people.answer(p.asks[0], t.answers[0]).click()
    await expect(people.suggestion(p.asks[0])).toHaveCount(0)
    await expect.poll(() => place(p.dana.name).stance).toBe(p.suggestions[0].value)
    expect(person(p.dana.name).in_thread).toBe(true)
    await people.answer(p.asks[1], t.answers[1]).click()
    await expect(people.suggestion(p.asks[1])).toHaveCount(0)
    expect(place(p.omar.name).influence ?? null).toBeNull()
    await people.answer(p.asks[2], t.answers[2]).click()
    const editing = people.suggestion(p.changing)
    await expect(editing.getByRole('textbox', { name: t.yourVersion })).toHaveValue(p.suggestions[2].value)
    await expect(editing.getByRole('button', { name: t.save, exact: true })).toBeVisible()
    await expect(editing.getByRole('button', { name: t.cancel, exact: true })).toBeVisible()
    await editing.getByRole('textbox', { name: t.yourVersion }).fill(p.changed)
    await editing.getByRole('button', { name: t.save, exact: true }).click()
    await expect(people.suggestions).toHaveCount(0)
    await expect.poll(() => person(p.omar.name).title).toBe(p.changed)
  })

  test("14 · A person's panel: title, stance and role edited in place", covers('people/TC-005', 'F-PPL-05'), async ({ page }) => {
    const p = data.set.people
    const t = data.text.people
    const pn = t.panel
    const people = new PeoplePage(page, data)
    await people.open(p.thread.id)
    await people.view(t.map).click()
    await people.chip(p.dana.name).click()
    await expect(people.panelTab(people.panel, pn.tab)).toHaveAttribute('aria-selected', 'true')
    await expect(people.panel.getByRole('button', { name: p.dana.name, exact: true })).toBeVisible()
    await expect(people.detail(p.dana.org)).toBeVisible()
    for (const s of pn.sections) await expect(people.detail(fill(s, { thread: p.thread.name }))).toBeVisible()
    await expect(people.panel.getByRole('button', { name: pn.describe, exact: true })).toBeVisible()
    for (const label of [...pn.inThread, ...pn.about]) await expect(people.detail(label)).toBeVisible()
    await expect(people.chosen(people.organisation, p.orgByDomain)).toHaveCount(1)
    await expect(people.detail(p.saw)).toBeVisible()

    const title = await people.editField(pn.title)
    await title.fill(p.edit.title)
    await title.press(key('enter'))
    await expect(people.value(pn.title)).toHaveText(p.edit.title)
    await expect(people.detail(p.edit.subtitle)).toBeVisible()
    await people.stance.selectOption({ label: p.edit.stance })
    await expect(people.chosen(people.stance, p.edit.stance)).toHaveCount(1)
    const role = await people.editField(pn.role)
    await role.fill(p.edit.role)
    await role.press(key('enter'))
    await expect(people.value(pn.role)).toHaveText(p.edit.role)
    await expect.poll(() => person(p.dana.name).title).toBe(p.edit.title)
    expect(place(p.dana.name)).toMatchObject({ stance: p.edit.stance, role: p.edit.role })
  })

  test('15 · Same person: Dana L merged into Dana Lee, then split again', covers('people/TC-007', 'F-PPL-07'), async ({ page }) => {
    const p = data.set.people
    const t = data.text.people
    const people = new PeoplePage(page, data)
    await people.open(p.thread.id)
    await people.view(t.list).click()
    await people.addPerson.click()
    await people.newPerson.fill(p.dup.entry)
    await people.newPerson.press(key('enter'))
    for (const name of [p.dana.name, p.dup.name]) await expect(people.row(people.inThread, name)).toBeVisible()
    await people.row(people.inThread, p.dup.name).click()
    await people.sameAs.click()
    await expect(people.chosen(people.samePick, t.panel.sameFirst)).toHaveCount(1)
    await expect(people.merge).toBeVisible()
    await expect(people.cancelMerge).toBeVisible()
    await people.samePick.selectOption({ label: p.dup.option })
    await people.merge.click()
    await expect(people.row(people.inThread, p.dup.name)).toHaveCount(0)
    await expect(people.row(people.inThread, p.dana.name)).toHaveCount(1)
    await expect(people.panel.getByRole('button', { name: p.dana.name, exact: true })).toBeVisible()
    for (const email of [p.dana.key, p.dup.email]) await expect(people.address(email)).toBeVisible()
    await expect(people.notSame).toHaveCount(1)
    await people.notSame.click()
    for (const name of [p.dana.name, p.dup.name]) await expect(people.row(people.inThread, name)).toHaveCount(1)
  })

  test('16 · + Person adds someone to the thread', covers('people/TC-002', 'F-PPL-01'), async ({ page }) => {
    const p = data.set.people
    const t = data.text.people
    const people = new PeoplePage(page, data)
    await people.open(p.thread.id)
    await people.view(t.list).click()
    await expect(people.view(t.list)).toHaveAttribute('aria-pressed', 'true')
    await expect(people.row(people.inThread, p.dana.name)).toBeVisible()
    const before = await people.count()
    await people.addPerson.click()
    await expect(people.newPerson).toHaveAttribute('placeholder', t.newPersonPlaceholder)
    await people.newPerson.fill(p.pat.entry)
    await people.newPerson.press(key('enter'))
    const row = people.row(people.inThread, p.pat.name)
    for (const text of [p.pat.email, p.pat.org]) await expect(row).toContainText(text)
    await expect.poll(() => people.count()).toBe(before + 1)
    expect(person(p.pat.name)).toMatchObject({ name: p.pat.name, in_thread: true })
  })

  test('17 · A person from an unknown domain: name their organisation', covers('people/TC-008', 'F-PPL-07'), async ({ page }) => {
    const p = data.set.people
    const t = data.text.people
    const people = new PeoplePage(page, data)
    await people.open(p.thread.id)
    await people.view(t.list).click()
    await people.addPerson.click()
    await people.newPerson.fill(p.lena.entry)
    await people.newPerson.press(key('enter'))
    await expect(people.row(people.inThread, p.lena.name)).toContainText(p.lena.domain)
    await people.row(people.inThread, p.lena.name).click()
    await expect(people.detail(p.lena.line)).toBeVisible()
    await expect(people.nameOrg).toHaveAccessibleName(`Name this organisation (${p.lena.domain})`)
    await people.nameOrg.click()
    await expect(people.orgName).toHaveAttribute('placeholder', fill(t.panel.orgPlaceholder, { domain: p.lena.domain }))
    await people.orgName.fill(p.lena.org)
    await people.addOrg.click()
    await expect(people.detail(p.lena.org)).toBeVisible()
    await expect(people.nameOrg).toHaveCount(0)
    expect(cli('org', 'list').find((o) => o.id === p.lena.orgId)).toMatchObject({ account: false, domains: [p.lena.domain] })

    // Teardown: the scenario's thread and the organisation removed.
    expect(data.cli('project', 'delete', p.thread.id).code).toBe(0)
    expect(data.cli('org', 'remove', p.lena.orgId).code).toBe(0)
  })
})
