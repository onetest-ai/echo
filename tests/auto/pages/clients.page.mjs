// Threads and clients: the Threads activity's left list (its client groups and threads), a client's page (its header,
// tabs, Overview, Calls, Tasks and Profile rows) and a thread's header, menu and Profile.
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

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
}
