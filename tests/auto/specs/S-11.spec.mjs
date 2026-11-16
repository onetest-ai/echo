// S-11 Set Echo up your way — its stages with fixed checks (small data).
// Not automated: stage 37 (window/TC-025, the release's announcement strip): it needs the speech models of an
// installed Echo linked into the run's config, and its last step is the user's (Echo's own window and tray).
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { SettingsPage } from '../pages/settings.page.mjs'
import { VocabularyPage } from '../pages/vocabulary.page.mjs'

const data = useData(test, { set: 'small' })

/** A key of the window's shortcuts, by its name in texts.json. */
const key = (name) => data.text.shell.keys[name]
const files = () => data.text.settings.config
/** The data's settings (`.echo/config.json`), as they are now. */
const config = () => data.json(data.home, files().file)
/** The files of a folder of the data (none when it isn't there). */
const ls = (rel) => (data.exists(data.home, rel) ? fs.readdirSync(path.join(data.home, rel)) : [])
/** How many trashed copies of a template there are (`<id>-<digits>.md`). */
const trashed = (id) => ls(files().trash).filter((n) => new RegExp(`^${literal(id)}-\\d+\\.md$`).test(n)).length
/** A template's file, as it is now. */
const template = (id) => data.read(data.home, files().templates, `${id}.md`)
/** Its lines starting so. */
const lines = (text, start) => text.split('\n').filter((l) => l.startsWith(start))
/** `echoctl …` that must succeed. */
const cli = (...args) => {
  const r = data.cli(...args)
  if (r.code !== 0) throw new Error(`${args.join(' ')}: ${r.err}`)
  return r
}

/** Settings → Templates, a template's editor open on it. */
async function openTemplate(page, name) {
  const settings = new SettingsPage(page, data)
  await settings.openTemplate(name)
  return settings
}
/** Its Reset to built-in, back to Echo's own (a stage's teardown). */
async function reset(settings, name) {
  const tp = data.text.settings.templates
  await settings.editorButton(name, tp.reset).click()
  await expect(settings.row(name)).toHaveAccessibleName(new RegExp(`^${literal(name)} ${literal(tp.builtIn)} `))
}
/** A row of the list, as it reads: its name, its tag, its use. */
const rowName = (name, tag, use) => `${name} ${tag} ${use}`
/** A template's use line: its words and its sections. */
const use = (ws, n) => `${ws.join(', ')} · ${fill(data.text.settings.templates.sections, { n })}`
/** A template file's section headings, its match line, its name line. */
const fileLines = (id, kind) => lines(template(id), data.text.settings.templates.lines[kind])
const sectionLine = (s) => fill(data.text.settings.templates.sectionLine, { s })

/** The calls with the Vocabulary panel open from the book beside the list. */
async function vocabulary(page) {
  const voc = new VocabularyPage(page, data)
  // A fresh load at Calls (it opens the newest call): a hash change after the load on Today opens none.
  await voc.load(data.text.shell.routes.calls)
  await voc.book.click()
  await expect(voc.tab).toHaveAttribute('aria-selected', 'true')
  return voc
}
const learn = (...terms) => cli('vocab', 'add', '--call', data.callName(data.set.setup.vocabulary.call), ...terms)

scenario(test, 'S-11', 'Set Echo up your way: appearance, recording, languages, About me, a template, a meeting type, the vocabulary', () => {
  test('1 · Every section of Settings opens with its own heading', covers('settings/TC-001', 'F-SET-01'), async ({ page }) => {
    const s = data.text.settings
    const settings = new SettingsPage(page, data)
    await settings.open(s.route)
    await expect(settings.heading(s.general.heading)).toBeVisible()
    await expect(settings.sectionButtons).toHaveCount(s.sections.length)
    for (const [i, name] of s.sections.entries()) await expect(settings.sectionButtons.nth(i)).toHaveAccessibleName(new RegExp(`^${literal(name)}`))
    await expect(settings.says(s.savedFoot)).toBeVisible()
    for (const name of s.sections.slice(1)) {
      await settings.sectionButtons.filter({ hasText: new RegExp(`^${literal(name)}`) }).click()
      // The section's first heading (Knowledge's reads Recall, a level lower).
      const [heading, level] = s.headings[name] || [name, 2]
      await expect(settings.main.getByRole('heading', { name: heading, exact: true, level })).toBeVisible()
      await expect(settings.main.getByText(s.broken)).toHaveCount(0)
    }
  })

  test('2 · Search settings finds a section by its name or a setting in it, picks those out, and says when nothing matches', covers('settings/TC-002', 'F-SET-02'), async ({ page }) => {
    const s = data.text.settings
    const q = s.search
    const settings = new SettingsPage(page, data)
    await settings.open(s.general.route)
    const all = async () => {
      await expect(settings.sectionButtons).toHaveCount(s.sections.length)
      await expect(settings.sections.getByText(fill(q.nothing, { q: q.none }))).toHaveCount(0)
    }
    await all()
    for (const [i, [typed, sections, shown, hits]] of q.queries.entries()) {
      await settings.search.fill(typed)
      await expect(settings.sectionButtons).toHaveText(sections)
      await expect(settings.heading(shown)).toBeVisible()
      await expect(settings.hits).toHaveText(hits)
      if (i) continue
      await settings.search.fill('')
      await all()
      await expect(settings.heading(s.general.heading)).toBeVisible()
      await expect(settings.hits).toHaveCount(0)
    }
    await settings.search.fill('')
    await settings.search.fill(q.none)
    await expect(settings.sectionButtons).toHaveCount(0)
    await expect(settings.sections.getByText(fill(q.nothing, { q: q.none }), { exact: true })).toBeVisible()
    await settings.search.fill('')
    await all()
  })

  test('3 · Appearance: Dark, Light, kept after a reload, back to System', covers('settings/TC-003', 'F-SET-04', 'F-SET-03'), async ({ page }) => {
    const s = data.text.settings
    const a = s.appearance
    // The system is light: System draws the window light, Dark dark whatever the system says.
    await page.emulateMedia({ colorScheme: 'light' })
    const settings = new SettingsPage(page, data)
    const shows = async (name, look) => {
      await expect(settings.choice(a.group, name)).toHaveAttribute('aria-pressed', 'true')
      await expect(settings.topBar).toHaveCSS('background-color', a.bar[look])
    }
    await settings.open(s.general.route)
    await shows(a.system, 'light')
    await settings.choice(a.group, a.dark).click()
    await shows(a.dark, 'dark')
    await settings.choice(a.group, a.light).click()
    await shows(a.light, 'light')
    // Light stays light when the system turns dark.
    await page.emulateMedia({ colorScheme: 'dark' })
    await shows(a.light, 'light')
    await page.emulateMedia({ colorScheme: 'light' })
    await settings.choice(a.group, a.dark).click()
    await shows(a.dark, 'dark')
    await page.reload()
    await shows(a.dark, 'dark')
    await settings.choice(a.group, a.system).click()
    await shows(a.system, 'light')
    await expect(settings.choice(a.group, a.dark)).toHaveAttribute('aria-pressed', 'false')
    // System follows the system: dark when it turns dark.
    await page.emulateMedia({ colorScheme: 'dark' })
    await shows(a.system, 'dark')
  })

  test('4 · Recording: Calendar meetings set to Manual, saved and kept', covers('settings/TC-004', 'F-SET-06', 'F-SET-03'), async ({ page }) => {
    const r = data.text.settings.recording
    const settings = new SettingsPage(page, data)
    await settings.open(r.route)
    await expect(settings.choice(r.calendar, r.ask)).toHaveAttribute('aria-pressed', 'true')
    await expect(settings.choice(r.calendar, r.manual)).toHaveAttribute('aria-pressed', 'false')
    expect(config().scheduled).toBeNull()
    await settings.choice(r.calendar, r.manual).click()
    await expect(settings.choice(r.calendar, r.manual)).toHaveAttribute('aria-pressed', 'true')
    await expect(settings.choice(r.calendar, r.ask)).toHaveAttribute('aria-pressed', 'false')
    await expect(settings.choice(r.other, r.ask)).toHaveAttribute('aria-pressed', 'true')
    await expect.poll(() => config().scheduled).toBe(r.manualValue)
    await page.reload()
    await expect(settings.choice(r.calendar, r.manual)).toHaveAttribute('aria-pressed', 'true')
  })

  test('5 · Recording: Other Teams calls set to Manual, saved', covers('settings/TC-005', 'F-SET-06', 'F-SET-03'), async ({ page }) => {
    const r = data.text.settings.recording
    const settings = new SettingsPage(page, data)
    await settings.open(r.route)
    await expect(settings.choice(r.other, r.ask)).toHaveAttribute('aria-pressed', 'true')
    await expect(settings.choice(r.other, r.manual)).toHaveAttribute('aria-pressed', 'false')
    await settings.choice(r.other, r.manual).click()
    await expect(settings.choice(r.other, r.manual)).toHaveAttribute('aria-pressed', 'true')
    await expect(settings.choice(r.calendar, r.manual)).toHaveAttribute('aria-pressed', 'true')
    await expect.poll(() => config().unscheduled).toBe(r.manualValue)
  })

  test('6 · Recording: a language added and removed', covers('settings/TC-006', 'F-SET-07', 'F-SET-03'), async ({ page }) => {
    const r = data.text.settings.recording
    const l = data.set.setup.language
    const settings = new SettingsPage(page, data)
    const chips = settings.main.getByRole('button', { name: new RegExp(`^${literal(fill(r.remove, { name: '' }))}`) })
    await settings.open(r.route)
    await expect(chips).toHaveCount(1)
    await expect(chips).toHaveAccessibleName(fill(r.remove, { name: l.base }))
    await expect(settings.main).toContainText(r.neverGuesses)
    await settings.select(r.addLanguage).selectOption({ label: l.name })
    await expect(chips).toHaveCount(2)
    for (const [i, name] of [l.base, l.name].entries()) await expect(chips.nth(i)).toHaveAccessibleName(fill(r.remove, { name }))
    await expect(settings.select(r.addLanguage).getByRole('option', { name: l.name })).toHaveCount(0)
    await expect.poll(() => config().languages).toEqual([l.baseCode, l.code])
    await settings.button(fill(r.remove, { name: l.name })).click()
    await expect(chips).toHaveCount(1)
    await expect(chips).toHaveAccessibleName(fill(r.remove, { name: l.base }))
    await expect.poll(() => config().languages).toEqual([l.baseCode])
  })

  test("7 · Recording: the default language, first in the settings and the Record button's", covers('settings/TC-007', 'F-SET-07'), async ({ page }) => {
    const r = data.text.settings.recording
    const l = data.set.setup.language
    cli('config', '--languages', `${l.baseCode},${l.code}`)
    const settings = new SettingsPage(page, data)
    const chips = settings.main.getByRole('button', { name: new RegExp(`^${literal(fill(r.remove, { name: '' }))}`) })
    const order = async (...names) => { for (const [i, name] of names.entries()) await expect(chips.nth(i)).toHaveAccessibleName(fill(r.remove, { name })) }
    await settings.open(r.route)
    await order(l.base, l.name)
    await expect(settings.main).toContainText(r.neverGuesses)
    await expect(settings.chosen(r.defaultLanguage, l.base)).toHaveCount(1)
    await expect(settings.topBar.getByText(l.short[0], { exact: true })).toBeVisible()
    await expect(settings.select(r.callLanguage).getByRole('option')).toHaveText([l.base, l.name])
    await settings.select(r.defaultLanguage).selectOption({ label: l.name })
    await expect(settings.chosen(r.defaultLanguage, l.name)).toHaveCount(1)
    await order(l.name, l.base)
    await expect.poll(() => config().languages).toEqual([l.code, l.baseCode])
    await settings.goto()
    await expect(settings.topBar.getByText(l.short[1], { exact: true })).toBeVisible()

    // Teardown: English only.
    cli('config', '--languages', l.baseCode)
  })

  test("8 · Recording: Follow Teams' mute off, saved and kept", covers('settings/TC-008', 'F-SET-08', 'F-SET-03'), async ({ page }) => {
    const r = data.text.settings.recording
    const settings = new SettingsPage(page, data)
    await settings.open(r.route)
    await expect(settings.switch(r.mute)).toHaveAttribute('aria-checked', 'true')
    await settings.switch(r.mute).click()
    await expect(settings.switch(r.mute)).toHaveAttribute('aria-checked', 'false')
    await expect.poll(() => config().follow_app_mute).toBe(false)
    await page.reload()
    await expect(settings.switch(r.mute)).toHaveAttribute('aria-checked', 'false')
  })

  test('9 · Recording: Read the Teams window off; Follow mute goes with it', covers('settings/TC-009', 'F-SET-08', 'F-SET-03'), async ({ page }) => {
    const r = data.text.settings.recording
    const settings = new SettingsPage(page, data)
    await settings.open(r.route)
    await expect(settings.switch(r.teams)).toHaveAttribute('aria-checked', 'true')
    for (const words of [...r.teamsLines, r.screens]) await expect(settings.main).toContainText(words)
    await settings.switch(r.teams).click()
    await expect(settings.switch(r.teams)).toHaveAttribute('aria-checked', 'false')
    await expect(settings.switch(r.mute)).toBeDisabled()
    await expect.poll(() => config().capture_ui).toBe(false)
  })

  test('10 · About me: your name saved, your email untouched', covers('settings/TC-021', 'F-SET-14', 'F-SET-03'), async ({ page }) => {
    const m = data.text.settings.me
    const me = data.set.setup.me
    const settings = new SettingsPage(page, data)
    await settings.open(m.route)
    await expect(settings.box(m.name)).toHaveValue(me.name)
    await expect(settings.box(m.email)).toHaveValue(me.email)
    await settings.box(m.name).fill(me.newName)
    await settings.box(m.name).press(key('tab'))
    await expect(settings.box(m.name)).toHaveValue(me.newName)
    await expect.poll(() => config().me).toMatchObject({ name: me.newName, email: me.email })
    await page.reload()
    await expect(settings.box(m.name)).toHaveValue(me.newName)
  })

  test('11 · About me: your profile edited and saved to me.md', covers('settings/TC-022', 'F-SET-14'), async ({ page }) => {
    const m = data.text.settings.me
    const me = data.set.setup.me
    const settings = new SettingsPage(page, data)
    await settings.open(m.route)
    await expect(settings.main.getByRole('heading', { name: m.heading, level: 1 })).toBeVisible()
    for (const line of me.profileLines) await expect(settings.main.getByText(line)).toBeVisible()
    for (const words of m.readers) await expect(settings.main).toContainText(words)
    await settings.button(m.edit).click()
    const box = settings.box(m.box)
    for (const line of me.profileLines) await expect(box).toContainText(line)
    await box.fill(me.profile)
    await expect(box).toHaveValue(me.profile)
    await box.press(key('save'))
    await expect(box).toHaveCount(0)
    await expect(settings.says(me.profile)).toBeVisible()
    await expect(settings.button(m.edit)).toBeVisible()
    await expect.poll(() => data.read(data.home, files().me)).toContain(me.profile)
  })

  test('12 · Templates: three tabs with their counts switch the list', covers('templates/TC-001', 'F-TPL-01'), async ({ page }) => {
    const tp = data.text.settings.templates
    const { tabs } = data.set.setup.templates
    const settings = new SettingsPage(page, data)
    await settings.open(tp.route)
    await expect(settings.heading(tp.heading)).toBeVisible()
    await expect(settings.textStarting(tp.line)).toBeVisible()
    const list = settings.main.getByRole('tablist', { name: tp.tablist })
    await expect(list.getByRole('tab')).toHaveText(Object.values(tabs).map(([n, c]) => `${n} ${c}`))
    await expect(settings.tab(tabs.minutes[0])).toHaveAttribute('aria-selected', 'true')
    await settings.tab(tabs.types[0]).click()
    await expect(settings.tab(tabs.types[0])).toHaveAttribute('aria-selected', 'true')
    await expect(settings.textStarting(tp.typesHead)).toBeVisible()
    await expect(settings.button(tp.newType)).toBeVisible()
    await settings.tab(tabs.skills[0]).click()
    await expect(settings.tab(tabs.skills[0])).toHaveAttribute('aria-selected', 'true')
    await expect(settings.button(tp.newSkill)).toBeVisible()
    await settings.tab(tabs.minutes[0]).click()
    await expect(settings.tab(tabs.minutes[0])).toHaveAttribute('aria-selected', 'true')
    await expect(settings.button(tp.newTemplate)).toBeVisible()
  })

  test('13 · The minutes templates: four built in, each with its use', covers('templates/TC-002', 'F-TPL-02'), async ({ page }) => {
    const tp = data.text.settings.templates
    const settings = new SettingsPage(page, data)
    await settings.open(tp.route)
    await expect(settings.says(tp.list)).toBeVisible()
    const rows = settings.rows()
    await expect(rows).toHaveCount(data.set.setup.templates.rows.length)
    for (const [i, [name, tag, u]] of data.set.setup.templates.rows.entries()) await expect(rows.nth(i)).toHaveAccessibleName(rowName(name, tag, u))
  })

  test("14 · A built-in template's editor: its words, its sections, its foot; Close", covers('templates/TC-003', 'F-TPL-03'), async ({ page }) => {
    const tp = data.text.settings.templates
    const st = data.set.setup.templates.status
    const settings = await openTemplate(page, st.name)
    const ed = settings.editor(st.name)
    await expect(ed.getByText(tp.builtIn, { exact: true })).toBeVisible()
    await expect(settings.editorButton(st.name, tp.reset)).toHaveCount(0)
    await expect(ed.getByText(tp.usedWhen, { exact: true })).toBeVisible()
    const chips = settings.words(st.name)
    await expect(chips).toHaveCount(st.words.length)
    for (const [i, word] of st.words.entries()) await expect(chips.nth(i)).toHaveAccessibleName(fill(tp.removeWord, { word }))
    await expect(settings.headings(st.name)).toHaveCount(st.sections.length)
    for (const [i, h] of st.sections.entries()) await expect(settings.headings(st.name).nth(i)).toHaveValue(h)
    await expect(ed.getByText(tp.foot, { exact: true })).toBeVisible()
    await settings.editorButton(st.name, tp.close).click()
    await expect(ed).toHaveCount(0)
    await expect(settings.says(tp.choose)).toBeVisible()
  })

  test("15 · A built-in template edited is Edited, its file in the data folder", covers('templates/TC-004', 'F-TPL-03', 'F-TPL-04'), async ({ page }) => {
    const tp = data.text.settings.templates
    const st = data.set.setup.templates.status
    const settings = await openTemplate(page, st.name)
    const ed = settings.editor(st.name)
    await expect(ed.getByText(tp.builtIn, { exact: true })).toBeVisible()
    expect(ls(files().templates)).toEqual([])
    await settings.box(tp.summaryBox).fill(st.summary)
    await settings.box(tp.summaryBox).press(key('tab'))
    await expect(settings.box(tp.summaryBox)).toHaveValue(st.summary)
    await expect(settings.row(st.name)).toHaveAccessibleName(new RegExp(`^${literal(st.name)} ${tp.edited} `))
    await expect(ed.getByText(tp.edited, { exact: true })).toBeVisible()
    await expect(settings.editorButton(st.name, tp.reset)).toBeVisible()
    await expect.poll(() => ls(files().templates)).toEqual([`${st.id}.md`])
    expect(template(st.id).split('\n').filter((l) => l.includes(st.summary))).toHaveLength(1)
    await page.reload()
    await settings.row(st.name).click()
    await expect(settings.editor(st.name).getByText(tp.edited, { exact: true })).toBeVisible()
    await expect(settings.box(tp.summaryBox)).toHaveValue(st.summary)
  })

  test("16 · Reset to built-in brings Echo's template back; your edit goes to the trash", covers('templates/TC-005', 'F-TPL-04'), async ({ page }) => {
    const tp = data.text.settings.templates
    const st = data.set.setup.templates.status
    const before = trashed(st.id)
    const settings = await openTemplate(page, st.name)
    const ed = settings.editor(st.name)
    await expect(ed.getByText(tp.edited, { exact: true })).toBeVisible()
    await expect(settings.box(tp.summaryBox)).toHaveValue(st.summary)
    await settings.editorButton(st.name, tp.reset).click()
    await expect(settings.row(st.name)).toHaveAccessibleName(new RegExp(`^${literal(st.name)} ${tp.builtIn} `))
    await expect(settings.editorButton(st.name, tp.reset)).toHaveCount(0)
    await expect(settings.box(tp.summaryBox)).not.toHaveValue(st.summary)
    await expect.poll(() => ls(files().templates)).toEqual([])
    expect(trashed(st.id)).toBe(before + 1)
  })

  test('17 · A word added to a template and removed', covers('templates/TC-006', 'F-TPL-03'), async ({ page }) => {
    const tp = data.text.settings.templates
    const st = data.set.setup.templates.status
    const settings = await openTemplate(page, st.name)
    const ed = settings.editor(st.name)
    const chips = settings.words(st.name)
    await expect(chips).toHaveCount(st.words.length)
    await settings.box(tp.addWord).fill(st.word)
    await settings.box(tp.addWord).press(key('enter'))
    await expect(chips).toHaveCount(st.words.length + 1)
    await expect(chips.last()).toHaveAccessibleName(fill(tp.removeWord, { word: st.word }))
    await expect(settings.row(st.name)).toHaveAccessibleName(rowName(st.name, tp.edited, use([...st.words, st.word], st.sections.length)))
    await expect.poll(() => fileLines(st.id, 'match')).toEqual([fill(tp.match, { words: [...st.words, st.word].join(', ') })])
    await ed.getByRole('button', { name: fill(tp.removeWord, { word: st.word }) }).click()
    await expect(chips).toHaveCount(st.words.length)
    await expect(settings.row(st.name)).toHaveAccessibleName(rowName(st.name, tp.edited, use(st.words, st.sections.length)))
    await expect.poll(() => fileLines(st.id, 'match')).toEqual([fill(tp.match, { words: st.words.join(', ') })])
    await reset(settings, st.name)
  })

  test('18 · A section moved down changes the order, in the page and the file', covers('templates/TC-007', 'F-TPL-03'), async ({ page }) => {
    const tp = data.text.settings.templates
    const st = data.set.setup.templates.status
    const [first, second, ...rest] = st.sections
    const settings = await openTemplate(page, st.name)
    const ed = settings.editor(st.name)
    for (const [i, h] of st.sections.entries()) await expect(settings.headings(st.name).nth(i)).toHaveValue(h)
    await expect(ed.getByRole('button', { name: fill(tp.moveUp, { s: first }) })).toBeDisabled()
    await ed.getByRole('button', { name: fill(tp.moveDown, { s: first }) }).click()
    for (const [i, h] of [second, first, ...rest].entries()) await expect(settings.headings(st.name).nth(i)).toHaveValue(h)
    await expect(settings.row(st.name)).toHaveAccessibleName(new RegExp(`^${literal(st.name)} ${tp.edited} `))
    await expect.poll(() => fileLines(st.id, 'section').slice(0, 2)).toEqual([sectionLine(second), sectionLine(first)])
    await reset(settings, st.name)
  })

  test('19 · A section added and named', covers('templates/TC-008', 'F-TPL-03'), async ({ page }) => {
    const tp = data.text.settings.templates
    const st = data.set.setup.templates.status
    const n = st.sections.length
    const settings = await openTemplate(page, st.name)
    const ed = settings.editor(st.name)
    await expect(settings.headings(st.name)).toHaveCount(n)
    await settings.editorButton(st.name, tp.addSection).click()
    await expect(settings.headings(st.name)).toHaveCount(n + 1)
    const added = settings.headings(st.name).nth(n)
    await expect(added).toHaveValue(tp.newSection)
    await expect(added).toBeFocused()
    await added.fill(st.newSection)
    await added.press(key('tab'))
    await expect(added).toHaveValue(st.newSection)
    await expect.poll(() => fileLines(st.id, 'section')).toHaveLength(n + 1)
    expect(fileLines(st.id, 'section').at(-1)).toBe(sectionLine(st.newSection))
    await page.reload()
    await settings.row(st.name).click()
    await expect(settings.row(st.name)).toHaveAccessibleName(new RegExp(` · ${literal(fill(tp.sections, { n: n + 1 }))}$`))
    await reset(settings, st.name)
  })

  test('20 · A section removed', covers('templates/TC-009', 'F-TPL-03'), async ({ page }) => {
    const tp = data.text.settings.templates
    const st = data.set.setup.templates.status
    const settings = await openTemplate(page, st.name)
    const ed = settings.editor(st.name)
    await expect(settings.headings(st.name)).toHaveCount(st.sections.length)
    await ed.getByRole('button', { name: fill(tp.removeSection, { s: st.removed }) }).click()
    const left = st.sections.filter((s) => s !== st.removed)
    await expect(settings.headings(st.name)).toHaveCount(left.length)
    for (const [i, h] of left.entries()) await expect(settings.headings(st.name).nth(i)).toHaveValue(h)
    await expect(settings.row(st.name)).toHaveAccessibleName(new RegExp(` · ${literal(fill(tp.sections, { n: left.length }))}$`))
    await expect.poll(() => fileLines(st.id, 'section')).toHaveLength(left.length)
    await reset(settings, st.name)
  })

  test("21 · A header value inserted from the values' note", covers('templates/TC-010', 'F-TPL-03'), async ({ page }) => {
    const tp = data.text.settings.templates
    const { status: st, values } = data.set.setup.templates
    const settings = await openTemplate(page, st.name)
    const ed = settings.editor(st.name)
    await expect(ed.getByText(tp.header, { exact: true })).toBeVisible()
    await settings.editorButton(st.name, tp.values).click()
    for (const v of values) await expect(ed.getByRole('button', { name: v, exact: true })).toBeVisible()
    await ed.getByRole('button', { name: st.value, exact: true }).click()
    await expect(settings.box(tp.header)).toHaveValue(new RegExp(`${literal(st.value)}$`))
    await expect(settings.row(st.name)).toHaveAccessibleName(new RegExp(`^${literal(st.name)} ${tp.edited} `))
    await expect.poll(() => template(st.id).split(st.value).length - 1).toBe(1)
    await reset(settings, st.name)
  })

  test('22 · A new template takes its id from its name', covers('templates/TC-011', 'F-TPL-04'), async ({ page }) => {
    const tp = data.text.settings.templates
    const { own, tabs } = data.set.setup.templates
    const settings = new SettingsPage(page, data)
    await settings.open(tp.route)
    await settings.button(tp.newTemplate).click()
    const ed = settings.editor(tp.newTemplate)
    await expect(ed.getByRole('textbox', { name: tp.name, exact: true })).toBeVisible()
    for (const [i, h] of own.sections.entries()) await expect(settings.headings(tp.newTemplate).nth(i)).toHaveValue(h)
    await ed.getByRole('textbox', { name: tp.name, exact: true }).fill(own.name)
    await settings.box(tp.addWord).fill(own.word)
    await settings.box(tp.addWord).press(key('enter'))
    await expect(ed.getByRole('button', { name: fill(tp.removeWord, { word: own.word }) })).toBeVisible()
    await settings.editorButton(tp.newTemplate, tp.create).click()
    await expect(settings.row(own.name)).toHaveAccessibleName(rowName(own.name, tp.yours, use([own.word], own.sections.length)))
    await expect(settings.tab(tabs.minutes[0])).toHaveText(`${tabs.minutes[0]} ${tabs.minutes[1] + 1}`)
    await expect(settings.editorButton(own.name, tp.delete)).toBeVisible()
    await expect.poll(() => [...fileLines(own.id, 'name'), ...fileLines(own.id, 'match')]).toEqual([fill(tp.nameLine, { id: own.id }), fill(tp.match, { words: own.word })])
  })

  test('23 · Deleting your own template moves it to the trash', covers('templates/TC-012', 'F-TPL-04'), async ({ page }) => {
    const tp = data.text.settings.templates
    const { own, tabs } = data.set.setup.templates
    const before = trashed(own.id)
    const settings = await openTemplate(page, own.name)
    await expect(settings.row(own.name)).toHaveAccessibleName(new RegExp(`^${literal(own.name)} ${tp.yours} `))
    await settings.editorButton(own.name, tp.delete).click()
    await expect(settings.row(own.name)).toHaveCount(0)
    await expect(settings.tab(tabs.minutes[0])).toHaveText(`${tabs.minutes[0]} ${tabs.minutes[1]}`)
    await expect(settings.says(tp.choose)).toBeVisible()
    await expect.poll(() => ls(files().templates)).toEqual([])
    expect(trashed(own.id)).toBe(before + 1)
  })

  test("24 · Meeting types: Echo's six, built in, each with how to tell it", covers('templates/TC-013', 'F-TPL-05'), async ({ page }) => {
    const tp = data.text.settings.templates
    const { types, tabs, client, oneOnOne } = data.set.setup.templates
    const settings = new SettingsPage(page, data)
    await settings.open(tp.route)
    await settings.tab(tabs.types[0]).click()
    const rows = settings.rows(tp.builtIn)
    await expect(rows).toHaveCount(types.length)
    for (const [i, name] of types.entries()) await expect(rows.nth(i)).toHaveAccessibleName(new RegExp(`^${literal(name)} ${tp.builtIn} `))
    await expect(settings.row(client.name)).toHaveAccessibleName(rowName(client.name, tp.builtIn, client.about))
    await expect(settings.row(oneOnOne.name)).toHaveAccessibleName(rowName(oneOnOne.name, tp.builtIn, oneOnOne.about))
  })

  test('25 · A new meeting type is yours; deleted, six are left', covers('templates/TC-014', 'F-TPL-05'), async ({ page }) => {
    const tp = data.text.settings.templates
    const { tabs, newType: nt } = data.set.setup.templates
    const types = () => data.json(data.home, files().types)
    const settings = new SettingsPage(page, data)
    await settings.open(tp.route)
    await settings.tab(tabs.types[0]).click()
    await settings.button(tp.newType).click()
    const ed = settings.editor(tp.newTypeEditor)
    await ed.getByRole('textbox', { name: tp.name, exact: true }).fill(nt.name)
    await ed.getByRole('textbox', { name: tp.howToTell, exact: true }).fill(nt.about)
    await settings.editorButton(tp.newTypeEditor, tp.create).click()
    await expect(settings.row(nt.name)).toHaveAccessibleName(new RegExp(`^${literal(nt.name)} ${tp.yours} `))
    await expect(settings.tab(tabs.types[0])).toHaveText(`${tabs.types[0]} ${tabs.types[1] + 1}`)
    await expect.poll(() => types()).toEqual([expect.objectContaining({ id: nt.id, about: nt.about })])
    await settings.editorButton(nt.name, tp.delete).click()
    await expect(settings.row(nt.name)).toHaveCount(0)
    await expect(settings.tab(tabs.types[0])).toHaveText(`${tabs.types[0]} ${tabs.types[1]}`)
    await expect.poll(() => types()).toEqual([])
  })

  test('26 · A built-in meeting type edited is Edited and saved', covers('templates/TC-015', 'F-TPL-05'), async ({ page }) => {
    const tp = data.text.settings.templates
    const { tabs, client } = data.set.setup.templates
    const settings = new SettingsPage(page, data)
    await settings.open(tp.route)
    await settings.tab(tabs.types[0]).click()
    await settings.row(client.name).click()
    const ed = settings.editor(client.name)
    await expect(ed.getByText(tp.builtIn, { exact: true })).toBeVisible()
    await expect(settings.editorButton(client.name, tp.reset)).toHaveCount(0)
    const box = ed.getByRole('textbox', { name: tp.howToTell, exact: true })
    await box.fill(client.edited)
    await box.press(key('tab'))
    await expect(settings.row(client.name)).toHaveAccessibleName(rowName(client.name, tp.edited, client.edited))
    await expect(settings.editorButton(client.name, tp.reset)).toBeVisible()
    await expect.poll(() => data.json(data.home, files().types)).toEqual([expect.objectContaining({ id: client.id, about: client.edited })])
  })

  test("27 · Reset to built-in brings Echo's meeting type back", covers('templates/TC-016', 'F-TPL-05'), async ({ page }) => {
    const tp = data.text.settings.templates
    const { tabs, client } = data.set.setup.templates
    const settings = new SettingsPage(page, data)
    await settings.open(tp.route)
    await settings.tab(tabs.types[0]).click()
    await expect(settings.row(client.name)).toHaveAccessibleName(rowName(client.name, tp.edited, client.edited))
    await settings.row(client.name).click()
    await settings.editorButton(client.name, tp.reset).click()
    await expect(settings.row(client.name)).toHaveAccessibleName(rowName(client.name, tp.builtIn, client.about))
    await expect.poll(() => data.json(data.home, files().types)).toEqual([])
  })

  test('28 · echoctl templates lists what the window shows', covers('templates/TC-017', 'F-TPL-07'), async () => {
    const { cli: c } = data.set.setup.templates
    const t = cli('templates').json()
    expect(Object.keys(t).sort()).toEqual(c.keys)
    expect(t.folder.endsWith(`/${files().templates}`)).toBe(true)
    expect(t.templates.map((x) => [x.id, x.kind])).toEqual(c.templates.map((id) => [id, c.kind]))
    expect(t.meeting_types.map((x) => x.id)).toEqual(c.types)
    expect(t.skills).toEqual([expect.objectContaining({ id: c.skill, kind: c.kind, on: true })])
    expect(t.values.map((x) => x.name ?? x)).toEqual(c.values)
  })

  test("29 · The book beside Calls opens the empty Vocabulary panel; it closes", covers('vocabulary/TC-001', 'F-VOC-01'), async ({ page }) => {
    const v = data.text.vocabulary
    const voc = await vocabulary(page)
    await expect.poll(() => voc.hash()).toMatch(new RegExp(fill(data.text.calls.openOn, { tab: data.text.calls.tabs.minutes })))
    const p = voc.panel
    await expect(p.getByText(v.line, { exact: true })).toBeVisible()
    await expect(p.getByRole('heading', { name: v.every, exact: true })).toBeVisible()
    await expect(p.getByText(v.none, { exact: true })).toBeVisible()
    await expect(p.getByRole('textbox', { name: v.add })).toBeVisible()
    await expect(p.getByRole('button', { name: v.addButton, exact: true })).toBeVisible()
    await expect(voc.learnedHead(0)).toBeVisible()
    await expect(p.getByText(v.learnedNone, { exact: true })).toBeVisible()
    await voc.closePanel(p).click()
    await expect(voc.closePanel(p)).toHaveCount(0)
  })

  test('30 · Terms added to Every call, saved, listed by the CLI, kept after a reload', covers('vocabulary/TC-003', 'F-VOC-02'), async ({ page }) => {
    const v = data.text.vocabulary
    const [a, b] = data.set.setup.vocabulary.terms
    const voc = await vocabulary(page)
    await expect(voc.says(v.none)).toBeVisible()
    await voc.add(a, 'click')
    await expect(voc.chip(a)).toBeVisible()
    await expect(voc.addBox).toHaveValue('')
    await voc.add(b)
    await expect(voc.chip(b)).toBeVisible()
    await expect.poll(() => config().vocabulary).toEqual([a, b])
    const list = cli('vocab', 'list').out
    expect(list).toContain(fill(v.always2, { n: 2, terms: `${a}, ${b}` }))
    expect(list).toContain(fill(v.learnedN, { n: 0 }))
    expect(list).not.toContain(v.moved)
    expect(config().vocabulary).toEqual([a, b])
    await page.reload()
    await voc.book.click()
    for (const t of [a, b]) {
      await expect(voc.chip(t)).toBeVisible()
      await expect(voc.learned(t)).toHaveCount(0)
    }

    // Teardown: both removed.
    for (const t of [a, b]) await voc.chip(t).click()
    await expect(voc.says(v.none)).toBeVisible()
  })

  test('31 · A term removed from Every call', covers('vocabulary/TC-004', 'F-VOC-02'), async ({ page }) => {
    const v = data.text.vocabulary
    const [a] = data.set.setup.vocabulary.terms
    const voc = await vocabulary(page)
    await voc.add(a)
    await expect(voc.chip(a)).toBeVisible()
    await voc.chip(a).click()
    await expect(voc.chip(a)).toHaveCount(0)
    await expect(voc.says(v.none)).toBeVisible()
    await expect.poll(() => config().vocabulary).toEqual([])
  })

  test('32 · The same term twice is one chip', covers('vocabulary/TC-005', 'F-VOC-02'), async ({ page }) => {
    const t = data.set.setup.vocabulary.twice
    const voc = await vocabulary(page)
    await voc.add(t)
    await expect(voc.chip(t)).toHaveCount(1)
    await expect.poll(() => config().vocabulary).toEqual([t])
    await voc.add(t)
    await expect(voc.chip(t)).toHaveCount(1)
    await expect.poll(() => config().vocabulary).toEqual([t])

    // Teardown: removed.
    await voc.chip(t).click()
    await expect.poll(() => config().vocabulary).toEqual([])
  })

  test('33 · Learned terms show their meetings and people', covers('vocabulary/TC-006', 'F-VOC-03'), async ({ page }) => {
    const v = data.text.vocabulary
    const set = data.set.setup.vocabulary
    expect(learn(...set.learned).out).toContain(`${fill(v.learnedFrom, { call: data.callName(set.call) })}${set.learned.join(', ')}`)
    const voc = await vocabulary(page)
    await expect(voc.learnedHead(set.learned.length)).toBeVisible()
    const rows = voc.learnedRows
    await expect(rows).toHaveCount(set.learned.length)
    for (const [i, term] of set.learned.entries()) await expect(rows.nth(i)).toHaveAccessibleName(term)
    const okta = voc.learned(set.learned[1])
    await expect(okta.getByRole('button')).toHaveText([v.always, v.remove])
    await expect(okta.getByText(set.scope, { exact: true })).toBeVisible()
    await expect(voc.says(v.none)).toBeVisible()
  })

  test('34 · Always moves a learned term to Every call', covers('vocabulary/TC-007', 'F-VOC-03'), async ({ page }) => {
    const v = data.text.vocabulary
    const set = data.set.setup.vocabulary
    const [berlin, okta] = set.learned
    const voc = await vocabulary(page)
    await voc.learnedButton(okta, v.always).click()
    await expect(voc.chip(okta)).toBeVisible()
    await expect(voc.learned(okta)).toHaveCount(0)
    await expect(voc.learned(berlin)).toBeVisible()
    await expect.poll(() => config().vocabulary).toContain(okta)

    // Teardown: both terms forgotten.
    cli('vocab', 'remove', ...set.learned)
  })

  test('35 · Remove forgets a learned term', covers('vocabulary/TC-008', 'F-VOC-03'), async ({ page }) => {
    const v = data.text.vocabulary
    const set = data.set.setup.vocabulary
    const [berlin, okta] = set.learned
    learn(...set.learned)
    const voc = await vocabulary(page)
    await voc.learnedButton(berlin, v.remove).click()
    await expect(voc.learned(berlin)).toHaveCount(0)
    await expect(voc.learnedHead(1)).toBeVisible()
    await expect(voc.learned(okta)).toBeVisible()
    const list = cli('vocab', 'list').out
    const learned = list.slice(list.indexOf(fill(v.learnedN, { n: 1 })))
    expect(list).toContain(fill(v.learnedN, { n: 1 }))
    expect(learned).toContain(okta)
    expect(learned).not.toContain(berlin)

    // Teardown: Okta SSO forgotten.
    cli('vocab', 'remove', okta)
  })

  test('36 · The learned terms get a filter past six', covers('vocabulary/TC-009', 'F-VOC-03'), async ({ page }) => {
    const v = data.text.vocabulary
    const set = data.set.setup.vocabulary
    const six = [...set.learned, ...set.more]
    expect(learn(...six).out).toContain(`${fill(v.learnedFrom, { call: data.callName(set.call) })}${six.join(', ')}`)
    const voc = await vocabulary(page)
    const filter = voc.filter
    await expect(voc.learnedHead(six.length)).toBeVisible()
    await expect(filter).toHaveCount(0)
    learn(set.seventh)
    await page.reload()
    await voc.book.click()
    await expect(voc.learnedHead(six.length + 1)).toBeVisible()
    await expect(filter).toBeVisible()
    await filter.fill(set.filter)
    await expect(voc.learnedRows).toHaveCount(1)
    await expect(voc.learned(set.learned[1])).toBeVisible()
    await filter.fill(set.nomatch)
    await expect(voc.says(v.nothing)).toBeVisible()

    // Teardown: the seven forgotten.
    cli('vocab', 'remove', ...six, set.seventh)
  })
})
