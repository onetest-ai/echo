// Settings: its sections (the list on the left) and Layout (where each panel view opens, Reset).
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

export class SettingsPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.sections = this.main.getByRole('complementary')
    this.reset = this.main.getByRole('button', { name: 'Reset', exact: true })
    // Layout's rows: a view's Right and Bottom, in a group named as the view.
    this.layoutRows = this.main.getByRole('group').filter({ has: page.getByRole('button', { name: 'Bottom', exact: true }) })
  }

  async open(route) {
    await this.goto(route)
  }

  /** A section of the list on the left, by its name. */
  section(name) {
    return this.sections.getByRole('button', { name, exact: true })
  }

  /** Layout: a view's place button ("Right", "Bottom"). */
  place(view, where) {
    return this.main.getByRole('group', { name: view, exact: true }).getByRole('button', { name: where, exact: true })
  }

  // ── Search, the sections' controls ──

  get search() {
    return this.sections.getByRole('textbox', { name: 'Search settings' })
  }

  /** The sections the list on the left shows, in order (Health's name carries its warnings: "Health ⚠ 1"). */
  get sectionButtons() {
    return this.sections.getByRole('button')
  }

  /** The settings a search picked out (their titles, marked). */
  get hits() {
    return this.main.getByRole('mark')
  }

  /** A group of buttons, by its name ("Appearance", "Calendar meetings"), and one of its buttons. */
  choice(group, name) {
    return this.main.getByRole('group', { name: group, exact: true }).getByRole('button', { name, exact: true })
  }

  /** A switch, a select, a text box, a button, a text, by its name. */
  switch(name) {
    return this.main.getByRole('switch', { name, exact: true })
  }

  select(name) {
    return this.page.getByRole('combobox', { name, exact: true })
  }

  box(name) {
    return this.main.getByRole('textbox', { name, exact: true })
  }

  button(name) {
    return this.main.getByRole('button', { name, exact: true })
  }

  says(text) {
    return this.main.getByText(text, { exact: true })
  }

  /** A select's chosen option. */
  chosen(select, text) {
    return this.select(select).getByRole('option', { name: text, exact: true, selected: true })
  }

  // ── A setting's row (named by its title), AI & minutes' profiles, the account chip's picker ──

  /** A row of a card, by its name ("Account", a speech model, an AI profile). */
  settingRow(name) {
    return this.main.getByRole('group', { name, exact: true })
  }

  /** A card, by its heading ("Profiles"). */
  card(name) {
    return this.main.getByRole('region', { name, exact: true })
  }

  /** A card's heading (level 3), and its rows (named groups). */
  cardHeading(name) {
    return this.main.getByRole('heading', { name, level: 3, exact: true })
  }

  cardRows(name) {
    return this.card(name).getByRole('group')
  }

  /** A text of a row, as it reads; one it contains; a button of it, or all of them; a switch of it. */
  rowSays(row, text) {
    return row.getByText(text, { exact: true })
  }

  rowHas(row, text) {
    return row.getByText(text)
  }

  rowButton(row, name) {
    return row.getByRole('button', { name, exact: true })
  }

  rowButtons(row) {
    return row.getByRole('button')
  }

  rowSwitch(row, name) {
    return row.getByRole('switch', { name, exact: true })
  }

  /** A progress bar on the page (a download running). */
  get progress() {
    return this.main.getByRole('progressbar')
  }

  /** The top bar's selects; the account chip's picker (with two profiles or more), and what the chip reads. */
  get topSelects() {
    return this.topBar.getByRole('combobox')
  }

  topSelect(name) {
    return this.topBar.getByRole('combobox', { name, exact: true })
  }

  /** A top bar select's options; one by its name (chosen or not). */
  topOptions(select) {
    return this.topSelect(select).getByRole('option')
  }

  topOption(select, name, selected) {
    return this.topSelect(select).getByRole('option', { name, exact: true, ...(selected === undefined ? {} : { selected }) })
  }

  chipReads(name) {
    // The chip's own label comes before its picker's options, which read the same.
    return this.topBar.getByText(name, { exact: true }).first()
  }

  /** A text of the page that contains these words. */
  saysPart(text) {
    return this.main.getByText(text)
  }

  // ── Templates ──

  tab(name) {
    return this.main.getByRole('tab', { name: new RegExp(`^${literal(name)}`) })
  }

  /** The tags a row of the templates' lists carries ("Built in", "Edited", "Yours"), as one alternative. */
  get tags() {
    const tp = this.data.text.settings.templates
    return [tp.builtIn, tp.edited, tp.yours].map(literal).join('|')
  }

  /** A row of a list (a template, a meeting type), by its name; it reads "<name> <tag> <use>". */
  row(name) {
    return this.main.getByRole('button', { name: new RegExp(`^${literal(name)} (${this.tags}) `) })
  }

  /** Every row of the list shown, in order; with a tag, only the rows with it. */
  rows(tag) {
    return this.main.getByRole('button', { name: new RegExp(` (${tag ? literal(tag) : this.tags}) `) })
  }

  /** The editor of a template or type, by its title ("Status sync", "New template"). */
  editor(title) {
    return this.main.getByRole('region', { name: title, exact: true })
  }

  /** A button of an editor, by its name ("Reset to built-in", "Delete", "Create"). */
  editorButton(title, name) {
    return this.editor(title).getByRole('button', { name, exact: true })
  }

  /** A section's heading boxes in an editor, in order. */
  headings(title) {
    return this.editor(title).getByRole('textbox', { name: 'Heading', exact: true })
  }

  /** An editor's word chips, by their × buttons ("Remove daily"; a section's reads "Remove the section …"). */
  words(title) {
    const tp = this.data.text.settings.templates
    const [word, section] = [tp.removeWord.replace('{word}', ''), tp.removeSection.replace('{s}', '')]
    return this.editor(title).getByRole('button', { name: new RegExp(`^${literal(word)}(?!${literal(section.slice(word.length))})`) })
  }

  /** Settings → Templates, a template's editor open on it. */
  async openTemplate(name) {
    await this.open(this.data.text.settings.templates.route)
    await this.row(name).click()
    await this.editor(name).waitFor()
  }

  /** A group of the page, by its name ("Appearance"). */
  group(name) {
    return this.main.getByRole('group', { name, exact: true })
  }

  /** The page's whole text, as it reads. */
  async pageText() {
    return this.main.innerText()
  }

  /** The page's switches (none while config.json can't be read). */
  get switches() {
    return this.main.getByRole('switch')
  }
}
