// S-12 Skills and AI: skills on and off, AI profiles and the default, plugins and marketplaces — every stage (small
// data). The local model profile points at a port nothing listens on (never a model server); the plugin is a made-up
// one from a marketplace folder in the data's own temporary folder.
import { test, expect } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { SettingsPage } from '../pages/settings.page.mjs'
import { SkillsPage } from '../pages/skills.page.mjs'
import { PluginsPage } from '../pages/plugins.page.mjs'

const data = useData(test, { set: 'small' })

// The prelude: the made-up marketplace, written once beside the data (the plugin stages read it; none writes it again).
const market = () => path.join(data.dir, data.set.s12.market.folder)
test.beforeAll(() => {
  const { market: m, plugin: p } = data.set.s12
  fs.mkdirSync(path.join(market(), p.folder), { recursive: true })
  fs.writeFileSync(path.join(market(), 'echo-marketplace.json'), JSON.stringify(m.manifest))
  fs.writeFileSync(path.join(market(), p.folder, 'echo-plugin.json'), JSON.stringify(p.manifest))
})

/** A JSON file of the data folder or the config folder, or null while it doesn't exist. */
const jsonOr = (...p) => (data.exists(...p) ? data.json(...p) : null)
const config = () => jsonOr(data.home, data.text.settings.config.file)
const aiFile = () => jsonOr(data.env.ECHOCTL_CONFIG_DIR, data.text.ai.file)
/** The names in a folder of the data (none when it doesn't exist). */
const ls = (rel) => (data.exists(data.home, rel) ? fs.readdirSync(path.join(data.home, rel)) : [])
/** `echoctl plugin list`, read. */
const pluginList = () => {
  const r = data.cli('plugin', 'list')
  expect(r.code).toBe(0)
  return r.json()
}
/** One of a plugin's settings as `echoctl plugin set <id> <key>` prints it. */
const pluginSetting = (key) => {
  const r = data.cli('plugin', 'set', data.set.s12.plugin.id, key)
  expect(r.code).toBe(0)
  return r.json().value
}

scenario(test, 'S-12', 'Skills and AI: skills on and off, AI profiles and the default, plugins and marketplaces', () => {
  // ── Skills ──

  test("1 · Skills lists Echo's built-in skill with its switch and description", covers('skills/TC-001', 'F-SKILL-01'), async ({ page }) => {
    const sk = data.set.s12.skill
    const k = data.text.skills
    const skills = new SkillsPage(page, data)
    await skills.openSkills()
    await expect(skills.tab(k.tab)).toHaveAccessibleName(`${k.tab} ${sk.count}`)
    await expect(skills.saysPart(k.listLine)).toBeVisible()
    await expect(skills.skills).toHaveCount(sk.count)
    const row = skills.skill(sk.name)
    await expect(skills.in(row, k.builtIn)).toBeVisible()
    await expect(skills.inStarting(row, k.problems)).toHaveCount(0)
    await expect(skills.inStarting(row, sk.description)).toBeVisible()
    await expect(skills.switchBox(sk.name)).toBeChecked()
    await expect(skills.switchTip(sk.name, k.on)).toBeVisible()
    await expect(skills.button(k.askClaude)).toBeVisible()
    await expect(skills.button(k.newSkill)).toBeVisible()
  })

  test('2 · Opening a skill shows its SKILL.md and how Claude names it', covers('skills/TC-002', 'F-SKILL-02'), async ({ page }) => {
    const sk = data.set.s12.skill
    const k = data.text.skills
    const skills = new SkillsPage(page, data)
    await skills.openSkills()
    await skills.skill(sk.name).click()
    const editor = skills.editor(sk.name)
    await expect(editor).toBeVisible()
    await expect(skills.in(editor, k.builtIn)).toBeVisible()
    await expect(skills.in(editor, fill(k.skillLine, { id: sk.id }))).toBeVisible()
    await expect(skills.inMatching(editor, fill(k.text, { id: sk.id, heading: sk.heading }))).toBeVisible()
    await expect(skills.in(editor, k.toFix)).toHaveCount(0)
    await expect(skills.editorButton(sk.name, k.customize)).toBeVisible()
    for (const name of [k.openCode, k.showFinder, k.reset]) await expect(skills.editorButton(sk.name, name)).toHaveCount(0)
  })

  test('3 · A skill switched off is recorded as off and switched on again', covers('skills/TC-003', 'F-SKILL-01', 'F-SKILL-04'), async ({ page }) => {
    const sk = data.set.s12.skill
    const k = data.text.skills
    const off = () => jsonOr(data.home, k.offFile)
    expect([null, []]).toContainEqual(off())
    const skills = new SkillsPage(page, data)
    await skills.openSkills()
    const on = await skills.nameColour(sk.name)
    try {
      await skills.switchTip(sk.name, k.on).click()
      await expect(skills.switchTip(sk.name, k.off)).toBeVisible()
      await expect(skills.switchBox(sk.name)).not.toBeChecked()
      await expect.poll(() => skills.nameColour(sk.name)).not.toBe(on)
      await expect.poll(off).toEqual([sk.id])
      await skills.switchTip(sk.name, k.off).click()
      await expect(skills.switchTip(sk.name, k.on)).toBeVisible()
      await expect(skills.skillName(sk.name)).toHaveCSS('color', on)
      await expect.poll(off).toEqual([])
    } finally {
      // The skill on again, whatever failed above.
      if (!(await skills.switchBox(sk.name).isChecked())) await skills.switchTip(sk.name, k.off).click()
    }
  })

  test('4 · Customize copies a built-in skill to the user\'s folder', covers('skills/TC-004', 'F-SKILL-03'), async ({ page }) => {
    const sk = data.set.s12.skill
    const k = data.text.skills
    const skills = new SkillsPage(page, data)
    await skills.openSkills()
    await skills.skill(sk.name).click()
    await expect(skills.editorButton(sk.name, k.customize)).toBeVisible()
    await skills.editorButton(sk.name, k.customize).click()
    await expect(skills.in(skills.skill(sk.name), k.customized)).toBeVisible()
    for (const name of [k.openCode, k.showFinder, k.reset]) await expect(skills.editorButton(sk.name, name)).toBeVisible()
    expect(ls(path.join(k.folder, sk.id))).toEqual([k.skillFile])
  })

  test('5 · Reset takes the customized copy away and keeps it in the trash', covers('skills/TC-005', 'F-SKILL-03'), async ({ page }) => {
    const sk = data.set.s12.skill
    const k = data.text.skills
    const trashed = () => ls(k.trash).filter((n) => new RegExp(fill(k.trashed, { id: sk.id })).test(n)).length
    const skills = new SkillsPage(page, data)
    await skills.openSkills()
    await skills.skill(sk.name).click()
    await expect(skills.in(skills.skill(sk.name), k.customized)).toBeVisible()
    await expect(skills.in(skills.editor(sk.name), k.customized)).toBeVisible()
    const n = trashed()
    expect(n).toBe(0)
    await skills.editorButton(sk.name, k.reset).click()
    await expect(skills.in(skills.skill(sk.name), k.builtIn)).toBeVisible()
    await expect(skills.editorButton(sk.name, k.customize)).toBeVisible()
    for (const name of [k.openCode, k.showFinder, k.reset]) await expect(skills.editorButton(sk.name, name)).toHaveCount(0)
    expect(ls(k.folder)).toEqual([])
    expect(trashed()).toBe(n + 1)
  })

  test('6 · New skill refuses a name without latin letters and says why in a toast', covers('skills/TC-006', 'F-SKILL-05', 'F-WS-06'), async ({ page }) => {
    const sk = data.set.s12.skill
    const k = data.text.skills
    const skills = new SkillsPage(page, data)
    await skills.openSkills()
    await skills.button(k.newSkill).click()
    await expect(skills.editor(k.newSkill)).toBeVisible()
    await expect(skills.editorBox(k.newSkill, k.name)).toBeVisible()
    await expect(skills.editorButton(k.newSkill, k.cancel)).toBeVisible()
    await expect(skills.editorButton(k.newSkill, k.create)).toBeVisible()
    await skills.editorBox(k.newSkill, k.name).fill(sk.badName)
    await skills.editorButton(k.newSkill, k.create).click()
    await expect(skills.notice(k.latin)).toBeVisible()
    await expect(skills.noticeOk).toBeVisible()
    expect(ls(k.folder)).toEqual([])
    await skills.noticeOk.click()
    await expect(skills.notice(k.latin)).toHaveCount(0)
    await skills.editorButton(k.newSkill, k.cancel).click()
    await expect(skills.says(k.choose)).toBeVisible()
  })

  // ── AI profiles ──

  test('7 · AI & minutes with only the built-in profile: default, no profile picker anywhere', covers('settings/TC-015', 'F-AI-01', 'F-AI-02', 'F-AI-07', 'F-AI-09'), async ({ page }) => {
    const { system } = data.set.s12.profiles
    const a = data.text.ai
    const settings = new SettingsPage(page, data)
    await settings.open(a.route)
    await expect(settings.cardRows(a.profiles)).toHaveCount(1)
    const row = settings.settingRow(system.name)
    await expect(settings.rowSays(row, a.default)).toBeVisible()
    await expect(settings.rowSays(row, a.builtIn)).toBeVisible()
    await expect(row).toContainText(fill(a.minutes, { model: system.minutes }))
    await expect(row).toContainText(fill(a.live, { model: system.live }))
    await expect(settings.button(a.makeDefault)).toHaveCount(0)
    await expect(settings.account).toHaveText(system.name)
    await expect(settings.topSelects).toHaveCount(1)
    await expect(settings.topSelect(a.language)).toBeVisible()
    await expect(settings.topSelect(a.chip)).toHaveCount(0)
    await expect(settings.account).toHaveAccessibleName(fill(a.chipButton, { name: system.name }))
    const r = data.cli('ai', 'status')
    expect(r.code).toBe(0)
    const listed = r.json().profiles
    expect(listed).toHaveLength(1)
    expect(listed[0]).toMatchObject({ id: system.id, name: system.name, default: true })
  })

  test('8 · The account chip shows the default AI profile; with two it picks the default, with one it opens AI & minutes', covers('settings/TC-027', 'F-AI-09', 'F-AI-01', 'F-WS-07'), async ({ page }) => {
    const { system, local } = data.set.s12.profiles
    const a = data.text.ai
    const sh = data.text.shell
    const settings = new SettingsPage(page, data)
    await settings.goto(sh.routes.today)
    await expect(settings.text(sh.yourDay)).toBeVisible()
    await expect(settings.account).toHaveText(system.name)
    await expect(settings.account).toHaveAccessibleName(fill(a.chipButton, { name: system.name }))
    const line = await settings.topBarLine()
    expect(line.inOrder).toBe(true)
    expect(line.sameLine).toBe(true)
    await expect(settings.topSelect(a.chip)).toHaveCount(0)
    for (const words of a.chipTitle) await expect(settings.account).toHaveAccessibleDescription(new RegExp(literal(words)))
    await settings.account.click()
    await expect.poll(() => settings.hash()).toBe(a.route)
    await expect(settings.heading(a.heading)).toBeVisible()

    const added = data.cli('ai', 'add-local', local.id, '--model', local.model, '--url', local.url, '--name', local.name)
    expect(added.code).toBe(0)
    expect(added.out.trim()).toBe(fill(a.added, { name: local.name, id: local.id }))
    try {
      await settings.goto(sh.routes.today)
      await expect(settings.text(sh.yourDay)).toBeVisible()
      const chip = settings.topSelect(a.chip)
      await expect(settings.chipReads(system.name)).toBeVisible()
      await expect(settings.topOptions(a.chip)).toHaveCount(2)
      await expect(settings.topOption(a.chip, system.name, true)).toHaveCount(1)
      await expect(settings.topOption(a.chip, local.name, false)).toHaveCount(1)
      // The Record pill holds Record, the call's language and Memo: one select, no profile segment.
      await expect(settings.record).toBeVisible()
      await expect(settings.memo).toBeVisible()
      await expect(settings.topSelects).toHaveCount(2)
      await expect(settings.topSelect(a.language)).toBeVisible()
      await chip.selectOption({ label: local.name })
      await expect(settings.chipReads(local.name)).toBeVisible()
      await expect.poll(() => aiFile()?.default).toBe(local.id)
      await page.reload()
      await expect(settings.text(sh.yourDay)).toBeVisible()
      await expect(settings.chipReads(local.name)).toBeVisible()
      await expect(chip).toHaveValue(local.id)
      await settings.goto(a.route)
      await expect(settings.rowSays(settings.settingRow(local.name), a.default)).toBeVisible()
      await expect(settings.rowButton(settings.settingRow(system.name), a.makeDefault)).toBeVisible()
    } finally {
      // Its teardown in S-12: only the default back; Local Llama stays for stages 9–10.
      const back = data.cli('ai', 'default', system.id)
      expect(back.out.trim()).toBe(fill(a.defaultBack, { name: system.name }))
    }
  })

  test('9 · A local model profile added from the CLI shows in AI & minutes and turns the account chip into a picker', covers('settings/TC-016', 'F-AI-01', 'F-AI-02', 'F-AI-07', 'F-AI-09'), async ({ page }) => {
    const { system, local } = data.set.s12.profiles
    const a = data.text.ai
    const settings = new SettingsPage(page, data)
    await settings.open(a.route)
    await expect(settings.cardRows(a.profiles)).toHaveCount(2)
    await expect(settings.rowSays(settings.settingRow(system.name), a.default)).toBeVisible()
    const row = settings.settingRow(local.name)
    await expect(settings.rowSays(row, a.local)).toBeVisible()
    await expect(row).toContainText(fill(a.minutes, { model: local.model }))
    await expect(row).toContainText(fill(a.live, { model: local.model }))
    await expect(settings.rowButton(row, a.makeDefault)).toBeVisible()
    await expect(settings.rowSays(row, a.default)).toHaveCount(0)
    await expect(settings.topSelects).toHaveCount(2)
    await expect(settings.topSelect(a.language)).toBeVisible()
    await expect(settings.topOptions(a.chip)).toHaveCount(2)
    await expect(settings.topOption(a.chip, system.name, true)).toHaveCount(1)
    await expect(settings.topOption(a.chip, local.name, false)).toHaveCount(1)
    const r = data.cli('ai', 'status')
    expect(r.code).toBe(0)
    const listed = r.json().profiles
    expect(listed).toHaveLength(2)
    expect(listed.find((p) => p.id === local.id)).toMatchObject({ kind: local.kind, default: false })
  })

  test('10 · Make default moves "Default for new calls" and the account chip follows', covers('settings/TC-017', 'F-AI-01', 'F-WS-07'), async ({ page }) => {
    const { system, local } = data.set.s12.profiles
    const a = data.text.ai
    const settings = new SettingsPage(page, data)
    await settings.open(a.route)
    try {
      await settings.rowButton(settings.settingRow(local.name), a.makeDefault).click()
      await expect(settings.rowSays(settings.settingRow(local.name), a.default)).toBeVisible()
      await expect(settings.rowButton(settings.settingRow(system.name), a.makeDefault)).toBeVisible()
      await expect(settings.chipReads(local.name)).toBeVisible()
      await expect(settings.topSelect(a.chip)).toHaveValue(local.id)
      await expect.poll(() => aiFile()?.default).toBe(local.id)
    } finally {
      const back = data.cli('ai', 'default', system.id)
      expect(back.out.trim()).toBe(fill(a.defaultBack, { name: system.name }))
      const removed = data.cli('ai', 'remove', local.id)
      expect(removed.out.trim()).toBe(fill(a.removed, { id: local.id }))
    }
  })

  // ── AI & minutes' switches, Transcription, Calendar & chat, Sends to you ──

  test('11 · AI & minutes: Live notes during calls on shows "Live notes on" in the status bar', covers('settings/TC-018', 'F-SET-12', 'F-WS-07'), async ({ page }) => {
    const a = data.text.ai
    const settings = new SettingsPage(page, data)
    await settings.open(a.route)
    await expect(settings.cardRows(a.profiles)).toHaveCount(1)
    await expect(settings.switch(a.liveNotes)).toHaveAttribute('aria-checked', 'false')
    await expect(settings.statusBar).not.toContainText(a.liveOn)
    await settings.switch(a.liveNotes).click()
    await expect(settings.switch(a.liveNotes)).toHaveAttribute('aria-checked', 'true')
    await expect(settings.statusBar).toContainText(a.liveOn)
    await expect.poll(() => aiFile()?.live_notes).toBe(true)
  })

  test('12 · AI & minutes: Write minutes automatically off is saved to config.json', covers('settings/TC-013', 'F-SET-12', 'F-SET-03'), async ({ page }) => {
    const a = data.text.ai
    const settings = new SettingsPage(page, data)
    await settings.open(a.route)
    await expect(settings.switch(a.autoMinutes)).toHaveAttribute('aria-checked', 'true')
    await settings.switch(a.autoMinutes).click()
    await expect(settings.switch(a.autoMinutes)).toHaveAttribute('aria-checked', 'false')
    await expect.poll(() => config().auto_minutes).toBe(false)
    await page.reload()
    await expect(settings.switch(a.autoMinutes)).toHaveAttribute('aria-checked', 'false')
  })

  test('13 · AI & minutes: Claude in the window without permission prompts shows its warning and is off', covers('settings/TC-014', 'F-SET-12'), async ({ page }) => {
    const a = data.text.ai
    const settings = new SettingsPage(page, data)
    await settings.open(a.route)
    const row = settings.settingRow(a.skipRow)
    await expect(row).toBeVisible()
    await expect(settings.rowSays(row, a.skipFlag)).toBeVisible()
    for (const words of a.skipSays) await expect(row).toContainText(words)
    await expect(settings.switch(a.skipRow)).toHaveAttribute('aria-checked', 'false')
    await expect(settings.rowSays(row, a.skipOn)).toHaveCount(0)
    expect(config().folder_skip_permissions).toBe(false)
  })

  test('14 · Transcription lists the speech models with their download size and starts none', covers('settings/TC-010', 'F-SET-09'), async ({ page }) => {
    const { models } = data.set.s12
    const t = data.text.transcription
    const settings = new SettingsPage(page, data)
    await settings.open(t.route)
    await expect(settings.heading(t.heading)).toBeVisible()
    await expect(settings.cardHeading(t.models)).toBeVisible()
    for (const [name, mb] of [models.turbo, ...models.others]) {
      const row = settings.settingRow(name)
      await expect(settings.rowButton(row, fill(t.download, { mb }))).toBeVisible()
      await expect(settings.rowHas(row, new RegExp(`${literal(fill(t.size, { mb }))}$`))).toBeVisible()
      await expect(settings.rowHas(row, new RegExp(`^${literal(t.whisper)}`))).toBeVisible()
    }
    await expect(settings.says(t.recommended)).toHaveCount(1)
    for (const name of models.notInBuild) {
      const row = settings.settingRow(name)
      await expect(settings.rowSays(row, t.notInBuild)).toBeVisible()
      await expect(settings.rowButtons(row)).toHaveCount(0)
    }
    await expect(settings.saysPart(t.downloading)).toHaveCount(0)
    await expect(settings.progress).toHaveCount(0)
  })

  test('15 · Transcription: Keep audio files on is saved to config.json', covers('settings/TC-011', 'F-SET-11', 'F-SET-03'), async ({ page }) => {
    const t = data.text.transcription
    const settings = new SettingsPage(page, data)
    await settings.open(t.route)
    await expect(settings.switch(t.keepAudio)).toHaveAttribute('aria-checked', 'false')
    await settings.switch(t.keepAudio).click()
    await expect(settings.switch(t.keepAudio)).toHaveAttribute('aria-checked', 'true')
    await expect.poll(() => config().keep_audio).toBe(true)
  })

  test('16 · Transcription: Echo cancellation off is saved to config.json', covers('settings/TC-012', 'F-SET-11', 'F-SET-03'), async ({ page }) => {
    const t = data.text.transcription
    const settings = new SettingsPage(page, data)
    await settings.open(t.route)
    await expect(settings.switch(t.echoCancel)).toHaveAttribute('aria-checked', 'true')
    await settings.switch(t.echoCancel).click()
    await expect(settings.switch(t.echoCancel)).toHaveAttribute('aria-checked', 'false')
    await expect.poll(() => config().echo_cancel).toBe(false)
  })

  test('17 · Calendar & chat says Not connected and offers Sign in…', covers('settings/TC-019', 'F-SET-13'), async ({ page }) => {
    const c = data.text.calendar
    const settings = new SettingsPage(page, data)
    await settings.open(c.route)
    await expect(settings.heading(c.heading)).toBeVisible()
    await expect(settings.cardHeading(c.card)).toBeVisible()
    const account = settings.settingRow(c.account)
    await expect(settings.rowSays(account, c.notConnected)).toBeVisible()
    await expect(settings.rowButton(account, c.signIn)).toBeVisible()
    for (const name of c.notThere) await expect(settings.button(name)).toHaveCount(0)
    const reg = settings.settingRow(c.registration)
    await expect(settings.rowSays(reg, c.registrationSays)).toBeVisible()
    await expect(settings.rowButton(reg, c.change)).toBeVisible()
    await expect(settings.rowSwitch(settings.settingRow(c.chat), c.chat)).toBeVisible()
  })

  test('18 · Sends to you shows Teams and Mail waiting for Microsoft 365', covers('settings/TC-020', 'F-SET-15'), async ({ page }) => {
    const d = data.text.deliver
    const settings = new SettingsPage(page, data)
    await settings.open(d.route)
    await expect(settings.heading(d.heading)).toBeVisible()
    for (const [name, says] of d.rows) await expect(settings.rowSays(settings.settingRow(name), says)).toBeVisible()
    await expect(settings.button(d.test)).toHaveCount(0)
  })

  // ── Plugins and marketplaces ──

  test('19 · With nothing installed Plugins opens on Discover and Installed says so', covers('plugins/TC-001', 'F-PLUG-01'), async ({ page }) => {
    const t = data.text.plugins
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins()
    await expect(plugins.pluginTab(t.installed)).toHaveAccessibleName(t.installed)
    await expect(plugins.pluginTab(t.discover)).toHaveAccessibleName(t.discover)
    await expect(plugins.pluginTab(t.markets)).toHaveAccessibleName(`${t.markets}1`)
    await expect(plugins.pluginTab(t.discover)).toHaveAttribute('aria-selected', 'true')
    await plugins.pluginTab(t.installed).click()
    await expect(plugins.pluginTab(t.installed)).toHaveAttribute('aria-selected', 'true')
    await expect(plugins.says(t.none)).toBeVisible()
    await expect(plugins.pluginTab(t.installed)).toHaveAccessibleName(t.installed)
  })

  test('20 · Marketplaces lists the built-in marketplace, not removable, with Read again and the Add form', covers('plugins/TC-009', 'F-PLUG-07'), async ({ page }) => {
    const t = data.text.plugins
    const { builtIn } = data.set.s12.market
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins(t.markets)
    await expect(plugins.pluginTab(t.markets)).toHaveAccessibleName(`${t.markets}1`)
    await expect(plugins.saysPart(t.marketsLine)).toBeVisible()
    const row = plugins.market(builtIn.name)
    await expect(plugins.in(row, t.builtIn)).toBeVisible()
    await expect(plugins.in(row, builtIn.source)).toBeVisible()
    await expect(plugins.buttonIn(row, t.readAgain)).toBeVisible()
    await plugins.buttonIn(row, t.more).click()
    await expect(plugins.menuItems).toHaveCount(1)
    await expect(plugins.menuItem(t.hide)).toBeVisible()
    await expect(plugins.menuItem(t.remove)).toHaveCount(0)
    await plugins.headingPlugins.click()
    await expect(plugins.menu).toHaveCount(0)
    await expect(plugins.says(t.add)).toBeVisible()
    await expect(plugins.source).toHaveAttribute('placeholder', t.sourceHint)
    await expect(plugins.marketName).toHaveAttribute('placeholder', t.nameHint)
    await expect(plugins.addMarket).toBeVisible()
  })

  test('21 · echoctl plugin list gives installed, offered and marketplaces', covers('plugins/TC-013', 'F-PLUG-10'), async () => {
    const t = data.text.plugins
    const { builtIn } = data.set.s12.market
    const list = pluginList()
    expect(Object.keys(list).sort()).toEqual(t.cliKeys)
    expect(list.installed).toEqual([])
    expect(list.markets).toHaveLength(1)
    expect(list.markets[0]).toMatchObject({ name: builtIn.name, source: builtIn.source, built_in: true, hidden: false })
  })

  test('22 · Add a marketplace from a folder lists it with its plugin count', covers('plugins/TC-010', 'F-PLUG-07'), async ({ page }) => {
    const t = data.text.plugins
    const { market: m } = data.set.s12
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins(t.markets)
    await plugins.source.fill(market())
    await plugins.marketName.fill(m.name)
    await plugins.addMarket.click()
    await expect(plugins.pluginTab(t.markets)).toHaveAccessibleName(`${t.markets}2`)
    const row = plugins.market(m.name)
    await expect(plugins.in(row, fs.realpathSync(market()))).toBeVisible()
    await expect(plugins.in(row, fill(t.read, { n: 1 }))).toBeVisible()
    const markets = pluginList().markets
    expect(markets.map((x) => x.name)).toEqual([m.builtIn.name, m.name])
    expect(markets.find((x) => x.name === m.name)).toMatchObject({ count: 1, built_in: false })
    await plugins.buttonIn(row, t.readAgain).click()
    await expect(plugins.in(row, fill(t.read, { n: 1 }))).toBeVisible()
  })

  test('23 · Discover filters by category chip and search, each card naming its marketplace', covers('plugins/TC-002', 'F-PLUG-02'), async ({ page }) => {
    const t = data.text.plugins
    const { market: m, plugin: p } = data.set.s12
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins(t.discover)
    await expect(plugins.chips.first()).toHaveAccessibleName(t.all)
    await expect(plugins.chip(p.category)).toBeVisible()
    await expect(plugins.pluginSearch).toBeVisible()
    await plugins.pluginSearch.fill(p.search)
    const card = plugins.offer(p.name)
    await expect(plugins.offers).toHaveCount(1)
    await expect(card).toHaveCount(1)
    await expect(plugins.in(card, p.category)).toBeVisible()
    await expect(plugins.in(card, m.name)).toBeVisible()
    await expect(plugins.in(card, p.summary)).toBeVisible()
    await expect(plugins.in(card, p.version)).toBeVisible()
    await expect(plugins.buttonIn(card, t.install)).toBeVisible()
    await plugins.pluginSearch.fill('')
    await plugins.chip(p.category).click()
    await expect(card).toBeVisible()
    await plugins.chip(t.all).click()
    await plugins.pluginSearch.fill(p.noMatch)
    await expect(plugins.says(t.nothing)).toBeVisible()
  })

  test("24 · ⋯ hides a marketplace's plugins from Discover and shows them again", covers('plugins/TC-011', 'F-PLUG-07', 'F-PLUG-02'), async ({ page }) => {
    const t = data.text.plugins
    const { market: m, plugin: p } = data.set.s12
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins(t.markets)
    const row = plugins.market(m.name)
    await plugins.buttonIn(row, t.more).click()
    await plugins.menuItem(t.hide).click()
    await expect(plugins.in(row, t.hidden)).toBeVisible()
    await plugins.pluginTab(t.discover).click()
    await plugins.pluginSearch.fill(p.search)
    await expect(plugins.offer(p.name)).toHaveCount(0)
    await plugins.pluginTab(t.markets).click()
    await plugins.buttonIn(row, t.more).click()
    await plugins.menuItem(t.show).click()
    await expect(plugins.in(row, t.hidden)).toHaveCount(0)
    await plugins.pluginTab(t.discover).click()
    await expect(plugins.pluginSearch).toHaveValue(p.search)
    await expect(plugins.offer(p.name)).toBeVisible()
  })

  test('25 · Install opens a sheet with what the plugin may do, and Cancel installs nothing', covers('plugins/TC-003', 'F-PLUG-03'), async ({ page }) => {
    const t = data.text.plugins
    const { market: m, plugin: p } = data.set.s12
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins(t.discover)
    await plugins.pluginSearch.fill(p.search)
    await plugins.buttonIn(plugins.offer(p.name), t.install).click()
    await expect(plugins.sheet).toHaveAccessibleName(fill(t.sheet, { name: p.name, version: p.version }))
    await expect(plugins.sheet).toContainText(fill(t.from, { market: m.name }))
    await expect(plugins.inPart(plugins.sheet, fill(t.warn, { market: m.name }))).toBeVisible()
    await expect(plugins.in(plugins.sheet, t.able)).toBeVisible()
    await expect(plugins.in(plugins.sheet, t.usesNothing)).toBeVisible()
    await expect(plugins.buttonIn(plugins.sheet, t.cancel)).toBeVisible()
    await expect(plugins.buttonIn(plugins.sheet, t.install)).toBeVisible()
    await plugins.buttonIn(plugins.sheet, t.cancel).click()
    await expect(plugins.sheet).toHaveCount(0)
    await expect(plugins.buttonIn(plugins.offer(p.name), t.install)).toBeVisible()
    expect(pluginList().installed).toEqual([])
  })

  test('26 · Install through the sheet adds the plugin to Installed', covers('plugins/TC-004', 'F-PLUG-03', 'F-PLUG-05'), async ({ page }) => {
    const t = data.text.plugins
    const { market: m, plugin: p } = data.set.s12
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins(t.discover)
    await plugins.pluginSearch.fill(p.search)
    await plugins.buttonIn(plugins.offer(p.name), t.install).click()
    await plugins.buttonIn(plugins.sheet, t.install).click()
    await expect(plugins.sheet).toHaveCount(0)
    await expect(plugins.pluginTab(t.installed)).toHaveAccessibleName(`${t.installed}1`)
    await expect(plugins.in(plugins.offer(p.name), t.installedMark)).toBeVisible()
    await plugins.pluginTab(t.installed).click()
    const row = plugins.installedRow(p.name)
    await expect(row).toHaveCount(1)
    await expect(plugins.in(row, `${p.name} ${p.version}`)).toBeVisible()
    await expect(plugins.in(row, p.summary)).toBeVisible()
    await expect(plugins.switchOf(row, fill(t.on, { name: p.name }))).toHaveAttribute('aria-checked', 'true')
    const detail = plugins.detail(p.name)
    await expect(detail).toContainText(fill(t.detail, { version: p.version, market: m.name }))
    await expect(plugins.in(detail, t.ownSettings)).toBeVisible()
    await expect(plugins.in(detail, p.greeting.label)).toBeVisible()
    await expect(plugins.in(detail, p.loud.label)).toBeVisible()
    await expect(plugins.in(detail, t.canDo)).toBeVisible()
    await expect(plugins.in(detail, t.usesNothing)).toBeVisible()
    await expect(plugins.buttonIn(detail, t.removeAsk)).toBeVisible()
    const installed = pluginList().installed
    expect(installed).toHaveLength(1)
    expect(installed[0]).toMatchObject({ id: p.id, enabled: true, version: p.version })
  })

  test("27 · A plugin's own settings are saved when changed", covers('plugins/TC-005', 'F-PLUG-05'), async ({ page }) => {
    const t = data.text.plugins
    const { plugin: p } = data.set.s12
    expect(pluginSetting(p.greeting.key)).toBe(p.greeting.before)
    expect(pluginSetting(p.loud.key)).toBe(p.loud.before)
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins(t.installed)
    await expect(plugins.field(p.name, p.greeting.label)).toHaveValue(p.greeting.before)
    await expect(plugins.fieldSwitch(p.name, p.loud.label)).toHaveAttribute('aria-checked', 'false')
    await plugins.field(p.name, p.greeting.label).fill(p.greeting.after)
    await plugins.field(p.name, p.greeting.label).press(data.text.shell.keys.tab)
    await expect(plugins.field(p.name, p.greeting.label)).toHaveValue(p.greeting.after)
    await plugins.fieldSwitch(p.name, p.loud.label).click()
    await expect(plugins.fieldSwitch(p.name, p.loud.label)).toHaveAttribute('aria-checked', 'true')
    await expect.poll(() => pluginSetting(p.greeting.key)).toBe(p.greeting.after)
    await expect.poll(() => pluginSetting(p.loud.key)).toBe(p.loud.after)
  })

  test("28 · The plugin's switch turns it off and on", covers('plugins/TC-006', 'F-PLUG-05'), async ({ page }) => {
    const t = data.text.plugins
    const { plugin: p } = data.set.s12
    const enabled = () => pluginList().installed.find((x) => x.id === p.id)?.enabled
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins(t.installed)
    const name = fill(t.on, { name: p.name })
    const row = plugins.switchOf(plugins.installedRow(p.name), name)
    const detail = plugins.switchOf(plugins.detail(p.name), name)
    const lines = plugins.rowLines(p.name, p.version, p.summary)
    await expect(row).toHaveAttribute('aria-checked', 'true')
    await expect(detail).toHaveAttribute('aria-checked', 'true')
    await expect(lines).toHaveCSS('opacity', t.opacity.on)
    await detail.click()
    await expect(detail).toHaveAttribute('aria-checked', 'false')
    await expect(row).toHaveAttribute('aria-checked', 'false')
    await expect(lines).toHaveCSS('opacity', t.opacity.off)
    await expect.poll(enabled).toBe(false)
    await detail.click()
    await expect(detail).toHaveAttribute('aria-checked', 'true')
    await expect(row).toHaveAttribute('aria-checked', 'true')
    await expect(lines).toHaveCSS('opacity', t.opacity.on)
    await expect.poll(enabled).toBe(true)
  })

  test('29 · Remove… asks whether to keep the plugin\'s data and Cancel changes nothing', covers('plugins/TC-007', 'F-PLUG-05'), async ({ page }) => {
    const t = data.text.plugins
    const { plugin: p } = data.set.s12
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins(t.installed)
    const detail = plugins.detail(p.name)
    await plugins.buttonIn(detail, t.removeAsk).click()
    const line = plugins.in(detail, fill(t.removeLine, { name: p.name }))
    await expect(line).toBeVisible()
    for (const name of [t.cancel, t.keepData, t.withData]) await expect(plugins.buttonIn(detail, name)).toBeVisible()
    await plugins.buttonIn(detail, t.cancel).click()
    await expect(line).toHaveCount(0)
    await expect(plugins.installedRow(p.name)).toBeVisible()
    expect(pluginList().installed.map((x) => x.id)).toEqual([p.id])
  })

  test('30 · Remove, keep data removes the plugin and leaves its settings', covers('plugins/TC-008', 'F-PLUG-05'), async ({ page }) => {
    const t = data.text.plugins
    const { plugin: p } = data.set.s12
    const set = data.cli('plugin', 'set', p.id, p.greeting.key, p.greeting.after)
    expect(set.code).toBe(0)
    expect(set.out).toBe('')
    expect(pluginSetting(p.greeting.key)).toBe(p.greeting.after)
    const plugins = new PluginsPage(page, data)
    try {
      await plugins.openPlugins(t.installed)
      await plugins.buttonIn(plugins.detail(p.name), t.removeAsk).click()
      await plugins.buttonIn(plugins.detail(p.name), t.keepData).click()
      await expect(plugins.says(t.none)).toBeVisible()
      await expect(plugins.pluginTab(t.installed)).toHaveAccessibleName(t.installed)
      expect(pluginList().installed).toEqual([])
      expect(data.read(data.home, t.dataFolder, p.id, t.settingsFile)).toContain(p.greeting.after)
    } finally {
      // Its teardown in S-12: only the settings it left on purpose (the marketplace stays for stage 31).
      fs.rmSync(path.join(data.home, t.dataFolder, p.id), { recursive: true, force: true })
    }
  })

  test('31 · ⋯ Remove takes an added marketplace away', covers('plugins/TC-012', 'F-PLUG-07'), async ({ page }) => {
    const t = data.text.plugins
    const { market: m } = data.set.s12
    const plugins = new PluginsPage(page, data)
    await plugins.openPlugins(t.markets)
    const row = plugins.market(m.name)
    await plugins.buttonIn(row, t.more).click()
    await expect(plugins.menuItem(t.hide)).toBeVisible()
    await expect(plugins.menuItem(t.remove)).toBeVisible()
    await plugins.menuItem(t.remove).click()
    await expect(row).toHaveCount(0)
    await expect(plugins.pluginTab(t.markets)).toHaveAccessibleName(`${t.markets}1`)
    expect(pluginList().markets.map((x) => x.name)).toEqual([m.builtIn.name])
  })
})
