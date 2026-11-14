// S-28 Echo from the terminal — every stage (standard data): echoctl's help and JSON, the thread lists against the
// window, what read gives behind each echo:// link and where the window opens it, Microsoft 365 sends that send nothing
// without --yes (and are refused with it, not signed in), a series' rule and two settings, clients and an
// organisation, a thread made and shaped and deleted, a thread's work edited as the user, one call tidied (its kind,
// title, action item, minutes page) and its trash, and read inside a run. The read stages come first; each changing
// stage puts back what it changed, but stage 9 (Acme rollout's work), as the scenario says.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal, changed } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CalendarPage } from '../pages/calendar.page.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { ClientsPage } from '../pages/clients.page.mjs'
import { MeetingPage } from '../pages/meeting.page.mjs'
import { PeoplePage } from '../pages/people.page.mjs'
import { SettingsPage } from '../pages/settings.page.mjs'
import { WorkPage } from '../pages/work.page.mjs'

const data = useData(test, { set: 'standard', ai: 'fake' })

/** `echoctl …` that must succeed, its output read as JSON. */
const cli = (...args) => {
  const r = data.cli(...args)
  if (r.code !== 0) throw new Error(`${args.join(' ')}: ${r.err}`)
  return r.json()
}
/** A command's error line (its last line on stderr). */
const err = (r) => r.err.trim().split('\n').at(-1)
/** The newest call folder whose name ends so (`ls … | grep … | tail -1`). */
const lastCall = (slug) => fs.readdirSync(path.join(data.home, 'calls')).filter((n) => n.endsWith(`-${slug}`)).sort().at(-1)
/** The commands a help text lists under "Commands:", in order. */
const commands = (help) => {
  const lines = help.split('\n')
  const from = lines.indexOf('Commands:') + 1
  const to = lines.findIndex((l, i) => i >= from && !l.trim())
  return lines.slice(from, to).map((l) => l.trim().split(/\s+/)[0])
}
/** The CLI's own words in its JSON (states, ops, answers). */
const J = data.text.ledgerJson
/** `echoctl project list` on the data as built (the prelude): stages 1 and 2 read it. */
let built = null
/** The window at an echo:// link, as the app opens one the system hands it. */
const linkRoute = (link) => fill(data.text.links.route, { link: encodeURIComponent(link) })
/** A day as read writes it (YYYY-MM-DD) and a time (HH:MM), of an ISO time, on this computer's clock. */
const ymd = (iso) => { const d = new Date(iso); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}` }
const hhmm = (iso) => { const d = new Date(iso); return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}` }
/** A call's start, from its meta.json. */
const startOf = (name) => data.json(data.home, 'calls', name, 'meta.json').started_at

scenario(test, 'S-28', 'Echo from the terminal: help, threads, links, sends, series and settings, clients, thread changes, the work and one call', () => {
  test('prelude · project list on the data as built', async () => {
    const r = data.cli('project', 'list')
    expect(r.code).toBe(0)
    built = r.json()
  })

  test('1 · echoctl --help lists every command, project --help its own, help <cmd> is --help; read commands print JSON; an unknown one fails', covers('cli/TC-007', 'F-CLI-01'), async ({ page }) => {
    const h = data.set.s28.help
    const help = data.cli('--help')
    expect(help.code).toBe(0)
    expect(help.out.split('\n')[0]).toBe(h.first)
    expect(help.out).toContain(h.usage)
    const all = commands(help.out)
    for (const c of h.has) expect([c, all.includes(c)]).toEqual([c, true])
    expect(all.at(-1)).toBe(h.last)
    const project = data.cli('project', '--help')
    expect(project.code).toBe(0)
    expect(project.out).toContain(h.projectUsage)
    expect(commands(project.out)).toEqual(h.project)
    const edit = data.cli('project', 'ledger', data.set.s28.ledger.thread, 'edit', '--help')
    expect(edit.code).toBe(0)
    for (const line of h.edit) expect(edit.out.split('\n')).toContain(line)
    const helpSeries = data.cli('help', 'series')
    expect(helpSeries.code).toBe(0)
    expect(helpSeries.out).toBe(data.cli('series', '--help').out)
    expect(helpSeries.out).toContain(h.series.usage)
    expect(commands(helpSeries.out)).toEqual(h.series.commands)
    const version = data.cli('--version')
    expect(version.code).toBe(0)
    const v = new RegExp(h.version).exec(version.out.trim())
    expect(v).not.toBeNull()
    const settings = new SettingsPage(page, data)
    await settings.open(data.text.settings.route)
    await expect(settings.says(fill(h.echo, { v: v[1] }))).toBeVisible()
    expect(Object.keys(cli('status'))).toEqual(expect.arrayContaining(h.status))
    expect(Object.keys(built)).toEqual(expect.arrayContaining(h.list))
    const bad = data.cli(h.bad)
    expect(bad.code).toBe(h.badCode)
    for (const line of h.badErr) expect(bad.err.split('\n')).toContain(line)
  })

  test('2 · project list, show, of, people and found agree with the Threads list, the Calls and People tabs and Found for you', covers('cli/TC-008', 'F-CLI-04'), async ({ page }) => {
    const l = data.set.s28.lists
    const t = data.text.threads
    // The read commands first, in one go (nothing changes the data in this stage).
    const A = lastCall(data.set.s28.read.weekly)
    const I = data.callName(l.planning.slug)
    const show = cli('project', 'show', l.rollout.id)
    const ofA = cli('project', 'of', A)
    const ofI = cli('project', 'of', I)
    const people = cli('project', 'people', l.rollout.id)
    const found = cli('project', 'found')

    expect(built.projects.map((p) => p.id).sort()).toEqual(l.ids)
    const byId = Object.fromEntries(built.projects.map((p) => [p.id, p]))
    const { row: rr, ...rollout } = l.rollout
    const { row: lr, ...lead } = l.lead
    expect(byId[rollout.id]).toMatchObject(rollout)
    expect(byId[lead.id]).toMatchObject(lead)
    expect(built.stages.map((s) => s.id ?? s)).toEqual(l.stages)

    const clients = new ClientsPage(page, data)
    await clients.goto(t.routes.projects)
    await expect(clients.foundForYou).toHaveAccessibleName(fill(t.foundCount, { n: l.found }))
    const day = await clients.dayLabel(byId[rollout.id].last_call)
    await expect(clients.rowNamed(clients.rowName({ name: rr[0], calls: rr[1], day, stage: rr[2] }))).toBeVisible()
    await expect(clients.rowNamed(clients.rowName({ name: lr[0], calls: lr[1], stage: lr[2] }))).toBeVisible()

    expect(show.calls.map((c) => [c.title, c.why])).toEqual(l.calls)
    expect(show.rules).toEqual(l.rules)
    await clients.openThread(l.rollout.id, t.tabs.calls)
    await expect(clients.tab(t.tabs.callsTab)).toHaveAccessibleName(fill(t.tabCount, { tab: t.tabs.callsTab, n: l.calls.length }))
    const want = []
    for (const [i, [title, why]] of l.calls.entries()) {
      const { started_at: at, minutes } = show.calls[i]
      const vars = { day: await clients.dayLabel(at), time: await clients.clock(at), why: t.why[why], minutes: t.noMinutes }
      want.push([title, fill(minutes ? t.rowLine : t.rowLineMinutes, vars), why === J.pinned ? t.unpin : t.takeOut])
    }
    await expect.poll(() => clients.callRowsRead()).toEqual(want)

    expect(ofA.projects).toEqual(l.ofA.projects)
    expect(ofA.suggested.map((s) => ({ id: s.id, why: s.why }))).toEqual(l.ofA.suggested)
    expect(ofI.projects).toEqual([])
    expect(ofI.suggested.map((s) => [s.id, s.why])).toEqual(l.ofI.suggested.map((id) => [id, l.ofI.why]))

    const calls = new CallsPage(page, data)
    await calls.goto(data.text.shell.routes.calls)
    await calls.call(l.planning.title).click()
    await expect(calls.says(fill(t.partOf, { thread: l.planning.bar, why: l.ofI.why }))).toBeVisible()
    for (const b of [t.suggestAdd, t.notThis]) await expect(calls.headerButton(b)).toBeVisible()

    expect(people.calls).toBe(l.people.calls)
    expect(people.people).toHaveLength(l.people.all)
    expect(people.people.filter((p) => p.in_thread).map((p) => p.name)).toEqual(l.people.in)
    const pp = new PeoplePage(page, data)
    await pp.open(l.rollout.id)
    await expect(pp.peopleTab).toHaveAccessibleName(fill(data.text.people.tabCount, { n: l.people.in.length }))

    const c = l.candidates
    expect(found.map((x) => x.key)).toEqual(c.keys)
    expect(found.find((x) => x.key === c.initrode.key)).toMatchObject({ new_client: c.initrode.new_client, why: c.initrode.why })
    await clients.goto(t.routes.projects)
    await clients.foundForYou.click()
    await expect(clients.foundCards).toHaveCount(c.names.length)
    expect(await clients.foundNames()).toEqual(c.names)
    expect(found.map((x) => x.name)).toEqual(c.names)
    for (const name of c.names) for (const b of t.foundAnswers) await expect(clients.foundButton(name, b)).toBeVisible()
    await expect(clients.foundSays(c.names[0], c.firstWhy)).toBeVisible()
  })

  test('3 · read gives the text behind each kind of echo:// link, and refuses what isn\'t one', covers('cli/TC-013', 'F-CLI-09'), async () => {
    const r = data.set.s28.read
    const k = data.text.links
    const A = lastCall(r.weekly)
    const read = (link) => data.cli('read', link)
    const at = read(fill(k.callAt, { c: A, t: r.at }))
    expect(at.code).toBe(0)
    const lines = at.out.split('\n')
    expect(lines[0]).toBe(fill(r.callHead, { day: ymd(startOf(A)), time: hhmm(startOf(A)) }))
    const around = lines.indexOf(r.around)
    expect(around).toBeGreaterThan(0)
    const said = lines.slice(around + 1, lines.indexOf(r.minutes))
    for (const [i, start] of r.turns.entries()) expect(said[i].startsWith(start)).toBe(true)
    expect(said.filter(Boolean).at(-1).startsWith(r.lastTurn)).toBe(true)
    const minutes = lines.slice(lines.indexOf(r.minutes) + 1)
    expect(minutes).toContain(r.minutesHead)
    for (const s of r.sections) expect(minutes.some((l) => l.replace(/^#+ /, '') === s)).toBe(true)

    const plain = read(fill(k.call, { c: A }))
    expect(plain.code).toBe(0)
    expect(plain.out).not.toContain(r.around.split(' 00')[0])
    expect(plain.out.split('\n').filter(Boolean)[1]).toBe(r.minutes)

    const th = read(r.thread.link).out.split('\n')
    expect(th[0]).toBe(r.thread.head)
    // The first line under a heading.
    const under = (lines, heading) => lines.slice(lines.indexOf(heading) + 1).find(Boolean)
    for (const [heading, line] of [r.thread.brief, r.thread.people]) expect([heading, under(th, heading)]).toEqual([heading, line])
    const callLines = th.slice(th.indexOf(r.thread.calls) + 1).filter((l) => l.includes('echo://call/'))
    expect(callLines).toHaveLength(r.thread.callLinks)

    const cl = read(r.client.link).out.split('\n')
    expect(cl[0]).toBe(r.client.head)
    const threads = cl.slice(cl.indexOf(r.client.threads) + 1, cl.indexOf(r.client.about)).filter(Boolean)
    expect(threads).toEqual(r.client.lines)

    const pe = read(r.person.link).out.split('\n')
    expect(pe[0]).toBe(r.person.head)
    expect(pe.filter(Boolean)[1]).toBe(r.person.sub)
    // "## Threads" holds the line (among the threads she is in or seen in).
    const section = pe.slice(pe.indexOf(r.person.threads[0]) + 1)
    expect(section.slice(0, section.indexOf('')).includes(r.person.threads[1])).toBe(true)
    expect(pe).toContain(r.person.said)

    const ta = read(r.task.link).out.split('\n')
    expect(ta[0]).toBe(r.task.head)
    for (const line of r.task.lines) expect(ta).toContain(line)
    const where = ta.slice(ta.indexOf(r.task.where) + 1)
    expect(where.some((l) => l.includes(J.create) && l.includes(r.task.quote))).toBe(true)

    const pr = read(r.prep.link).out.split('\n')
    expect(pr[0]).toBe(r.prep.head)
    expect(pr).toContain(r.prep.short)
    expect(pr.some((l) => l.startsWith(r.prep.meeting))).toBe(true)

    const missing = read(r.missing.link)
    expect([missing.code, err(missing)]).toEqual([1, r.missing.err])
    for (const [link, e] of r.foreign) {
      const x = read(link)
      expect([x.code, err(x)]).toEqual([1, e])
    }
  })

  test('4 · An echo:// link opens its place in the window: a call\'s moment, a thread, a client, a person, a task, two preps; a broken one says why', covers('shell/TC-016', 'F-WS-08'), async ({ page }) => {
    const l = data.set.s28.links
    const k = data.text.links
    const A = lastCall(data.set.s28.read.weekly)
    const calls = new CallsPage(page, data)
    await calls.goto(linkRoute(fill(k.callAt, { c: A, t: l.turn.at })))
    await expect.poll(() => calls.hash()).toBe(fill(k.callRoute, { c: A, tab: l.tabs.transcript }))
    await expect(calls.heading(l.turn.title)).toBeVisible()
    await expect(calls.turn(l.turn.name)).toBeInViewport()

    const clients = new ClientsPage(page, data)
    await clients.goto(linkRoute(fill(k.thread, { id: l.thread.id })))
    await expect.poll(() => clients.hash()).toBe(fill(data.text.threads.routes.tab, { id: l.thread.id, tab: l.tabs.brief }))
    await expect(clients.heading(l.thread.name)).toBeVisible()
    await expect(clients.paragraph(l.thread.brief)).toHaveText(l.thread.brief)

    await clients.goto(linkRoute(fill(k.client, { id: l.client.id })))
    await expect.poll(() => clients.hash()).toBe(fill(data.text.clients.routes.client, { id: l.client.id, tab: data.text.clients.tabIds.overview }))
    // About folds (F-CLIENT-02): its head shows; opened, its text.
    await expect(clients.aboutFold).toHaveAttribute('aria-expanded', 'false')
    await clients.aboutFold.click()
    await expect(clients.paragraph(l.client.about)).toBeVisible()
    for (const name of l.client.cards) await expect(clients.overviewCard(name)).toBeVisible()

    const people = new PeoplePage(page, data)
    await people.goto(linkRoute(fill(k.person, { email: data.set.s28.read.person.sub.split(' · ').at(-1) })))
    await expect.poll(() => people.hash()).toBe(fill(data.text.threads.routes.tab, { id: l.person.thread, tab: l.tabs.people }))
    await expect(people.panelTab(people.panel, l.person.tab)).toHaveAttribute('aria-selected', 'true')
    await expect(people.panelButton(l.person.name)).toBeVisible()
    for (const line of l.person.lines) await expect(people.detail(line)).toBeVisible()

    const work = new WorkPage(page, data)
    await work.goto(linkRoute(fill(k.task, { thread: l.task.thread, id: l.task.id })))
    await expect.poll(() => work.hash()).toBe(fill(data.text.threads.routes.tab, { id: l.task.thread, tab: l.tabs.work }))
    await expect(work.panelTab(work.details, data.text.work.details)).toHaveAttribute('aria-selected', 'true')
    for (const text of [l.task.id, l.task.kind]) await expect(work.detail(text)).toBeVisible()
    await expect(work.itemTitle).toHaveText(l.task.title)
    await expect(work.status).toHaveText(l.task.status)

    const meeting = new MeetingPage(page, data)
    await meeting.goto(linkRoute(fill(k.prep, { id: l.upcoming.id })))
    await expect.poll(() => meeting.hash()).toBe(fill(l.upcoming.route, { id: l.upcoming.id }))
    await expect(meeting.header).toHaveText(l.upcoming.subject)
    for (const tab of l.upcoming.tabs) await expect(meeting.tab(tab)).toBeVisible()
    await expect(meeting.section(data.text.prep.inShort)).toBeVisible()
    await expect(meeting.says(l.upcoming.short)).toBeVisible()

    await meeting.goto(linkRoute(fill(k.prep, { id: l.recorded.id })))
    await expect.poll(() => meeting.hash()).toBe(fill(k.callRoute, { c: data.callName(l.recorded.slug), tab: l.tabs.prep }))
    await expect(meeting.header).toHaveText(l.recorded.subject)
    await expect(meeting.prepTab).toHaveAttribute('aria-selected', 'true')

    await meeting.goto(linkRoute(l.missing.link))
    await expect(meeting.noticeSays(l.missing.notice)).toBeVisible()
    await expect(meeting.noticeOk()).toBeVisible()
    await expect.poll(() => meeting.hash()).toBe('')
  })

  test('5 · Microsoft 365 sends send nothing without --yes, show what they would send, and are refused with it when not signed in', covers('cli/TC-016', 'F-M365-05'), async () => {
    const m = data.set.s28.m365
    const send = (...args) => data.cliIn(m.text, 'm365', ...args, '--file', '/dev/stdin')
    const mail = send('mail', 'send', '--to', m.to, '--cc', m.cc, '--subject', m.subject)
    expect(mail.code).toBe(0)
    const j = mail.json()
    expect([j.sent, j.would.to[0].emailAddress.address, j.would.cc[0].emailAddress.address, j.would.subject, j.would.text, j.next]).toEqual([false, m.to, m.cc, m.subject, m.text, m.next])
    const teams = send('teams', 'send', '--to', m.to)
    expect(teams.code).toBe(0)
    expect(teams.json()).toMatchObject({ sent: false, would: { kind: m.kinds.oneOnOne, text: m.text, to: m.to }, next: m.next })
    expect(teams.json().would).toEqual({ kind: m.kinds.oneOnOne, text: m.text, to: m.to })
    const post = send('teams', 'post', m.post.team, m.post.channel, '--subject', m.post.subject)
    expect(post.code).toBe(0)
    expect(post.json()).toMatchObject({ sent: false, would: { ...m.post, text: m.text } })
    const self = send('teams', 'send', '--chat', m.self)
    expect(self.code).toBe(0)
    expect(self.json()).toMatchObject({ sent: false })
    expect(self.json().would).toEqual({ chat: m.self, kind: m.kinds.self, text: m.text })
    const file = path.join(data.home, m.upload.file)
    const up = data.cli('m365', 'drive', 'upload', file, m.upload.to)
    expect(up.code).toBe(0)
    expect(up.json()).toMatchObject({ sent: false, would: { to: m.upload.shown, file, bytes: fs.statSync(file).size } })
    // Not signed in on this data: --yes reaches nobody, it is refused. No Microsoft 365 token in the data's config (the
    // file a sign-in writes), so a change of the fixture can never make this a real send.
    expect(fs.existsSync(path.join(data.env.ECHOCTL_CONFIG_DIR, data.text.m365.token))).toBe(false)
    const yes = send('mail', 'send', '--to', m.to, '--cc', m.cc, '--subject', m.subject, '--yes')
    expect([yes.code, err(yes)]).toEqual([1, m.notSigned])
    const bad = send('mail', 'send', '--to', m.bad, '--subject', 'x')
    expect([bad.code, err(bad)]).toEqual([1, m.badErr])
  })

  test('6 · series and config show and change what the window shows: a series\' rule on its meeting\'s card, two settings', covers('cli/TC-014', 'F-CLI-10'), async ({ page }) => {
    const x = data.set.s28.settings
    const list = cli('series', 'list')
    for (const [key, rule] of Object.entries(x.list)) {
      const got = list[key]
      expect([key, got.record, got.languages ?? null]).toEqual([key, rule.record, rule.languages ?? null])
    }
    const cal = new CalendarPage(page, data)
    const card = async (state) => {
      await cal.open('day')
      await cal.openSeries(x.subject)
      await expect(cal.cardSays(x.subject, x.caption)).toBeVisible()
      await expect(cal.seriesButton(x.subject, state.pressed)).toHaveAttribute('aria-pressed', 'true')
      for (const b of state.not) await expect(cal.seriesButton(x.subject, b)).toHaveAttribute('aria-pressed', 'false')
      await expect(cal.cardChosen(x.subject, x.language, state.language)).toHaveCount(1)
    }
    try {
      await card(x.before)
      const set = cli('series', 'set', x.series, JSON.stringify(x.set))
      expect(set.rule).toMatchObject({ ...x.set, subject: x.subject })
      const of = cli('series', 'list', '--thread', x.thread).series
      expect(of.map((s) => ({ key: s.key, subject: s.subject, record: s.rule.record, languages: s.rule.languages }))).toEqual([{ key: x.series, subject: x.subject, ...x.set }])
      await card(x.after)
      expect(cli('config')).toMatchObject(x.config)
      expect(cli('config', '--scheduled', x.changed.scheduled, '--keep-audio', String(x.changed.keep_audio))).toMatchObject(x.changed)
      const settings = new SettingsPage(page, data)
      await settings.goto(x.recording.route)
      await expect(settings.choice(x.recording.group, x.recording.on)).toHaveAttribute('aria-pressed', 'true')
      await expect(settings.choice(x.recording.group, x.recording.off)).toHaveAttribute('aria-pressed', 'false')
      await settings.goto(x.transcription.route)
      await expect(settings.switch(x.transcription.switch)).toHaveAttribute('aria-checked', 'true')
    } finally {
      // 10 (the teardown): the rule and the settings as they were.
      data.cli('series', 'set', x.series, JSON.stringify(x.back))
      data.cli('config', '--scheduled', x.config.scheduled, '--keep-audio', String(x.config.keep_audio))
    }
    expect(cli('series', 'list')[x.series]).toMatchObject({ record: x.back.record })
    expect(cli('series', 'list')[x.series].languages ?? null).toBeNull()
    expect(cli('config')).toMatchObject({ scheduled: x.config.scheduled, keep_audio: x.config.keep_audio })
  })

  test('7 · client and org commands change what the window shows: a client added and renamed, an organisation linked and unlinked, both removed', covers('cli/TC-015', 'F-CLI-10'), async ({ page }) => {
    const x = data.set.s28.clients
    const c = data.text.clients
    const t = data.text.threads
    const clients = new ClientsPage(page, data)
    const list = cli('client', 'list')
    expect(list.map((k) => k.id)).toEqual(x.six)
    expect(list.find((k) => k.id === x.internal).internal).toBe(true)
    expect(list.find((k) => k.id === x.globex).domains).toEqual(x.globexDomains)
    const orgs = cli('org', 'list')
    for (const k of orgs.filter((o) => x.six.includes(o.id))) expect(k.account).toBeUndefined()
    expect(orgs.filter((o) => !x.six.includes(o.id)).map((o) => [o.id, o.account])).toEqual(x.orgs.map((id) => [id, false]))
    let removed
    try {
      expect(cli('client', 'add', x.add.name, '--domains', x.add.domains)).toEqual(x.add.json)
      expect(cli('client', 'set', x.set.id, '--name', x.set.name, '--domains', x.set.domains)).toMatchObject({ id: x.set.id, name: x.set.name, domains: x.set.list })
      await clients.openClient(x.set.id, c.tabIds.overview)
      const lines = await clients.listLines()
      expect(lines.indexOf(x.set.name)).toBeGreaterThan(lines.indexOf(x.set.after))
      expect(lines.indexOf(x.set.name)).toBeLessThan(lines.indexOf(x.set.before))
      await expect(clients.heading(x.set.name)).toBeVisible()
      for (const text of [c.client, x.set.line, c.noThreads]) await expect(clients.says(text)).toBeVisible()
      expect(cli('org', 'add', x.org.name, '--domains', x.org.domains, '--thread', x.org.thread)).toMatchObject({ id: x.org.id, account: false, domains: [x.org.domains] })
      await clients.openThread(x.org.thread, t.tabs.profile)
      await expect(clients.row(t.rows.orgs)).toContainText(x.org.name)
      cli('org', 'set', x.org.id, '--unlink', x.org.thread)
      await clients.openThread(x.org.thread, t.tabs.profile)
      await expect(clients.row(t.rows.orgs)).not.toContainText(x.org.name)
      const keep = data.cli('client', 'remove', x.keep.id)
      expect([keep.code, err(keep)]).toEqual([1, x.keep.err])
    } finally {
      // 10 (the teardown): what the case added, removed; checked below.
      removed = [data.cli('org', 'remove', x.org.id), data.cli('client', 'remove', x.set.id)]
    }
    expect(removed.map((r) => r.out.trim())).toEqual(x.removed)
    expect(cli('client', 'list').map((k) => k.id)).toEqual(x.six)
    await clients.goto(t.routes.projects)
    await expect(clients.listHeading(t.threads)).toBeVisible()
    await expect(clients.clientGroup(x.set.name)).toHaveCount(0)
  })

  test('8 · project create, rule, set, exclude, include, reset and delete shape a thread for Duff, and the window follows', covers('cli/TC-009', 'F-CLI-05'), async ({ page }) => {
    const x = data.set.s28.thread
    const t = data.text.threads
    const [FU, DISC] = x.calls.map((s) => data.callName(s))
    const COFFEE = data.callName(x.coffee)
    const stamp = (name) => name.slice(0, 17)
    const clients = new ClientsPage(page, data)
    let del
    try {
      expect(cli('project', 'create', x.name, '--client', x.client, '--stage', x.stage)).toMatchObject({ id: x.id, client: x.client, stage: x.stage, group: x.group, call_count: 0, rules: [] })
      const ruled = cli('project', 'rule', x.id, '--people')
      expect(ruled.rules).toEqual([{ kind: J.people }])
      expect(ruled.calls.map((k) => [k.title, k.why]).sort()).toEqual(x.titles.map((title) => [title, J.people]).sort())
      await clients.openThread(x.id, t.tabs.calls)
      const lines = await clients.listLines()
      const day = await clients.dayLabel(cli('project', 'show', x.id).last_call)
      expect(lines[lines.indexOf(x.groupName) + 1]).toBe(clients.rowName({ name: x.row[0], calls: x.row[1], day, stage: x.row[2] }))
      for (const title of x.titles) {
        await expect(clients.callRow(title)).toContainText(t.why.people)
        await expect(clients.callButton(title, t.takeOut)).toBeVisible()
      }
      const set = cli('project', 'set', x.id, '--stage', x.set.stage, '--reason', x.set.reason, '--name', x.set.name)
      expect(set).toMatchObject({ id: x.id, name: x.set.name, stage: x.set.stage })
      expect(set.stages.at(-1)).toMatchObject({ stage: x.set.stage, reason: x.set.reason })
      const ex = cli('project', 'exclude', x.id, DISC)
      expect([ex.exclusions, ex.calls.map((k) => k.title)]).toEqual([[stamp(DISC)], [x.titles[0]]])
      const inc = cli('project', 'include', x.id, COFFEE)
      expect([inc.pins, inc.calls.map((k) => [k.title, k.why]).sort()]).toEqual([[stamp(COFFEE)], [[x.titles[0], J.people], [x.coffeeTitle, J.pinned]].sort()])
      await clients.openThread(x.id, t.tabs.calls)
      await expect(clients.heading(x.set.name)).toBeVisible()
      await expect(clients.factEnding(x.set.name, x.set.tag)).toBeVisible()
      await expect(clients.callButton(x.titles[0], t.takeOut)).toBeVisible()
      await expect(clients.callRow(x.coffeeTitle)).toContainText(t.why.pinned)
      await expect(clients.callButton(x.coffeeTitle, t.unpin)).toBeVisible()
      await expect(clients.paragraph(fill(t.takenOut, { n: 1 }))).toHaveText(fill(t.takenOut, { n: 1 }))
      await expect(clients.callRow(x.titles[1])).toHaveCount(0)
      cli('project', 'reset', x.id, DISC)
      const reset = cli('project', 'reset', x.id, COFFEE)
      expect([reset.exclusions, reset.pins, reset.calls.map((k) => [k.title, k.why]).sort()]).toEqual([[], [], x.titles.map((title) => [title, J.people]).sort()])
      expect(cli('project', 'rule', x.id, '--remove', '1')).toMatchObject({ rules: [], calls: [] })
    } finally {
      // 10: the thread deleted (also when a check failed before); checked below.
      del = data.cli('project', 'delete', x.id)
    }
    expect(del.out.trim()).toBe(x.trashed)
    const gone = data.cli('project', 'show', x.id)
    expect([gone.code, err(gone)]).toEqual([1, x.gone])
    await clients.goto(t.routes.projects)
    await expect(clients.listHeading(t.threads)).toBeVisible()
    await expect(clients.threadRow(x.set.name)).toHaveCount(0)
    await expect(clients.fold(x.groupName)).toHaveCount(0)
  })

  test('9 · project ledger tree and edit change the work as the user: a task made, edited, linked, merged and undone, deleted; a move accepted, a merge rejected', covers('cli/TC-010', 'F-CLI-06'), async ({ page }) => {
    const x = data.set.s28.ledger
    const w = data.text.work
    const E = (...args) => data.cli('project', 'ledger', x.thread, ...args)
    const edit = (op) => E('edit', JSON.stringify(op))
    const tree = () => data.ledger(x.thread, 'tree')
    const node = (id) => tree().nodes[id]

    const t0 = tree()
    expect(Object.keys(t0).sort()).toEqual(x.keys)
    expect(t0.roots).toEqual(x.roots)
    const merge = t0.nodes[x.merge.node].suggestions[0]
    const move = t0.nodes[x.move.node].suggestions[0]
    expect([merge.op, merge.to, merge.status]).toEqual([J.merge, x.merge.to, J.pending])
    expect([move.op, move.to, move.status]).toEqual([J.move, x.move.to, J.pending])

    const made = edit(x.create)
    expect(made.code).toBe(0)
    expect(made.json()).toMatchObject({ type: J.create, id: x.id, by: J.human })
    expect(node(x.id)).toMatchObject({ state: J.open, parent: x.create.parent, internal_owner: x.create.internal_owner })
    const edited = edit({ op: J.edit, node: x.id, set: x.edit })
    expect(edited.json()).toMatchObject({ type: J.human, op: J.edit, node: x.id })
    expect(node(x.id)).toMatchObject({ title: x.edit.title, due: x.edit.due })
    expect(edit({ op: J.acSet, node: x.story, ac: 1, met: true }).code).toBe(0)
    const ac = node(x.story).ac
    expect([ac[0].text ?? ac[0].title ?? x.criteria[0], ac[0].met, ac[0].human, ac[1].met]).toEqual([x.criteria[0], true, true, false])
    expect(edit({ op: J.link, node: x.id, depends_on: x.link }).code).toBe(0)
    expect([node(x.id).depends_on, node(x.link).blocks]).toEqual([[x.link], [x.id]])
    expect(edit({ op: J.accept, target: move.mid }).code).toBe(0)
    expect(edit({ op: J.reject, target: merge.mid }).code).toBe(0)
    const t1 = tree().nodes
    expect([t1[x.move.node].parent, t1[x.move.node].suggestions[0].status, t1[x.merge.node].parent, t1[x.merge.node].suggestions[0].status]).toEqual([x.move.to, J.accept, x.oldParent, J.reject])

    // 7: the Tree.
    const work = new WorkPage(page, data)
    await work.open(x.thread)
    await work.views.waitFor()
    if ((await work.view(w.views.tree).getAttribute('aria-current')) !== 'true') await work.view(w.views.tree).click()
    await expect(work.treeRow(x.story)).toContainText(x.tree.story)
    await expect(work.treeRow(x.story)).toContainText(x.tree.criteria)
    await expect(work.treeRow(x.move.node)).toContainText(x.tree.pinned)
    for (const id of [x.move.node, x.link, x.id]) expect(await work.indent(id)).toBeGreaterThan(await work.indent(x.story))
    await expect(work.treeRow(x.id)).toContainText(x.edit.title)
    // Whose and when — the date with its weekday while it is within a week ("Me · Fri, Nov 20").
    const [who, when] = x.tree.meta.split(' · ')
    await expect(work.treeRow(x.id)).toContainText(new RegExp(`${who} · (\\w{3}, )?${when}`))
    // Under Data migration, not under SSO for the Berlin pilot: the rows between the story and the next story.
    const order = await work.treeOrder()
    const story = order.indexOf(x.story)
    const next = order.findIndex((id, i) => i > story && id.startsWith('S'))
    expect(order.slice(story, next < 0 ? undefined : next)).toEqual(expect.arrayContaining([x.move.node, x.link, x.id]))

    // 8–10: a merge undone, the task deleted.
    const merged = edit({ op: J.merge_op, node: x.id, into: x.link })
    expect(merged.code).toBe(0)
    expect(node(x.id)).toMatchObject({ state: J.merged, merged_into: x.link })
    expect(edit({ op: J.undo, target: merged.json().mid }).code).toBe(0)
    expect(node(x.id)).toMatchObject({ state: J.open, merged_into: null })
    expect(edit({ op: J.delete, node: x.id }).code).toBe(0)
    expect(node(x.id).state).toBe(J.deleted)
    await work.open(x.thread)
    await expect(work.treeRow(x.story)).toBeVisible()
    await expect(work.treeRow(x.id)).toHaveCount(0)

    // 11–12: bad edits write nothing.
    const events = () => data.read(data.home, 'projects', x.thread, w.ledger).trim().split('\n').length
    const before = events()
    const badOp = edit({ op: x.badOpName })
    const badNode = edit({ op: J.edit, node: J.badNode, set: { title: 'x' } })
    expect([[badOp.code, err(badOp)], [badNode.code, err(badNode)]]).toEqual([[1, x.badOp], [1, x.badNode]])
    for (const op of J.badOps) {
      const r = edit({ op, target: x.mid })
      expect([op, r.code, err(r)]).toEqual([op, 1, x.noEvent])
    }
    expect(events()).toBe(before)
  })

  test('10 · A call from the CLI: its context, its action item done and back, its kind and title changed and back, its minutes page; context of the latest', covers('cli/TC-011', 'F-CLI-08'), async ({ page }) => {
    const x = data.set.s28.call
    const C = data.callName(x.slug)
    const S = `${C}${x.suffix}`
    try {
      const ctx = cli('context', C)
      expect(ctx.dir.endsWith(`/calls/${C}`)).toBe(true)
      expect(ctx).toMatchObject({ duration_min: x.minutes, dominant_language: x.language, external: x.external, colleagues: [] })
      expect(ctx.meeting_type.type).toBe(x.type)
      expect(cli('context', path.join(data.home, 'calls', C))).toEqual(ctx)
      expect(cli('actions', C)).toEqual([expect.objectContaining(x.action)])
      expect(data.cli('action', C, x.action.id, J.done).out.trim()).toBe(J.done)
      expect(cli('actions', C)[0].status).toBe(J.done)
      expect(data.cli('action', C, x.action.id, J.todo).out.trim()).toBe(J.todo)
      expect(cli('actions', C)[0].status).toBe(J.todo)
      expect(cli('type', C, x.partner.type)).toEqual(x.partner)
      const bogus = data.cli('type', C, 'bogus')
      expect([bogus.code, err(bogus)]).toEqual([1, x.badType])
      const calls = new CallsPage(page, data)
      await calls.goto(data.text.shell.routes.calls)
      await calls.call(x.title).click()
      await expect(calls.kind).toBeVisible()
      await expect(calls.kindChip(data.text.calls.yours)).toHaveText(new RegExp(`^${literal(fill(data.text.calls.kindChip, { kind: x.partner.label }))}`))

      const renamed = data.cli('title', C, x.signed)
      expect(renamed.out.trim().endsWith(`/calls/${S}`)).toBe(true)
      expect(fs.readdirSync(path.join(data.home, 'calls')).filter((n) => n.includes(`-${x.slug}`))).toEqual([S])
      await calls.goto(data.text.shell.routes.calls)
      await calls.call(x.signed).click()
      await expect(calls.heading(x.signed)).toBeVisible()
      await expect(calls.kindChip(data.text.calls.yours)).toHaveText(new RegExp(`^${literal(fill(data.text.calls.kindChip, { kind: x.partner.label }))}`))
      await expect(calls.threadChip(x.thread)).toBeVisible()
      const html = data.cli('html', S).out.trim()
      expect(html.endsWith(`/calls/${S}/${x.signed}${x.html}`)).toBe(true)
      expect(fs.existsSync(html)).toBe(true)
    } finally {
      // 12 (and 5): its title, its kind and its action item as they were.
      if (fs.existsSync(path.join(data.home, 'calls', S))) data.cli('title', S, x.title)
      data.cli('type', C, x.type)
      data.cli('action', C, x.action.id, J.todo)
    }
    expect(data.callName(x.slug)).toBe(C)
    expect(cli('context', C).meeting_type.type).toBe(x.type)
    const newest = fs.readdirSync(path.join(data.home, 'calls')).filter((n) => /^20/.test(n)).sort().at(-1)
    expect(cli('context').dir.endsWith(`/calls/${newest}`)).toBe(true)
  })

  test('11 · delete moves a call to the trash and restore brings it back, each once; note needs a watcher', covers('cli/TC-012', 'F-CLI-08'), async ({ page }) => {
    const x = data.set.s28.trash
    const C = data.callName(x.slug)
    const trash = path.join(data.home, '.echo', 'trash')
    const D = fs.readdirSync(trash).find((n) => n.endsWith(`-${x.draft}`))
    const calls = new CallsPage(page, data)
    const find = async () => {
      await calls.goto(data.text.shell.routes.calls)
      await calls.search.fill(x.search)
      return calls.call(x.title)
    }
    await expect(await find()).toBeVisible()
    let back
    try {
      const del = data.cli('delete', C)
      expect(del.code).toBe(0)
      const j = JSON.parse(del.out.trim().split('\n')[0])
      expect([j.deleted, j.restore, j.trash.endsWith(`/.echo/trash/${C}`)]).toEqual([C, `echoctl restore ${C}`, true])
      expect(fs.readdirSync(trash)).toEqual(expect.arrayContaining([D, C]))
      await expect(await find()).toHaveCount(0)
      const again = data.cli('delete', C)
      expect([again.code, err(again)]).toEqual([1, fill(x.notCall, { c: C })])
    } finally {
      // 6: the call restored (also when a check failed before); checked below.
      back = data.cli('restore', C)
    }
    expect([back.code, back.out.trim().endsWith(`/calls/${C}`)]).toEqual([0, true])
    await expect(await find()).toBeVisible()
    const twice = data.cli('restore', C)
    expect([twice.code, err(twice)]).toEqual([1, fill(x.notInTrash, { c: C, home: data.home })])
    const before = data.files()
    const note = data.cli('note', x.note)
    expect([note.code, err(note)]).toEqual([1, x.noWatcher])
    expect(changed(before, data.files()).filter((f) => f.includes(x.notes))).toEqual([])
  })

  test('12 · read inside a run reads the calls it may and says why not: Yolo its thread\'s, a minutes run its own call, another profile none', covers('cli/TC-017', 'F-CC-11'), async () => {
    const x = data.set.s28.runs
    const v = data.text.runVars
    const k = data.text.links
    const G = lastCall(x.scoping)
    const A = fs.readdirSync(path.join(data.home, 'calls')).filter((n) => n.endsWith(`-${x.weekly}`)).sort()[0]
    const P = cli('ai', 'status').profiles.find((p) => p.default).id
    const yolo = { [v.headless]: '1', [v.task]: x.task, [v.profile]: P }
    const first = (r) => r.out.split('\n')[0]
    const read = (env, c) => data.cliWith(env, 'read', fill(k.call, { c }))
    const own = data.cli('read', fill(k.call, { c: G }))
    expect(first(own)).toBe(fill(x.head, { day: ymd(startOf(G)) }))
    const r2 = read(yolo, G)
    expect([r2.code, first(r2)]).toEqual([0, first(own)])
    const r3 = read(yolo, A)
    expect([r3.code, err(r3)]).toEqual([1, fill(x.yoloNot, { c: A })])
    const minutesRun = { [v.headless]: '1', [v.call]: path.join(data.home, 'calls', A) }
    const r4 = read(minutesRun, A)
    expect([r4.code, first(r4).startsWith(x.weeklyHead)]).toEqual([0, true])
    const r4b = read(minutesRun, G)
    expect([r4b.code, err(r4b)]).toEqual([1, fill(x.ownOnly, { c: G })])
    const r5 = read({ [v.headless]: '1' }, G)
    expect([r5.code, err(r5)]).toEqual([1, fill(x.ownOnly, { c: G })])
    const r5b = read(yolo, x.missing)
    expect([r5b.code, err(r5b)]).toEqual([1, x.noCall])
    const r6 = read({ ...yolo, [v.profile]: x.badProfile }, G)
    expect([r6.code, err(r6)]).toEqual([1, fill(x.otherProfile, { c: G })])
    const noProfile = { [v.headless]: '1', [v.task]: x.task }
    const r6b = read(noProfile, G)
    expect([r6b.code, err(r6b)]).toEqual([1, x.noProfile])
  })
})
