// A thread's People tab — its view switch (Map, Org chart, List), the Map's quadrants and tray, the List's two tables,
// + Person, the suggestions from its calls — and a person's details in the right panel.
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'
import { readLines } from '../support/aria.mjs'

export class PeoplePage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.views = this.main.getByRole('group', { name: 'View', exact: true })
    this.addPerson = this.main.getByRole('button', { name: '+ Person', exact: true })
    this.newPerson = this.main.getByRole('textbox', { name: 'New person' })
    // The List: the thread's people, then those only seen in its calls.
    this.inThread = this.main.getByRole('table', { name: 'In this thread' })
    this.seen = this.main.getByRole('table', { name: 'Seen in calls' })
    // A client's People tab: the people of its threads.
    this.inClient = this.main.getByRole('table', { name: "In this client's threads" })
    // The Map: the tray of people not placed yet.
    this.tray = this.main.getByRole('group', { name: 'Not placed yet' })
    this.suggestions = this.main.getByRole('list', { name: 'Suggestions' }).getByRole('listitem')
    this.peopleTab = this.main.getByRole('tab', { name: /^People/ })

    // A person in the right panel.
    this.panel = page.getByRole('complementary', { name: 'Right panel' })
    this.stance = this.panel.getByRole('combobox', { name: 'Stance', exact: true })
    this.organisation = this.panel.getByRole('combobox', { name: 'Organisation', exact: true })
    this.sameAs = this.panel.getByRole('button', { name: 'Same person as…', exact: true })
    this.samePick = this.panel.getByRole('combobox', { name: 'Same person as', exact: true })
    this.merge = this.panel.getByRole('button', { name: 'Merge', exact: true })
    this.cancelMerge = this.panel.getByRole('button', { name: 'Cancel', exact: true })
    this.notSame = this.panel.getByRole('button', { name: 'Not the same', exact: true })
    this.nameOrg = this.panel.getByRole('button', { name: /^Name this organisation / })
    this.orgName = this.panel.getByRole('textbox', { name: 'Organisation name' })
    this.addOrg = this.panel.getByRole('button', { name: 'Add', exact: true })
  }

  async open(thread) {
    await this.goto(`#project/${thread}/people`)
  }

  /** A button of the view switch ("Map", "Org chart", "List"): `aria-pressed` while it is the one shown. */
  view(name) {
    return this.views.getByRole('button', { name, exact: true })
  }

  /** A text of the tab, as it reads. */
  says(text) {
    return this.main.getByText(text, { exact: true })
  }

  /** A person's row in a table of the List, by the name it starts with. */
  row(table, name) {
    return table.getByRole('row', { name: new RegExp(`^${literal(name)} `) })
  }

  /** A person's cell in a table, by its column's header ("Owes"). */
  async cell(table, name, column) {
    const i = (await this.columns(table).allTextContents()).indexOf(column)
    if (i < 0) throw new Error(`no column ${column}`)
    return this.row(table, name).getByRole('cell').nth(i)
  }

  /** A column header of a table. */
  columns(table) {
    return table.getByRole('columnheader')
  }

  /** A quadrant of the Map ("Manage closely"). */
  quadrant(name) {
    return this.main.getByRole('region', { name, exact: true })
  }

  /** A person's chip on the Map, or in one of its areas (the tray, a quadrant): its name, then what they are ("Dana Lee Acme"). */
  chip(name, area = this.main) {
    return area.getByRole('button', { name: new RegExp(`^${literal(name)}( |$)`) })
  }

  /** A suggestion from the calls, by words of its question. */
  suggestion(words) {
    return this.suggestions.filter({ hasText: words })
  }

  /** A button of a suggestion ("Yes", "No", "Change…", "Save", "Cancel"). */
  answer(words, name) {
    return this.suggestion(words).getByRole('button', { name, exact: true })
  }

  /** The count on the People tab (0 while it shows none). */
  async count() {
    return Number((await this.peopleTab.textContent()).replace(/\D/g, '') || 0)
  }

  // ── The full set: what an area of the Map reads, the stance colours, a table's rows, the tag "new" ──

  /** What a quadrant (or the tray) reads, in order: its title and subtitle, then its chips. */
  async areaReads(area) {
    const snap = await area.ariaSnapshot()
    const lines = readLines(snap)
    if (lines.length) return lines
    // An area with nothing in it reads its title on its own line.
    const only = /^- \w+ "[^"]*": (.*)$/.exec(snap.split('\n')[0])
    return only ? [only[1]] : []
  }

  /** The colour of a chip's avatar (its stance), and of the legend's swatch for a stance. */
  async avatarColour(chip) {
    return chip.evaluate((el) => getComputedStyle(el.firstElementChild).backgroundColor)
  }

  /** A stance of the legend above the Map ("champion"). */
  legend(stance) {
    return this.main.getByText(stance, { exact: true }).first()
  }

  async legendColour(stance) {
    return this.main.getByText(stance, { exact: true }).first().evaluate((el) => getComputedStyle(el.firstElementChild).backgroundColor)
  }

  /** A table's rows (its header left out), in order. */
  bodyRows(table) {
    return table.getByRole('rowgroup').nth(1).getByRole('row')
  }

  /** The tag "new" beside a name: every one of the view, and a person's in a table. */
  get newTags() {
    return this.main.getByText('new', { exact: true })
  }

  /** A person's tag "new" in a table; a button of their row ("Add"); every tag "quiet" of the view. */
  rowNew(table, name) {
    return this.row(table, name).getByText('new', { exact: true })
  }

  rowButton(table, name, button) {
    return this.row(table, name).getByRole('button', { name: button, exact: true })
  }

  /** A button of the view, by its name ("Add all"). */
  button(name) {
    return this.main.getByRole('button', { name, exact: true })
  }

  get quietTags() {
    return this.main.getByText('quiet', { exact: true })
  }

  /** A paragraph of the view, by words it holds. */
  paragraph(words) {
    return this.main.getByRole('paragraph').filter({ hasText: words })
  }

  /** A row of a table: what its cell under a column reads (by the column's header). */
  async cellText(table, name, column) {
    return (await this.cell(table, name, column)).textContent()
  }

  // ── the panel ──

  /** A level of the panel ("influence", "interest"): the word it shows after its buttons. */
  async level(name) {
    return readLines(await this.panel.getByRole('group', { name, exact: true }).ariaSnapshot()).at(-1)
  }

  /** A select of the panel, by its name ("Governance", "Works for in this thread"). */
  panelSelect(name) {
    return this.panel.getByRole('combobox', { name, exact: true })
  }

  /** A text of the panel that ends with these words ("Tue, Nov 10 · yours (Echo leaves it)"). */
  detailEnding(words) {
    return this.panel.getByText(new RegExp(`${literal(words)}$`)).filter({ visible: true })
  }

  /** A button of the panel, by its name. */
  panelButton(name) {
    return this.panel.getByRole('button', { name, exact: true })
  }

  /** The facts said about the person: each its quote, in the block "What was said about them". */
  get facts() {
    return this.panel.getByText(/^“[^”]*”/)
  }

  /** A text of the panel, as it reads (not a select's option of the same words). */
  detail(text) {
    return this.panel.getByText(text, { exact: true }).filter({ visible: true })
  }

  /** A field's value in the panel, by the field's name ("Title: Set", "Role: Sponsor"). */
  value(field) {
    return this.panel.getByRole('button', { name: new RegExp(`^${literal(field)}:`) })
  }

  /** Open a field for a change: its box, by the field's name. */
  async editField(field) {
    await this.value(field).click()
    return this.panel.getByRole('textbox', { name: field, exact: true })
  }

  /** A select's chosen option, by its text. */
  chosen(select, text) {
    return select.getByRole('option', { name: text, exact: true, selected: true })
  }

  /** An address of the person (its Copy button names it). */
  address(email) {
    return this.panel.getByRole('button', { name: `Copy ${email}`, exact: true })
  }
}
