// A thread's Work tab — its views, its bar (filters, Fields, the sync line), the board (columns, cards), the tree and
// To review — and a work item's details in the right panel.
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

export class WorkPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.details = page.getByRole('complementary', { name: 'Right panel' })
    this.syncNow = page.getByRole('button', { name: 'Sync now', exact: true })
    // History in the details, newest first.
    this.history = this.details.getByRole('list', { name: 'History' }).getByRole('listitem')
    this.status = this.details.getByRole('button', { name: /^● / })
    this.statusBox = this.details.getByRole('combobox', { name: 'Status' })
    this.description = this.details.getByRole('button', { name: /^No description\b/ })
    this.descriptionBox = this.details.getByRole('textbox', { name: 'Description (Markdown)' })
    this.edit = this.details.getByRole('button', { name: 'Edit', exact: true })
    this.titleBox = this.details.getByRole('textbox', { name: 'Title' })
    this.sendToClaude = this.details.getByRole('button', { name: 'Send to Claude' })
    this.copyLink = this.details.getByRole('button', { name: 'Copy link' })
    this.who = this.details.getByRole('group', { name: 'Who does it' })
    this.linkOther = this.details.getByRole('button', { name: 'Link to an item in another thread…' })
    this.findOther = this.details.getByRole('textbox', { name: 'Find an item in another thread' })
    this.cancelLink = this.details.getByRole('button', { name: 'Cancel', exact: true })

    // The views, the bar and the board's own menus.
    this.views = this.main.getByRole('group', { name: 'Views' })
    this.newView = this.views.getByRole('button', { name: '+ New view' })
    this.viewOptions = this.views.getByRole('button', { name: 'View options' })
    this.menu = this.main.getByRole('menu')
    this.viewName = this.menu.getByRole('textbox', { name: 'View name' })
    this.fields = this.main.getByRole('button', { name: 'Fields ▾' })
    this.openOnly = this.main.getByRole('button', { name: 'Open only', exact: true })
    this.addColumn = this.main.getByRole('button', { name: '+ Column', exact: true })
    this.unhide = this.main.getByRole('button', { name: /^\d+ hidden$/ })
    this.columnHeads = this.main.getByRole('group').filter({ has: page.getByRole('button', { name: 'Column options' }) })
    this.yes = this.main.getByRole('button', { name: 'Yes', exact: true })
    this.no = this.main.getByRole('button', { name: 'No', exact: true })
  }

  /** The sync line of the work bar, as it reads ("Synced 11:10 · 3 to review"). */
  syncLine(text) {
    return this.main.getByText(text, { exact: true })
  }

  async open(thread) {
    await this.goto(`#project/${thread}/work`)
  }

  /** A saved view's button, by its name (`aria-current` while it is the one shown). */
  view(name) {
    return this.views.getByRole('button', { name, exact: true })
  }

  /** How many saved views there are (their group also holds View options, + New view and an open view menu). */
  async viewCount() {
    return (await this.views.getByRole('button').count()) - 2 - (await this.views.getByRole('menu').getByRole('button').count())
  }

  /** A select of the bar ("Layout", "Kind", "Owner", "Group by"). */
  select(name) {
    return this.main.getByRole('combobox', { name, exact: true })
  }

  /** An item of the open menu, by its name. */
  menuItem(name) {
    return this.menu.getByRole('button', { name, exact: true })
  }

  /** A field's checkbox in the Fields menu. */
  field(name) {
    return this.menu.getByRole('checkbox', { name, exact: true })
  }

  /** A card of the board, by its title. */
  card(title) {
    return this.main.getByRole('listitem').filter({ has: this.page.getByText(title, { exact: true }) })
  }

  /** A column of the board, by its name: the cards in it. */
  column(name) {
    return this.main.getByRole('list', { name, exact: true })
  }

  /** A column's header, by its name: its count and its menu. */
  columnHead(name) {
    return this.main.getByRole('group', { name, exact: true })
  }

  /** The count a column's header shows ("2", or "0 / 1" with a limit). */
  count(name) {
    return this.columnHead(name).getByText(/^\d+( \/ \d+)?$/)
  }

  columnMenu(name) {
    return this.columnHead(name).getByRole('button', { name: 'Column options' })
  }

  /** An open column menu's boxes: its name, what its cards are, its limit. */
  columnName(name) {
    return this.columnHead(name).getByRole('menu').getByRole('textbox', { name: 'Column name' })
  }

  columnState(name) {
    return this.columnHead(name).getByRole('menu').getByRole('combobox', { name: 'Cards in it are' })
  }

  columnLimit(name) {
    return this.columnHead(name).getByRole('menu').getByRole('spinbutton', { name: 'Limit' })
  }

  /** An item of an open column menu ("Hide", "Delete column"). */
  columnItem(name, item) {
    return this.columnHead(name).getByRole('menu').getByRole('button', { name: item, exact: true })
  }

  /** A row of the tree, by the id it starts with. */
  treeRow(id) {
    return this.main.getByRole('button', { name: new RegExp(`^${literal(id)} `) })
  }

  /** Where a tree row's id is drawn, from the left (how far the row is indented). */
  async indent(id) {
    return (await this.treeRow(id).getByText(id, { exact: true }).boundingBox()).x
  }

  /** Open a work item's details from its card on the board, by its title. */
  async openItem(title) {
    await this.card(title).click()
    await this.details.getByRole('heading', { name: title, level: 3 }).waitFor()
  }

  /** The details' heading: the open item's title. */
  get itemTitle() {
    return this.details.getByRole('heading', { level: 3 })
  }

  /** A field's value in the details, by the field's name ("Owner", "Handed over by", "Due"). */
  value(field) {
    return this.details.getByRole('button', { name: new RegExp(`^${literal(field)}:`) })
  }

  /** A date typed the way a person does: a click on the box's first part, then each key (a programmatic fill sets the
   *  whole date at once). */
  async typeDate(box, keys) {
    const { height } = await box.boundingBox()
    await box.click({ position: { x: 8, y: height / 2 } })
    for (const k of keys) await this.page.keyboard.press(k)
  }

  /** Open a field for a change: its box, by the field's name. */
  async editField(field) {
    await this.value(field).click()
    return this.details.getByLabel(field, { exact: true })
  }

  /** The names a field's box suggests as it is typed in: its list's options (hidden until the browser draws its own
   *  popup, so found with hidden ones; an option of a suggestion list is named only by its value). */
  async suggestions() {
    const options = this.details.getByRole('listbox', { includeHidden: true }).getByRole('option', { includeHidden: true })
    const names = []
    for (const o of await options.all()) names.push(await o.getAttribute('value'))
    return names
  }

  /** A link to another item in the details ("T001 Send Acme the SSO checklist"), or any button there, by its name. */
  link(name) {
    return this.details.getByRole('button', { name, exact: true })
  }

  /** A text of the details, as it reads (a row's name, a description). */
  detail(text) {
    return this.details.getByText(text, { exact: true })
  }

  /** A story's criterion, its checkbox. */
  criterion(text) {
    return this.details.getByRole('checkbox', { name: text })
  }

  /** A result of the search for an item in another thread, by its name ("Acme rollout · T001 Send Acme the SSO checklist"). */
  result(name) {
    return this.details.getByRole('group', { name, exact: true })
  }
}
