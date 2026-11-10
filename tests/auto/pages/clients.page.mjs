// Threads and clients: the Threads activity's left list (its client groups and threads), a client's page (its header,
// tabs, Overview, Calls, Tasks and Profile rows) and a thread's header, menu and Profile.
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'
import { readLines } from '../support/aria.mjs'

/** A snapshot's text as it reads, without the quotes YAML puts around a text with a colon in it. */
const unquote = (l) => (/^".*"$/.test(l) ? JSON.parse(l) : l)

export class ClientsPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.list = this.main.getByRole('complementary')
    this.tabs = this.main.getByRole('tablist').getByRole('tab')
    // A thread's header: the tag of its client.
    this.clientTag = this.main.getByTitle('Client', { exact: true })
    this.claudeAbout = this.main.getByRole('button', { name: /^Claude about / })
    // Overview: About the client, edited in place.
    this.editAbout = this.main.getByRole('button', { name: 'Edit', exact: true })
    this.aboutBox = this.main.getByRole('textbox', { name: 'About the client (Markdown)' })
    // Calls: a row a call.
    this.callRows = this.main.getByRole('list', { name: 'Calls', exact: true }).getByRole('listitem')
    // Profile: a new thread, and removing the client.
    this.newThread = this.main.getByRole('button', { name: '+ New thread', exact: true })
    this.newThreadName = this.main.getByRole('textbox', { name: 'New thread name' })
    this.removeClient = this.main.getByRole('button', { name: 'Remove this client…', exact: true })
    this.confirmRemove = this.main.getByRole('button', { name: 'Remove', exact: true })
    this.cancel = this.main.getByRole('button', { name: 'Cancel', exact: true })
    // A thread: its menu, and its Profile's organisations.
    this.threadActions = this.main.getByRole('button', { name: 'Thread actions' })
    this.addOrganisation = this.main.getByRole('button', { name: '+ Organisation', exact: true })
    this.organisation = this.main.getByRole('combobox', { name: 'Organisation', exact: true })
  }

  /** A client's page on a tab (`overview`, `calls`, `tasks`, `people`, `profile`). */
  async openClient(id, tab = 'overview') {
    await this.goto(`#client/${id}/${tab}`)
  }

  /** A thread on a tab (`brief`, `work`, `calls`, `people`, `profile`). */
  async openThread(id, tab) {
    await this.goto(`#project/${id}/${tab}`)
  }

  /** A client's group header in the left list, by its name as it reads ("Initech · your company"). */
  group(name) {
    return this.list.getByRole('button', { name, exact: true })
  }

  /** A text of the view, as it reads (the tag, the facts line, a hint). */
  says(text) {
    return this.main.getByText(text, { exact: true })
  }

  /** A tab of the page, by the name it starts with ("Calls" reads "Calls1" with its count). */
  tab(name) {
    return this.main.getByRole('tab', { name: new RegExp(`^${literal(name)}`) })
  }

  /** An item of the open thread menu. */
  menuItem(name) {
    return this.main.getByRole('menuitem', { name, exact: true })
  }

  /** A row of a client's Profile, by its label ("Other names", "Mail domains", "Threads"). */
  row(label) {
    return this.main.getByRole('group', { name: label, exact: true })
  }

  /** A row's "+ Add" and the box it opens. */
  addIn(label) {
    return this.row(label).getByRole('button', { name: '+ Add', exact: true })
  }

  addBox(label) {
    return this.row(label).getByRole('textbox')
  }

  /** A token of a row (a name, a domain: it reads with its × button), and that button. */
  token(label, value) {
    return this.row(label).getByText(new RegExp(`^${literal(value)}×$`))
  }

  removeToken(value) {
    return this.main.getByRole('button', { name: `Remove ${value}`, exact: true })
  }

  /** A thread's card on a client's Overview (its title button reads "<name> <stage>"), or its button in a row. */
  card(name) {
    return this.main.getByRole('button', { name, exact: true })
  }

  /** A task's card on a client's Tasks tab, by its title. */
  task(title) {
    return this.main.getByRole('button', { name: new RegExp(`^${literal(title)} `) })
  }

  /** A pile's heading, with its count ("I owe 1"). */
  pile(name, n) {
    return this.main.getByRole('heading', { name: `${name} ${n}`, exact: true, level: 3 })
  }

  /** The folded pile of others' tasks ("Others · 1 — …"). */
  others(n) {
    return this.main.getByRole('button', { name: new RegExp(`^Others · ${n} `) })
  }

  // ── Threads: the left list in order, a client's fold, New thread and Found for you, a thread's Profile rows, its
  //    Brief and its Calls ──

  /** What the left list reads from the top, in order: its buttons' names, each client group's name. */
  async listLines() {
    return readLines(await this.list.ariaSnapshot())
  }

  /** A client's group in the left list ("Acme", "Initech · your company"): its fold, and its count while folded. */
  clientGroup(name) {
    return this.list.getByRole('group', { name, exact: true })
  }

  fold(name) {
    return this.clientGroup(name).getByRole('button', { name: /its threads$/ })
  }

  foldCount(name, n) {
    return this.clientGroup(name).getByText(String(n), { exact: true })
  }

  /** A thread's row in the left list, by its name (it reads "<name> <n> call(s) · <last call> <stage>"). */
  threadRow(name) {
    return this.list.getByRole('button', { name: new RegExp(`^${literal(name)} \\d+ calls?\\b`) })
  }

  /** The left list's New thread (+), Found for you (with its count while it has one), and Closed · N. */
  get addThread() {
    return this.list.getByRole('button', { name: 'New thread', exact: true })
  }

  get foundForYou() {
    return this.list.getByRole('button', { name: /^Found for you\b/ })
  }

  get closedFold() {
    return this.list.getByRole('button', { name: /Closed · \d+$/ })
  }

  /** Found for you's new thread: its Client select and Create. */
  get threadClient() {
    return this.main.getByRole('combobox', { name: 'Client', exact: true })
  }

  get create() {
    return this.main.getByRole('button', { name: 'Create', exact: true })
  }

  /** A button of a Profile row, by its name ("+ Add", "Remove", "Active"); a text of the row; a select of the row. */
  rowButton(label, name) {
    return this.row(label).getByRole('button', { name, exact: true })
  }

  rowSays(label, text) {
    return this.row(label).getByText(text, { exact: true })
  }

  rowSelect(label, name) {
    return this.row(label).getByRole('combobox', { name, exact: true })
  }

  /** A box of the Profile, by its name ("Since", "Notes for Claude", "Client name"). */
  box(name) {
    return this.main.getByRole('textbox', { name, exact: true })
  }

  /** A day as a thread's Since reads it ("Nov 1, 2026"): a `YYYY-MM-DD` from a file or the CLI. */
  async sinceDay(date) {
    return this.page.evaluate((d) => new Date(d + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' }), date)
  }

  /** The Brief: its box while it is edited, a paragraph of the view (of the Brief, the Profile's foot), its bold
   *  words, a call of Latest calls by its title. */
  get briefBox() {
    return this.main.getByRole('textbox', { name: "The thread's brief (Markdown)", exact: true })
  }

  paragraph(text) {
    return this.main.getByRole('paragraph').filter({ hasText: text })
  }

  get bold() {
    return this.main.getByRole('strong')
  }

  latestCall(title) {
    return this.main.getByRole('button', { name: new RegExp(`^\\S+ ${literal(title)}$`) })
  }

  /** A thread's header (named by its heading), and a text of it as it reads: its client and stage tags, its facts
   *  line; or one that ends with these words. */
  head(thread) {
    return this.main.getByRole('group', { name: thread, exact: true })
  }

  fact(thread, text) {
    return this.head(thread).getByText(text, { exact: true })
  }

  factEnding(thread, words) {
    return this.head(thread).getByText(new RegExp(`${literal(words)}$`))
  }

  /** A text of a Profile row that starts with these words (a line that goes on, such as the stage's history). */
  rowStarting(label, words) {
    return this.row(label).getByText(new RegExp(`^${literal(words)}`))
  }

  /** A stage group of the Stage row ("Pursuit", "Delivery", "Closed"): its buttons, in order. */
  stageGroup(name) {
    return this.row('Stage').getByRole('group', { name, exact: true }).getByRole('button')
  }

  /** A Profile row's buttons, in order. */
  rowButtons(label) {
    return this.row(label).getByRole('button')
  }

  /** A row of the Calls tab, by its call's title, and a button of that row ("Unpin", "Take out", the title). */
  callRow(title) {
    return this.callRows.filter({ has: this.page.getByRole('button', { name: title, exact: true }) })
  }

  callButton(title, name) {
    return this.callRow(title).getByRole('button', { name, exact: true })
  }

  /** A heading of the view's own sections (level 3: "Latest calls"); the left list's heading (level 1: "Threads"). */
  subheading(name) {
    return this.main.getByRole('heading', { name, level: 3, exact: true })
  }

  listHeading(name) {
    return this.list.getByRole('heading', { name, level: 1, exact: true })
  }

  /** A select's options, in order, and the one chosen, by its name. */
  options(select) {
    return select.getByRole('option')
  }

  chosen(select, name) {
    return select.getByRole('option', { name, exact: true, selected: true })
  }

  /** A button of the view, by its name ("Save", "Move to trash", "Edit"). */
  button(name) {
    return this.main.getByRole('button', { name, exact: true })
  }

  // ── The full set: a thread row's name, a row's reading, a client's cards, the calls rows, Found for you's cards ──

  /** What a thread's row in the left list reads: its name (twice when it wears a picture, named by it), its calls and
   *  the day of the last one, its stage (none for a closed thread). */
  rowName({ name, calls, day, stage, logo }) {
    const n = `${calls} call${calls === 1 ? '' : 's'}${day ? ` · ${day}` : ''}`
    return [name, logo && name, n, stage].filter(Boolean).join(' ')
  }

  /** What a Profile row reads after its label, in order: its texts and buttons (a token, its "Remove", "+ Add", the
   *  line under them). */
  async rowReads(label) {
    const lines = readLines(await this.row(label).ariaSnapshot()).map(unquote)
    if (lines[0] === label) return lines.slice(1)
    if (lines[0]?.startsWith(`${label} `)) return [lines[0].slice(label.length + 1), ...lines.slice(1)]
    return lines
  }

  /** The Calls tab's rows, each as it reads: its title, its line (day, time, why, minutes) and its button. */
  async callRowsRead() {
    const out = []
    for (const r of await this.callRows.all()) out.push(readLines(await r.ariaSnapshot()).map(unquote))
    return out
  }

  /** A thread's card on a client's Overview (a group named by the thread), and what it reads, in order. */
  overviewCard(name) {
    return this.main.getByRole('group', { name, exact: true })
  }

  async cardReads(name) {
    return readLines(await this.overviewCard(name).ariaSnapshot()).map(unquote)
  }

  /** A text of a card that starts with these words ("2 calls · last …"), its "N late". */
  cardSays(name, text) {
    return this.overviewCard(name).getByText(text, { exact: true })
  }

  /** A button of a card (its title, a decision, a risk), by its name. */
  cardButton(name, button) {
    return this.overviewCard(name).getByRole('button', { name: button, exact: true })
  }

  /** The day a card writes its last call on ("last saturday"): the lists' day, in lower case. */
  async cardDay(iso) {
    return (await this.dayLabel(iso)).toLowerCase()
  }

  /** Of these threads, the ones with a card on the Overview, in the cards' order. */
  async cardOrder(names) {
    const out = []
    for (const g of await this.main.getByRole('group').all()) {
      const n = await g.getAttribute('aria-label')
      if (names.includes(n)) out.push(n)
    }
    return out
  }

  /** A picture of the view or of the left list, by its name (a logo: its client's or thread's name). */
  img(name) {
    return this.main.getByRole('img', { name, exact: true })
  }

  /** A Profile row's picture (the Logo row's). */
  rowPicture(label) {
    return this.row(label).getByRole('img')
  }

  /** The client's picture in the head of its page (the left list's, of its group, comes first). */
  headImg(name) {
    return this.main.getByRole('img', { name, exact: true }).last()
  }

  /** A client's group in the left list, its picture; a thread row's picture. */
  groupImg(name) {
    return this.clientGroup(name).getByRole('img')
  }

  rowImg(name) {
    // A row with a picture reads its name twice (the picture is named by it).
    return this.list.getByRole('button', { name: new RegExp(`^${literal(name)} ${literal(name)} \\d+ calls?\\b`) }).getByRole('img')
  }

  /** A link of the view, by its name. */
  link(name) {
    return this.main.getByRole('link', { name, exact: true })
  }

  /** A list item of the view (About's bullets), as it reads. */
  get bullets() {
    return this.main.getByRole('listitem')
  }

  /** Found for you: a candidate's card (a group named by its name), its why line, its calls (each "<day> <title>"),
   *  its selects. */
  foundCard(name) {
    return this.main.getByRole('group', { name, exact: true })
  }

  foundSays(name, text) {
    return this.foundCard(name).getByText(text, { exact: true })
  }

  foundCall(name, title) {
    return this.foundCard(name).getByRole('button', { name: new RegExp(` ${literal(title)}$`) })
  }

  foundCalls(name) {
    return this.foundCard(name).getByRole('button').filter({ hasNotText: /^(Accept|Reject)$/ })
  }

  foundSelect(name, select) {
    return this.foundCard(name).getByRole('combobox', { name: select, exact: true })
  }

  foundBox(name) {
    return this.foundCard(name).getByRole('textbox', { name: 'Thread name' })
  }

  /** Every Found for you card. */
  get foundCards() {
    return this.main.getByRole('group').filter({ has: this.page.getByRole('button', { name: 'Accept', exact: true }) })
  }

  /** A select's option of a Found for you card, by its name; a button of the card ("Accept", "Reject"). */
  foundOption(name, select, option) {
    return this.foundSelect(name, select).getByRole('option', { name: option, exact: true })
  }

  foundButton(name, button) {
    return this.foundCard(name).getByRole('button', { name: button, exact: true })
  }

  /** A token of a Profile row by its tooltip (an organisation's domain). */
  titled(label, title) {
    return this.row(label).getByTitle(title, { exact: true })
  }

  /** A text of a thread's header that starts with these words ("6 calls · since 2026-01-01"). */
  factStarting(thread, words) {
    return this.head(thread).getByText(new RegExp(`^${literal(words)}`))
  }

  /** A thread's row in the left list, by all it reads (`rowName`). */
  rowNamed(name) {
    return this.list.getByRole('button', { name, exact: true })
  }

  /** Found for you's new thread row: its Client select (the page's first; each card has its own below). */
  get newThreadClient() {
    return this.main.getByRole('combobox', { name: 'Client', exact: true }).first()
  }

  /** The names Found for you's cards hold in their boxes, in order. */
  async foundNames() {
    const names = []
    for (const box of await this.main.getByRole('textbox', { name: 'Thread name', exact: true }).all()) names.push(await box.inputValue())
    return names
  }

  /** A switch of the view ("Your own company"). */
  switch(name) {
    return this.main.getByRole('switch', { name, exact: true })
  }

  /** The open menu's items, in order. */
  get menuItems() {
    return this.main.getByRole('menu').getByRole('menuitem')
  }
}
