// Settings → Templates → Skills: the catalogue (a row a skill, its tag and its switch), a skill's editor (its SKILL.md,
// Customize, Reset), New skill, and the toast a refusal floats.
import { SettingsPage } from './settings.page.mjs'
import { fill, literal } from '../support/data.mjs'

export class SkillsPage extends SettingsPage {
  constructor(page, data) {
    super(page, data)
    const k = data.text.skills
    this.k = k
    this.notices = page.getByRole('status', { name: 'Notices', exact: true })
  }

  /** Templates, on the tab Skills. */
  async openSkills() {
    await this.open(this.data.text.settings.templates.route)
    await this.tab(this.k.tab).click()
  }

  /** The tags a skill's row carries ("Built in", "Customized", "Yours"), as one alternative. */
  get skillTags() {
    return [this.k.builtIn, this.k.customized, this.k.yours].map(literal).join('|')
  }

  /** A skill's row, by its name: it reads "<name> <tag> <name> on <description>". */
  skill(name) {
    return this.main.getByRole('button', { name: new RegExp(`^${literal(name)} (${this.skillTags}) `) })
  }

  /** Every skill's row. */
  get skills() {
    return this.main.getByRole('button', { name: new RegExp(` (${this.skillTags}) .* on `) })
  }

  /** A text of a row or an editor, as it reads; or one that starts with these words. */
  in(where, text) {
    return where.getByText(text, { exact: true })
  }

  inStarting(where, text) {
    return where.getByText(new RegExp(`^${literal(text)}`))
  }

  /** A text of an area that matches a pattern (a skill's SKILL.md as the editor shows it). */
  inMatching(where, pattern) {
    return where.getByText(new RegExp(pattern))
  }

  /** A skill's switch: its checkbox, and the switch drawn over it (the label whose tooltip says on or off). */
  switchBox(name) {
    return this.skill(name).getByRole('checkbox', { name: fill(this.k.switch, { name }), exact: true })
  }

  switchTip(name, tip) {
    return this.skill(name).getByTitle(tip, { exact: true })
  }

  /** The skill's name as its row draws it (dimmed while it is off). */
  skillName(name) {
    return this.skill(name).getByText(name, { exact: true })
  }

  /** The colour the row draws the skill's name in (dimmed while it is off). */
  async nameColour(name) {
    return this.skillName(name).evaluate((el) => getComputedStyle(el).color)
  }

  /** A button of the editor (or of the list above it), by its name. */
  editorButton(title, name) {
    return this.editor(title).getByRole('button', { name, exact: true })
  }

  /** The box of New skill, by its label. */
  editorBox(title, name) {
    return this.editor(title).getByRole('textbox', { name, exact: true })
  }

  /** A floating notice, by what it says, and its OK. */
  notice(text) {
    return this.notices.getByText(text, { exact: true })
  }

  get noticeOk() {
    return this.notices.getByRole('button', { name: this.data.text.notices.ok, exact: true })
  }
}
