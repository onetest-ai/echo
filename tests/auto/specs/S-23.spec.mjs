// S-23 A busy day of calls and minutes in every state — every stage (standard data): the list's icons and its one
// clock chip, the countdown, Not yet (15 minutes more, Until I say, back), the prep tabs, the minutes frames, a failed
// run over minutes and without, a run waiting for a sign-in, the calls Echo leaves alone, one due now, one failed or
// audio only, a memo's and an import's header, the tabs and a call being transcribed, What moved, chat, screens and
// kept live notes; then a memo's threads, Try again against the fake Claude Code, a deleted and a restored call.
// Each test has its own browser: a stage that continues an earlier one's view opens its call again (its rows are
// clicked, the list is never left), so what the scenario keeps in the browser is set again where a stage reads it.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CallsPage } from '../pages/calls.page.mjs'

const data = useData(test, { set: 'standard', ai: 'fake' })

/** The newest call folder whose name ends in a slug (today's Acme weekly sync, not the older ones). */
const folderOf = (slug) => fs.readdirSync(path.join(data.home, 'calls')).filter((n) => n.endsWith(`-${slug}`)).sort().at(-1)
const metaOf = (slug) => data.json(data.home, 'calls', folderOf(slug), data.text.calls.meta)
/** The trash folder's entries ending in a slug. */
const trashed = (slug) => {
  const dir = path.join(data.home, data.text.calls.run.trash)
  return fs.existsSync(dir) ? fs.readdirSync(dir).filter((n) => n.endsWith(`-${slug}`)) : []
}
/** The prep written for a meeting, by its subject (`<home>/prep/<id>/prep.json`). */
const prepOf = (subject) => {
  const dir = path.join(data.home, data.text.prep.folder)
  return fs.readdirSync(dir).map((id) => data.json(dir, id, data.text.prep.file)).find((p) => p.subject === subject)
}
const cs = () => data.text.callStates
const set = () => data.set.s23
/** The model line of a Minutes box, for a profile of the set. */
const modelLine = (p) => fill(cs().modelLine, { profile: p.name, model: p.model })

/** The Calls list on screen (the only navigation to `#calls`): a fresh load opens the newest call. */
async function calls(page) {
  const view = new CallsPage(page, data)
  await view.open()
  await expect(view.newest(set().acme.title)).toBeVisible()
  return view
}

/** The countdown's heading read: its minutes left and its time. */
async function countdown(view) {
  const re = new RegExp(cs().minutesInPattern)
  const h = view.paneHeadingLike(re)
  await expect(h).toBeVisible()
  const [, n, time] = re.exec(await h.textContent())
  return { n: Number(n), time }
}

/** The session line every Minutes box ends with: the words, the command for the call, Copy. */
async function sessionLine(view, slug) {
  await expect(view.saysStarting(cs().session)).toBeVisible()
  await expect(view.says(fill(cs().command, { folder: folderOf(slug) }))).toBeVisible()
  await expect(view.headerButton(cs().copy)).toBeVisible()
}

/** A day and time as the call's header writes them ("Tuesday, November 10, 12:41 PM"). */
async function when(view, slug) {
  const at = metaOf(slug).started_at
  return `${await view.longDay(at)}, ${await view.clock(at)}`
}

scenario(test, 'S-23', 'A busy day of calls and minutes in every state', () => {
  test("1 · Each row's icon names the call's state: minutes, transcript, transcribing, failed, audio, memo", covers('calls/TC-012', 'F-CALLS-02'), async ({ page }) => {
    const view = await calls(page)
    for (const g of set().groups) await expect(view.listGroup(g)).toBeVisible()
    for (const [title, icon, spans] of set().rows) {
      await expect(view.newest(title)).toHaveAccessibleName(new RegExp(`^${literal(cs().icons[icon])} ${literal(title)} .*${spans.map(literal).join(' ')}`))
    }
  })

  test('2 · Only a call with automatic minutes ahead carries the clock chip in the list', covers('calls/TC-013', 'F-MIN-07', 'F-CALLS-02'), async ({ page }) => {
    const view = await calls(page)
    const title = new RegExp(`^${literal(fill(cs().chipTitle, { time: '' }))}`)
    const chip = view.titled(title, view.newest(set().standup.title))
    await expect(chip).toHaveText(new RegExp(cs().chipPattern))
    const n = Number(new RegExp(cs().chipPattern).exec(await chip.textContent())[1])
    expect(n).toBeGreaterThanOrEqual(1)
    expect(n).toBeLessThanOrEqual(set().standup.due)
    const due = new Date(fs.statSync(path.join(data.home, 'calls', folderOf(set().standup.slug), set().standup.notes)).mtimeMs + set().standup.due * 60_000)
    await expect(chip).toHaveAttribute('title', fill(cs().chipTitle, { time: await view.clock(due.toISOString()) }))
    for (const t of set().noChip) await expect(view.titled(title, view.newest(t))).toHaveCount(0)
    await expect(view.titled(title)).toHaveCount(1)
  })

  test('3 · A call whose automatic minutes are ahead shows the time left, a ring, why it moved, Write now and Not yet', covers('calls/TC-027', 'F-MIN-07'), async ({ page }) => {
    const st = set().standup
    const view = await calls(page)
    await view.openNewest(st.title, cs().tabs.minutes)
    const edited = new Date(fs.statSync(path.join(data.home, 'calls', folderOf(st.slug), st.notes)).mtimeMs)
    const { n, time } = await countdown(view)
    expect(n).toBeGreaterThanOrEqual(1)
    expect(n).toBeLessThanOrEqual(st.due)
    expect(time).toBe(await view.clock(new Date(edited.getTime() + st.due * 60_000).toISOString()))
    await expect(view.saysLike(new RegExp(cs().ringPattern))).toHaveText(fill(cs().ring, { n }))
    await expect(view.says(cs().autoLine)).toBeVisible()
    await expect(view.says(fill(cs().movedNotes, { time: await view.clock(edited.toISOString()) }))).toBeVisible()
    await expect(view.says(modelLine(set().profile))).toBeVisible()
    await expect(view.select(cs().modelSelect)).toBeVisible()
    await expect(view.headerButton(cs().writeNow)).toBeVisible()
    await expect(view.headerButton(cs().notYet)).toBeVisible()
    await sessionLine(view, st.slug)
    // The row's chip says the same minutes left (read again if a minute turned between the two reads).
    await expect.poll(async () => {
      const now = await countdown(view)
      return (await view.titled(new RegExp(`^${literal(fill(cs().chipTitle, { time: '' }))}`), view.newest(st.title)).textContent()) === fill(cs().chip, { n: now.n })
    }).toBe(true)
  })

  test('4 · Not yet › 15 minutes more moves the time by 15 minutes and says you said not yet', covers('calls/TC-029', 'F-MIN-07'), async ({ page }) => {
    const st = set().standup
    const view = await calls(page)
    await view.openNewest(st.title, cs().tabs.minutes)
    const before = await countdown(view)
    const t0 = new Date(fs.statSync(path.join(data.home, 'calls', folderOf(st.slug), st.notes)).mtimeMs + st.due * 60_000)
    expect(before.time).toBe(await view.clock(t0.toISOString()))
    await view.headerButton(cs().notYet).click()
    await view.menuItemNamed(cs().notYetItems[0]).click()
    const later = await view.clock(new Date(t0.getTime() + 15 * 60_000).toISOString())
    await expect.poll(async () => (await countdown(view)).time).toBe(later)
    const after = await countdown(view)
    expect(after.n).toBeGreaterThanOrEqual(before.n + 14)
    expect(after.n).toBeLessThanOrEqual(before.n + 15)
    await expect(view.says(cs().movedNotYet)).toBeVisible()
    // Its teardown is stage 5's steps 3–5, which check it is undone.
  })

  test('5 · Not yet › Until I say holds the minutes; Write them automatically again brings the countdown back', covers('calls/TC-028', 'F-MIN-07', 'F-MIN-08'), async ({ page }) => {
    const st = set().standup
    const view = await calls(page)
    await view.openNewest(st.title, cs().tabs.minutes)
    const edited = new Date(fs.statSync(path.join(data.home, 'calls', folderOf(st.slug), st.notes)).mtimeMs)
    const t0 = new Date(edited.getTime() + st.due * 60_000)
    // About 15 minutes more than in stage 3: stage 4's 15 minutes more.
    expect((await countdown(view)).time).toBe(await view.clock(new Date(t0.getTime() + 15 * 60_000).toISOString()))
    await view.headerButton(cs().notYet).click()
    await expect(view.menuItemsShown).toHaveText(cs().notYetItems)
    await view.menuItemNamed(cs().notYetItems[1]).click()
    await expect(view.paneHeading(cs().later)).toBeVisible()
    await expect(view.says(cs().laterLine)).toBeVisible()
    await expect(view.headerButton(cs().autoAgain)).toBeVisible()
    await expect(view.headerButton(cs().writeMinutes)).toBeVisible()
    await expect(view.saysLike(new RegExp(cs().ringPattern))).toHaveCount(0)
    const chip = new RegExp(`^${literal(fill(cs().chipTitle, { time: '' }))}`)
    await expect(view.titled(chip, view.newest(st.title))).toHaveCount(0)
    await view.headerButton(cs().autoAgain).click()
    await expect.poll(async () => (await countdown(view)).time).toBe(await view.clock(t0.toISOString()))
    await expect(view.says(fill(cs().movedNotes, { time: await view.clock(edited.toISOString()) }))).toBeVisible()
    await expect(view.titled(chip, view.newest(st.title))).toHaveText(new RegExp(cs().chipPattern))
  })

  test('6 · A prep whose threads run on different AI profiles says so and shows no In short', covers('prep/TC-011', 'F-PREP-04'), async ({ page }) => {
    const view = await calls(page)
    const p = data.text.prep
    for (const title of set().prepError) {
      await view.openNewest(title)
      await view.tab(cs().tabs.prep).click()
      await expect(view.says(cs().prepNoSummary)).toBeVisible()
      await expect(view.paneHeading(p.inShort)).toHaveCount(0)
      await expect(view.paneHeading(p.expected)).toHaveCount(0)
      await expect(view.says(fill(cs().prepWritten, { time: await view.clock(prepOf(title).written_at) }))).toBeVisible()
      await expect(view.headerButton(p.writeAgain)).toHaveCount(0)
    }
  })

  test('7 · A recorded call whose meeting had a prep has a Prep tab before Minutes; other calls have none', covers('prep/TC-010', 'F-PREP-07', 'F-CALLS-07'), async ({ page }) => {
    const t = cs().tabs
    const view = await calls(page)
    const [first, ...rest] = set().prepFirst
    await view.openNewest(first)
    await expect(view.callTabs).toHaveText([t.prep, t.minutes, t.transcript])
    await view.tab(t.prep).click()
    await expect.poll(() => view.hash()).toMatch(new RegExp(`${literal(cs().routeEnd.prep)}$`))
    await expect(view.prepArticle).toBeVisible()
    for (const title of rest) {
      await view.openNewest(title)
      await expect(view.callTabs).toHaveText([t.prep, t.minutes, t.transcript])
    }
    const a = set().acme
    await view.openNewest(a.title)
    await expect(view.callTabs).toHaveText([t.minutes, t.transcript, fill(cs().tabCount, { tab: t.chat, n: a.chat }), fill(cs().tabCount, { tab: t.screens, n: a.screens })])
    await expect(view.tab(t.minutes)).toHaveAttribute('aria-selected', 'true')
    await expect.poll(() => view.hash()).toMatch(new RegExp(`${literal(cs().routeEnd.minutes)}$`))
  })

  test('8 · A failed run over existing minutes shows a bar above them; a finished run shows none', covers('calls/TC-023', 'F-MIN-09'), async ({ page }) => {
    const view = await calls(page)
    await view.openNewest(set().acme.title, cs().tabs.minutes)
    await expect(view.minutesFrame).toBeVisible()
    for (const w of cs().barWords) await expect(view.saysLike(new RegExp(literal(w)))).toHaveCount(0)
    await view.openNewest(set().kickoff.title)
    await expect(view.minutesFrame).toBeVisible()
    await expect(view.says(fill(cs().lastFailed, { why: cs().exitError }))).toBeVisible()
    await expect(view.headerButton(cs().tryAgain)).toBeVisible()
    await expect(view.headerButton(cs().showLog)).toHaveCount(0)
    await expect(view.headerButton(cs().signIn)).toHaveCount(0)
  })

  test('9 · The minutes frame shows only the sections the call\'s minutes hold, for a call, a Teams import and a memo', covers('calls/TC-022', 'F-MIN-01'), async ({ page }) => {
    const s = set()
    const view = await calls(page)
    await view.openNewest(s.acme.title, cs().tabs.minutes)
    await expect(view.sections).toHaveText(s.acme.sections)
    for (const w of cs().barWords) await expect(view.saysLike(new RegExp(literal(w)))).toHaveCount(0)
    for (const c of [s.steering, s.memo, s.kickoff]) {
      await view.openNewest(c.title)
      await expect(view.sections).toHaveText(c.sections)
    }
  })

  test('10 · A run that wrote nothing says it failed or was skipped, with the reason and the right button', covers('calls/TC-025', 'F-MIN-09'), async ({ page }) => {
    const s = set()
    const view = await calls(page)
    await view.openNewest(s.security.title, cs().tabs.minutes)
    await expect(view.minutesFrame).toHaveCount(0)
    await expect(view.paneHeading(cs().couldnt)).toBeVisible()
    await expect(view.says(cs().exitError)).toBeVisible()
    await expect(view.says(modelLine(s.profile))).toBeVisible()
    await expect(view.select(cs().modelSelect)).toBeVisible()
    await expect(view.headerButton(cs().tryAgain)).toBeVisible()
    await sessionLine(view, 'acme-security-review')
    await view.openNewest(s.walkthrough.title)
    await expect(view.paneHeading(cs().skipped)).toBeVisible()
    await expect(view.says(cs().tooShort)).toBeVisible()
    await expect(view.headerButton(cs().writeMinutes)).toBeVisible()
    await expect(view.headerButton(cs().tryAgain)).toHaveCount(0)
    await expect(view.select(cs().modelSelect)).toBeVisible()
    await sessionLine(view, 'umbrella-proposal-walkthrough')
  })

  test('11 · A run refused for sign-in says to sign in when the profile is signed out, and ready to try again when it is signed in', covers('calls/TC-026', 'F-MIN-09'), async ({ page }) => {
    const s = set()
    const view = await calls(page)
    await view.openNewest(s.vandelay.title, cs().tabs.minutes)
    // The sign-in checks settle in the background a few seconds after the server starts.
    await expect(view.paneHeading(cs().needsSignIn)).toBeVisible({ timeout: 20_000 })
    await expect(view.says(cs().signInFirst)).toBeVisible()
    await expect(view.says(fill(cs().notSignedIn, { profile: s.out.name }))).toBeVisible()
    await expect(view.headerButton(cs().signIn)).toBeVisible()
    await expect(view.chosen(cs().modelSelect, s.out.option)).toHaveCount(1)
    await expect(view.headerButton(cs().tryAgain)).toBeVisible()
    await view.openNewest(s.initrode.title)
    await expect(view.paneHeading(cs().readyAgain)).toBeVisible({ timeout: 20_000 })
    await expect(view.says(cs().lastTry)).toBeVisible()
    await expect(view.says(fill(cs().signedInNow, { profile: s.profile.name, account: s.profile.account }))).toBeVisible()
    await expect(view.headerButton(cs().signInAgain)).toBeVisible()
    await expect(view.headerButton(cs().tryAgain)).toBeVisible()
  })

  test('12 · A call held by Not yet or too thin for automatic minutes says so and still offers Write minutes', covers('calls/TC-030', 'F-MIN-08'), async ({ page }) => {
    const s = set()
    const view = await calls(page)
    await view.openNewest(s.triage.title, cs().tabs.minutes)
    await expect(view.paneHeading(cs().later)).toBeVisible()
    await expect(view.says(cs().laterLine)).toBeVisible()
    await expect(view.headerButton(cs().autoAgain)).toBeVisible()
    await expect(view.headerButton(cs().writeMinutes)).toBeVisible()
    for (const [title, why] of s.thin) {
      await view.openNewest(title)
      await expect(view.paneHeading(cs().notAuto)).toBeVisible()
      await expect(view.says(fill(cs().thin, { why }))).toBeVisible()
      await expect(view.headerButton(cs().writeMinutes)).toBeVisible()
      await expect(view.headerButton(cs().autoAgain)).toHaveCount(0)
    }
  })

  test('13 · A transcribed call Echo leaves alone offers Write minutes, the model, the profile picker and the command', covers('calls/TC-032', 'F-MIN-06', 'F-MIN-12'), async ({ page }) => {
    const s = set()
    const view = await calls(page)
    const [duff, guild] = s.leftAlone
    await view.openNewest(duff, cs().tabs.minutes)
    await expect(view.paneHeading(cs().noMinutes)).toBeVisible()
    await expect(view.says(cs().ready)).toBeVisible()
    await expect(view.says(modelLine(s.profile))).toBeVisible()
    await expect(view.options(cs().modelSelect)).toHaveText(s.profiles)
    await expect(view.chosen(cs().modelSelect, s.profileSelected)).toHaveCount(1)
    const write = view.headerButton(cs().writeMinutes)
    await expect(write).toBeVisible()
    await sessionLine(view, 'duff-discovery')
    // The long Model picker never squeezes the button (#117: it was 26 px and read "mi").
    const fit = await view.buttonBeside(write, view.select(cs().modelSelect))
    expect(fit.width).toBeGreaterThanOrEqual(80)
    expect(fit).toMatchObject({ overlap: false, cut: false })
    await view.openNewest(guild)
    await expect(view.paneHeading(cs().noMinutes)).toBeVisible()
    await expect(view.says(modelLine(s.profile))).toBeVisible()
    await expect(view.headerButton(cs().writeMinutes)).toBeVisible()
  })

  test('14 · A call whose automatic minutes are due now says Echo is starting them and offers no button', covers('calls/TC-031', 'F-MIN-07'), async ({ page }) => {
    const view = await calls(page)
    await view.openNewest(set().due.title, cs().tabs.minutes)
    await expect(view.paneHeading(cs().starting)).toBeVisible()
    await expect(view.says(cs().startingLine)).toBeVisible()
    for (const b of [cs().writeMinutes, cs().writeNow, cs().notYet]) await expect(view.headerButton(b)).toHaveCount(0)
  })

  test('15 · A call whose transcription failed, or that has audio only, says why there is no transcript, not Write minutes', covers('calls/TC-034', 'F-MIN-21', 'F-CALLS-02'), async ({ page }) => {
    const s = set()
    const t = cs().tabs
    const view = await calls(page)
    await view.openNewest(s.intro.title, t.minutes)
    await expect(view.paneHeading(cs().noTranscript)).toBeVisible()
    await expect(view.says(fill(cs().failedLine, { error: s.intro.error }))).toBeVisible()
    await expect(view.saysStarting(cs().again)).toBeVisible()
    await expect(view.says(fill(cs().transcribe, { folder: folderOf(s.intro.slug) }))).toBeVisible()
    await expect(view.headerButton(cs().copy)).toBeVisible()
    await expect(view.headerButton(cs().writeMinutes)).toHaveCount(0)
    await expect(view.says(cs().ready)).toHaveCount(0)
    await view.tab(t.transcript).click()
    await expect(view.says(cs().noTranscriptTab)).toBeVisible()
    await expect(view.turnGroups).toHaveCount(0)
    await expect(view.find).toHaveCount(0)
    await view.openNewest(s.recording.title)
    await expect(view.tab(t.transcript)).toHaveAttribute('aria-selected', 'true')
    await expect(view.says(cs().noTranscriptTab)).toBeVisible()
    await view.tab(t.minutes).click()
    await expect(view.paneHeading(cs().noTranscript)).toBeVisible()
    await expect(view.says(cs().notTranscribed)).toBeVisible()
    await expect(view.saysStarting(cs().again)).toHaveCount(0)
    await expect(view.headerButton(cs().writeMinutes)).toHaveCount(0)
  })

  test("16 · A memo's header says Memo with no kind chip; an imported call's says from Teams transcript", covers('calls/TC-014', 'F-CALLS-05', 'F-CALLS-02'), async ({ page }) => {
    const { memo, steering, acme } = set()
    const view = await calls(page)
    await view.openNewest(memo.title)
    // One line: Memo, its day and time, its length; no kind of meeting, no people.
    expect(await view.factLines()).toEqual([`${cs().memo} ${await when(view, memo.slug)} ${fill(cs().duration, { n: memo.minutes })}`])
    await view.openNewest(steering.title)
    const chip = `${steering.kind} ▾`
    expect(await view.factLines()).toEqual([`${await when(view, steering.slug)} ${fill(cs().duration, { n: steering.minutes })} ${chip}`, cs().kindSelect, `${cs().fromTeams} ${steering.people}`])
    await expect(view.kindChip(cs().invite)).toHaveText(new RegExp(`^${literal(chip)}`))
    await expect(view.factTitled(cs().fromTeamsHint)).toHaveText(cs().fromTeams)
    await view.openNewest(acme.title)
    expect(await view.factLines()).toEqual([`${await when(view, acme.slug)} ${fill(cs().duration, { n: acme.minutes })} ${acme.kind} ▾`, cs().kindSelect, fill(cs().more, { names: acme.people, n: acme.more })])
  })

  test('17 · A call shows Chat and Screens tabs, with counts, only when it has chat or frames', covers('calls/TC-016', 'F-CALLS-07', 'F-CALLS-14', 'F-CALLS-15'), async ({ page }) => {
    const s = set()
    const t = cs().tabs
    const view = await calls(page)
    await view.openNewest(s.acme.title, t.minutes)
    await expect(view.callTabs).toHaveText([t.minutes, t.transcript, fill(cs().tabCount, { tab: t.chat, n: s.acme.chat }), fill(cs().tabCount, { tab: t.screens, n: s.acme.screens })])
    await view.openNewest(s.kickoff.title)
    await expect(view.callTabs).toHaveText([t.minutes, t.transcript, fill(cs().tabCount, { tab: t.chat, n: s.kickoff.chat })])
    for (const title of s.plain) {
      await view.openNewest(title)
      await expect(view.callTabs).toHaveText([t.minutes, t.transcript])
    }
    await view.openNewest(s.quick.title)
    await expect(view.tab(t.transcript)).toHaveAttribute('aria-selected', 'true')
    await expect(view.callTabs).toHaveText([t.minutes, t.transcript])
  })

  test('18 · While a call is being transcribed, the call opens on Transcript and Minutes says they come after the call', covers('calls/TC-033', 'F-MIN-10', 'F-CALLS-06'), async ({ page }) => {
    const t = cs().tabs
    // A full load (this test's own page), the list, then Quick check-in for the first time since the load.
    const view = await calls(page)
    await view.openNewest(set().quick.title)
    await expect(view.tab(t.transcript)).toHaveAttribute('aria-selected', 'true')
    await expect.poll(() => view.hash()).toMatch(new RegExp(`${literal(cs().routeEnd.transcript)}$`))
    await view.tab(t.minutes).click()
    await expect(view.paneHeading(cs().afterCall)).toBeVisible()
    await expect(view.says(cs().afterCallLine)).toBeVisible()
    await expect(view.headerButton(cs().writeMinutes)).toHaveCount(0)
  })

  test('19 · What moved is a right panel view with the count of changes, and the minutes take the whole Minutes tab', covers('calls/TC-039', 'F-MIN-19', 'F-MIN-04', 'F-MIN-05'), async ({ page }) => {
    const { acme, memo, initech, moved } = set()
    const t = cs().tabs
    await page.setViewportSize({ width: 1201, height: 868 })
    const view = await calls(page)
    await view.openNewest(acme.title)
    if (await view.rightPanel.isVisible()) await view.closePanel(view.rightPanel).click()
    await expect(view.tab(t.minutes)).toHaveAttribute('aria-selected', 'true')
    await expect(view.minutesFrame).toBeVisible()
    expect(await view.minutesFill()).toBe(true)
    await view.headerButton(data.text.calls.myNotes).click()
    await expect(view.rightPanel).toBeVisible()
    expect(await view.minutesFill()).toBe(true)
    expect(await view.minutesWidth()).toBeGreaterThanOrEqual(360)
    await view.closePanel(view.rightPanel).click()
    const button = view.headerButton(fill(cs().whatMovedCount, { n: moved.count }))
    await expect(button).toHaveText(String(moved.count))
    await button.click()
    await expect(view.panelTab(view.rightPanel, cs().whatMoved)).toHaveAttribute('aria-selected', 'true')
    await expect(view.panelTab(view.rightPanel, cs().whatMoved)).toHaveText(String(moved.count))
    await expect(view.movedThread(moved.thread)).toContainText(fill(cs().changes, { n: moved.count }))
    await expect(view.movedCards).toHaveCount(moved.count)
    const [name, quote, at] = moved.cards[0]
    const first = view.movedCard(name)
    await expect(first).toContainText(quote)
    await expect(view.inside(first, at)).toBeVisible()
    await view.inside(first, at).click()
    await expect.poll(() => view.hash()).toMatch(new RegExp(`${literal(cs().routeEnd.transcript)}$`))
    const [id, title] = moved.item
    await view.inside(view.movedCard(moved.cards[1][0]), `${id} ${title}`).click()
    await expect(view.panelTab(view.rightPanel, cs().details)).toHaveAttribute('aria-selected', 'true')
    await expect(view.panelHas(id)).toBeVisible()
    await expect(view.panelHas(title)).toBeVisible()
    await view.panelTab(view.rightPanel, cs().whatMoved).click()
    await expect(view.movedCards).toHaveCount(moved.count)
    await view.openNewest(memo.title)
    await expect(view.panelTab(view.rightPanel, cs().whatMoved)).toHaveAttribute('aria-selected', 'true')
    for (const [thread, why] of moved.memoThreads) {
      await expect(view.movedThread(thread)).toContainText(cs()[why])
      await expect(view.inside(view.movedThread(thread), cs().syncNow)).toBeVisible()
    }
    await expect(view.panelTab(view.rightPanel, cs().whatMoved)).toHaveAccessibleName(cs().whatMoved)
    await expect(view.headerButton(cs().whatMoved)).toBeVisible()
    await view.openNewest(initech.title)
    await expect(view.headerButton(cs().whatMoved)).toHaveCount(0)
    await expect(view.panelTab(view.rightPanel, cs().whatMoved)).toHaveCount(0)
    await expect(view.panelTabShown).toHaveCount(1)
  })

  test('20 · What moved lists what the call changed in its thread, hides what the verifier called wrong, and marks the doubtful', covers('calls/TC-020', 'F-MIN-04'), async ({ page }) => {
    const { acme, moved } = set()
    const view = await calls(page)
    await view.openNewest(acme.title)
    await view.headerButton(fill(cs().whatMovedCount, { n: moved.count })).click()
    await expect(view.movedThread(moved.thread)).toContainText(fill(cs().changes, { n: moved.count }))
    await expect(view.panelTab(view.rightPanel, cs().whatMoved)).toHaveText(String(moved.count))
    await expect(view.headerButton(fill(cs().whatMovedCount, { n: moved.count }))).toHaveText(String(moved.count))
    expect(await view.movedCardNames()).toEqual(moved.cards.map((c) => c[0]))
    const marks = moved.cards.map((c) => c[3]).filter(Boolean)
    for (const [name, quote, at, mark] of moved.cards) {
      const card = view.movedCard(name)
      await expect(card).toContainText(quote)
      await expect(view.inside(card, at)).toBeVisible()
      if (mark) await expect(card).toContainText(mark)
      else for (const m of marks) await expect(card).not.toContainText(m)
    }
    await expect(view.panelHas(moved.notShown)).toHaveCount(0)
  })

  test('21 · What moved says why a thread has not read the call yet and offers Sync now', covers('calls/TC-021', 'F-MIN-05'), async ({ page }) => {
    const { memo, kickoff, moved } = set()
    const view = await calls(page)
    await view.openNewest(memo.title)
    await view.headerButton(cs().whatMoved).click()
    await expect(view.panelTab(view.rightPanel, cs().whatMoved)).toHaveAttribute('aria-selected', 'true')
    for (const [thread, why] of moved.memoThreads) {
      await expect(view.movedThread(thread)).toContainText(cs()[why])
      await expect(view.inside(view.movedThread(thread), cs().syncNow)).toBeVisible()
    }
    await expect(view.panelTab(view.rightPanel, cs().whatMoved)).toHaveAccessibleName(cs().whatMoved)
    await expect(view.headerButton(cs().whatMoved)).toHaveAccessibleName(cs().whatMoved)
    await view.openNewest(kickoff.title)
    for (const [thread, why] of moved.kickoffThreads) {
      await expect(view.movedThread(thread)).toContainText(cs()[why])
      await expect(view.inside(view.movedThread(thread), cs().syncNow)).toBeVisible()
    }
    await expect(view.movedThread(moved.kickoffNot)).toHaveCount(0)
  })

  test('22 · The Chat tab lists each message with sender, time and attached file names', covers('calls/TC-017', 'F-CALLS-14'), async ({ page }) => {
    const { acme, kickoff, chat } = set()
    const view = await calls(page)
    await view.openNewest(acme.title, cs().tabs.chat)
    await expect(view.messages).toHaveCount(chat.acme.length)
    for (const [i, m] of chat.acme.entries()) expect(await view.messageLines(i)).toEqual(m)
    await view.openNewest(kickoff.title)
    await expect(view.messages).toHaveCount(chat.kickoff.length)
    expect(await view.messageLines(0)).toEqual(chat.kickoff[0])
  })

  test('23 · The Screens tab shows each frame with its time, and a click opens it large', covers('calls/TC-018', 'F-CALLS-15'), async ({ page }) => {
    const { acme, frames } = set()
    const view = await calls(page)
    await view.openNewest(acme.title, cs().tabs.screens)
    await expect(view.frames).toHaveCount(frames.length)
    for (const t of frames) await expect(view.frame(t)).toBeVisible()
    await expect(view.frameImage(frames[0])).toHaveCount(1)
    await view.frame(frames[0]).click()
    await expect(view.largeFrame).toHaveCount(1)
    await page.keyboard.press(data.text.shell.keys.escape)
    await expect(view.largeFrame).toHaveCount(0)
    await expect(view.frames).toHaveCount(frames.length)
  })

  test('24 · A call that kept live notes shows the transcript alone in its Transcript tab; the live notes are in My notes', covers('calls/TC-019', 'F-CALLS-07'), async ({ page }) => {
    const { acme, initech, live } = set()
    const view = await calls(page)
    await view.openNewest(acme.title, cs().tabs.transcript)
    await expect(view.find).toBeVisible()
    await expect(view.turnGroups).toHaveCount(acme.turns)
    expect(await view.transcriptFills(70)).toBe(true)
    await expect(view.liveRegion(view.main)).toHaveCount(0)
    await expect(view.headerButton(cs().askClaude)).toHaveCount(0)
    await view.headerButton(data.text.calls.myNotes).click()
    const region = view.liveRegion()
    await expect(view.within(region, cs().now)).toBeVisible()
    await expect(view.within(region, live.now)).toBeVisible()
    await expect(region).toContainText(cs().ended)
    for (const item of live.items) await expect(view.noteRow(item)).toContainText(cs().liveMark)
    await expect(view.noteRow(live.items[0])).toContainText(live.owner)
    await expect(view.panelSays(cs().asked)).toBeVisible()
    await expect(view.panelSays(live.asks[0][0])).toBeVisible()
    await expect(view.panelSays(live.asks[0][1])).toBeVisible()
    await view.openNewest(initech.title)
    await expect(view.turnGroups.first()).toBeVisible()
    await expect(view.find).toBeVisible()
    await expect(view.liveRegion()).toHaveCount(0)
    await expect(view.liveRows).toHaveCount(0)
  })

  test('25 · My notes holds the live notes: Now on top, ✦ items among the user\'s notes by time; the Transcript tab is the transcript alone', covers('window/TC-023', 'F-LIVE-03', 'F-LIVE-06', 'F-LIVE-02', 'F-NOTES-01'), async ({ page }) => {
    const { acme, initech, quick, live } = set()
    const t = cs().tabs
    await page.setViewportSize({ width: 1201, height: 868 })
    const view = new CallsPage(page, data)
    await view.goto(fill(data.text.calls.route, { name: folderOf(acme.slug), tab: data.text.calls.tabs.transcript }))
    await expect(view.heading(acme.title)).toBeVisible()
    await expect(view.liveRegion(view.main)).toHaveCount(0)
    await expect(view.find).toBeVisible()
    await expect(view.turnGroups).toHaveCount(acme.turns)
    expect(await view.transcriptFills(70)).toBe(true)
    if (!(await view.rightPanel.isVisible())) await view.headerButton(data.text.calls.myNotes).click()
    const region = view.liveRegion()
    await expect(region).toBeVisible()
    expect((await region.boundingBox()).y).toBeLessThan((await view.notesList.boundingBox()).y)
    await expect(view.within(region, cs().now)).toBeVisible()
    await expect(view.within(region, live.now)).toBeVisible()
    expect(await view.liveLines()).toContain(live.status)
    expect(await view.noteTexts()).toEqual(live.rows)
    const heading = view.within(view.notesList, live.heading)
    expect((await heading.boundingBox()).y).toBeLessThan((await view.noteRows.first().boundingBox()).y)
    await expect(view.panelSays(cs().asked)).toBeVisible()
    for (const [q, a] of live.asks) {
      await expect(view.panelSays(q)).toBeVisible()
      await expect(view.panelSays(a)).toBeVisible()
    }
    // A live row is read-only: no ✎ or ×, and a click opens no editor.
    for (const item of live.items) {
      await expect(view.inside(view.noteRow(item), cs().editNote)).toHaveCount(0)
      await expect(view.inside(view.noteRow(item), cs().deleteNote)).toHaveCount(0)
    }
    await view.panelSays(live.readOnly).click()
    await expect(view.noteEditor).toHaveCount(0)
    // The user's own notes edit as before: ✎ opens the editor with the text, Escape leaves it unchanged.
    const mine = view.noteRow(live.edit)
    await mine.hover()
    await view.inside(mine, cs().editNote).click()
    await expect(view.noteEditor).toHaveValue(live.edit)
    await page.keyboard.press(data.text.shell.keys.escape)
    await expect(view.noteEditor).toHaveCount(0)
    await expect(view.note(live.edit)).toBeVisible()
    // Now's menu: the call's model, the account, Ask Claude; no Pause after the call.
    await view.liveMenuButton.click()
    await expect(view.liveMenuSays(cs().callModel)).toBeVisible()
    await expect(view.liveMenuSelect(cs().callModel)).toBeVisible()
    await expect(view.liveMenuSays(set().profile.name)).toBeVisible()
    await expect(view.liveMenuSays(cs().withAccount)).toBeVisible()
    await expect(view.liveMenuItem(cs().askClaude)).toBeVisible()
    await expect(view.liveMenuItem(cs().pause)).toHaveCount(0)
    await page.keyboard.press(data.text.shell.keys.escape)
    await expect(view.liveMenu).toHaveCount(0)
    // A ✦ row's time chip opens the transcript at its moment, as a user's chip does.
    const [chip, lo, hi] = live.jump
    await view.tab(t.minutes).click()
    await view.within(view.noteRow(live.readOnly), chip).click()
    await expect.poll(() => view.hash()).toMatch(new RegExp(`${literal(cs().routeEnd.transcript)}$`))
    await expect.poll(async () => (await view.centredTurn()).scrolled).toBe(true)
    const centred = await view.centredTurn()
    expect(centred.at).toBeGreaterThanOrEqual(lo)
    expect(centred.at).toBeLessThanOrEqual(hi)
    for (const title of [initech.title, quick.title]) {
      await view.openNewest(title)
      await expect(view.liveRegion()).toHaveCount(0)
      await expect(view.liveRows).toHaveCount(0)
      await expect(view.panelSays(cs().asked)).toHaveCount(0)
    }
    await view.openNewest(initech.title)
    await expect(view.panelSays(data.text.calls.notes.empty)).toBeVisible()
  })

  test("26 · A memo's thread chips show the time they were found in, and × takes one out", covers('calls/TC-015', 'F-CALLS-10', 'F-CALLS-09'), async ({ page }) => {
    const { memo } = set()
    const [[a, aId, aSpan], [g, gId, gSpan]] = memo.chips
    const threads = () => {
      const r = data.cli('project', 'of', folderOf(memo.slug))
      expect(r.code).toBe(0)
      return r.json().projects.map((p) => p.id).sort()
    }
    const view = await calls(page)
    await view.openNewest(memo.title)
    await expect(view.threadChip(a)).toBeVisible()
    await expect(view.says(aSpan)).toBeVisible()
    await expect(view.threadChip(g)).toBeVisible()
    await expect(view.says(gSpan)).toBeVisible()
    await expect(view.threadChip(a)).toHaveAccessibleDescription(cs().foundFrom)
    await expect(view.headerButton(fill(cs().notThread, { thread: a }))).toBeVisible()
    expect(threads()).toEqual([aId, gId].sort())
    await view.headerButton(fill(cs().notThread, { thread: g })).click()
    await expect(view.threadChip(g)).toHaveCount(0)
    await expect(view.says(gSpan)).toHaveCount(0)
    await expect(view.threadChip(a)).toBeVisible()
    await expect(view.says(aSpan)).toBeVisible()
    await expect.poll(threads).toEqual([aId])
    // Its teardown is skipped, as the scenario says: no later stage reads the memo's threads.
  })

  test('27 · Try again ends in a failure that names the fake Claude Code, with minutes or without', covers('calls/TC-024', 'F-MIN-09', 'F-MIN-13'), async ({ page }) => {
    const { kickoff, security } = set()
    const fake = data.text.fake.claudeFails
    const view = await calls(page)
    await view.openNewest(kickoff.title, cs().tabs.minutes)
    await expect(view.says(fill(cs().lastFailed, { why: cs().exitError }))).toBeVisible()
    await view.headerButton(cs().tryAgain).click()
    await expect(view.says(fill(cs().lastFailed, { why: fake }))).toBeVisible({ timeout: 15_000 })
    await expect(view.minutesFrame).toBeVisible()
    await expect(view.headerButton(cs().tryAgain)).toBeVisible()
    await expect(view.headerButton(cs().showLog)).toBeVisible()
    await view.openNewest(security.title)
    await expect(view.paneHeading(cs().couldnt)).toBeVisible()
    await expect(view.says(cs().exitError)).toBeVisible()
    await view.headerButton(cs().tryAgain).click()
    await expect(view.says(fake)).toBeVisible({ timeout: 15_000 })
    await expect(view.paneHeading(cs().couldnt)).toBeVisible()
    await expect(view.headerButton(cs().showLog)).toBeVisible()
    // The Inbox follows the run that ended (#121): its line reads the new message.
    await view.inbox.click()
    const item = view.inboxItem(fill(cs().minutesFailed, { title: security.title }))
    await expect(item).toContainText(fake)
    await expect(view.inside(item, cs().writeAgain)).toBeVisible()
    await expect(view.inside(item, cs().showLog)).toBeVisible()
    await view.inbox.click()
    await expect(view.rightPanel).toBeHidden()
  })

  test('28 · Delete asks first; Keep cancels; Delete takes the call out of the list and into the trash folder', covers('calls/TC-035', 'F-CALLS-12'), async ({ page }) => {
    const { deleted } = set()
    const c = data.text.calls
    const name = folderOf(deleted.slug)
    let gone = false
    try {
      const view = await calls(page)
      await view.openNewest(deleted.title)
      const ask = view.says(fill(c.deleteAsk, { title: deleted.title }))
      await view.callAction(c.deleteItem)
      await expect(ask).toBeVisible()
      await expect(view.headerButton(c.keep)).toBeVisible()
      await expect(view.headerButton(c.delete)).toBeVisible()
      await view.headerButton(c.keep).click()
      await expect(ask).toHaveCount(0)
      await expect(view.heading(deleted.title)).toBeVisible()
      await expect(view.newest(deleted.title)).toBeVisible()
      expect(trashed(deleted.slug)).toEqual([])
      await view.callAction(c.deleteItem)
      await view.headerButton(c.delete).click()
      await expect(view.newest(deleted.title)).toHaveCount(0)
      await expect.poll(() => trashed(deleted.slug)).toEqual([name])
      gone = true
      expect(trashed(deleted.trashed)).toHaveLength(1)
      expect(folderOf(deleted.slug)).toBeUndefined()
    } finally {
      // Teardown: the echoctl restore only (stage 29 reloads the list itself).
      if (trashed(deleted.slug).length) expect(data.cli('restore', trashed(deleted.slug)[0]).code).toBe(0)
    }
    expect(gone).toBe(true)
    expect(folderOf(deleted.slug)).toBe(name)
  })

  test('29 · echoctl restore brings the trashed call back into the list', covers('calls/TC-036', 'F-CALLS-13'), async ({ page }) => {
    const { draft } = set()
    const view = await calls(page)
    await view.search.fill(draft.q)
    await expect(view.listSays(fill(cs().nothingMatches, { q: draft.q }))).toBeVisible()
    const day = new Date(Date.now() - draft.daysBefore * 86_400_000)
    const ymd = `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, '0')}-${String(day.getDate()).padStart(2, '0')}`
    const folder = `${ymd}${draft.at}-${draft.slug}`
    const dir = path.join(data.home, data.text.calls.run.trash)
    expect(fs.readdirSync(dir)).toEqual([folder])
    expect(data.cli('restore', folder).code).toBe(0)
    expect(folderOf(draft.slug)).toBe(folder)
    expect(fs.readdirSync(dir)).not.toContain(folder)
    await page.reload()
    await view.search.fill(draft.q)
    await expect(view.newest(draft.title)).toBeVisible()
    // Its teardown is skipped: the last stage.
  })
})
