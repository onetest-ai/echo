// Today: Needs you now (its items; ✦ and My work's Claude dialog; I'll do it and Won't do from ⌘K), Asks, and My work ·
// this week (a task's ✦, Yolo and its outcome above the list).
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

export class TodayPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    const t = data.text
    this.needs = page.getByRole('region', { name: 'Needs you now' })
    this.asks = page.getByRole('region', { name: 'Asks' })

    // I'll do it
    this.taskDialog = page.getByRole('dialog', { name: 'A task of yours' })
    this.taskTitle = this.taskDialog.getByLabel('What to do')
    this.taskThread = this.taskDialog.getByLabel('In which thread')
    this.addToTasks = this.taskDialog.getByRole('button', { name: 'Add to Tasks' })

    // Won't do
    this.wontDialog = page.getByRole('dialog', { name: "Won't do" })
    this.wontComment = this.wontDialog.getByLabel(/Why, or who it's for/)
    this.wontConfirm = this.wontDialog.getByRole('button', { name: "Won't do" })
    this.wontCancel = this.wontDialog.getByRole('button', { name: 'Cancel' })

    // My work's Claude dialog (✦), on a task or on an item of Needs you now
    this.claudeDialog = page.getByRole('dialog', { name: t.needs.claudeDialog, exact: true })
    this.claudeSkill = this.claudeDialog.getByLabel('Skill')
    this.claudeWorkOnIt = this.claudeDialog.getByRole('button', { name: t.needs.workOnIt })
    this.claudeYolo = this.claudeDialog.getByRole('button', { name: t.needs.yolo })
    this.claudeCancel = this.claudeDialog.getByRole('button', { name: t.needs.cancel })
    // The last Yolo's outcome, above My work: Claude is on it, then what it says, with Dismiss.
    this.askBar = page.getByRole('status', { name: t.needs.yolo })
    this.yoloWorking = this.askBar.getByText(t.needs.working)
    this.dismiss = this.askBar.getByRole('button', { name: t.needs.dismiss })

    // Close this task? (a task of My work's Close button)
    this.closeDialog = page.getByRole('dialog', { name: 'Close this task?' })
    this.closeNote = this.closeDialog.getByLabel(/^A comment, if you like/)
    this.closeIt = this.closeDialog.getByRole('button', { name: 'Close it', exact: true })
    this.closeCancel = this.closeDialog.getByRole('button', { name: 'Cancel', exact: true })
    // The Claude dialog's chosen skill: its cross, and the list it opens.
    this.noSkill = this.claudeDialog.getByRole('button', { name: 'No skill — just the task' })
    this.skills = this.claudeDialog.getByRole('listbox')
  }

  /** The greeting (level 1), as a pattern ("Good afternoon, Sam" by the hour). */
  greeting(pattern) {
    return this.main.getByRole('heading', { level: 1, name: new RegExp(pattern) })
  }

  /** A heading of Today that starts with these words ("Your day", "My work · this week", "A quiet day"). */
  section(words) {
    return this.main.getByRole('heading', { name: new RegExp(`^${literal(words)}`) })
  }

  /** A task of My work's ✦ button, by the task's title. */
  claudeFor(title) {
    return this.page.getByRole('button', { name: `Claude: “${title}”…`, exact: true })
  }

  /** A line of the last Yolo's outcome: what the run says, then a line for each mail draft it wrote. */
  askSays(text) {
    return this.askBar.getByText(text)
  }

  /** A task of My work's Close button: there while the task is listed. */
  closeFor(title) {
    return this.page.getByRole('button', { name: `Close “${title}”…`, exact: true })
  }

  /** A skill the Claude dialog offers, by its name. */
  skill(name) {
    return this.claudeDialog.getByRole('option', { name: new RegExp(`^${literal(name)}\\b`) })
  }

  /** Today, once Needs you now is drawn (its items loaded: ⌘K lists them only then). */
  async open() {
    await this.goto('#today')
    await this.needs.getByRole('listitem').first().waitFor()
  }

  /** An item of Needs you now, as it reads: where it came from and what it says. */
  item(where, says) {
    return this.needs.getByRole('listitem', { name: `${where} — ${says}`, exact: true })
  }

  /** An item's own buttons, by their names (Reply …, ✦). */
  async itemButtons(where, says) {
    const names = []
    for (const b of await this.item(where, says).getByRole('button').all()) names.push(await b.getAttribute('aria-label'))
    return names
  }

  /** ✦ on an item: My work's Claude dialog on it. */
  async claude(where, says) {
    await this.item(where, says).getByRole('button', { name: this.data.text.needs.claude }).click()
  }

  /** Your day: its rows (a meeting or call each, named by its subject) and the line now between them. */
  get yourDay() {
    return this.main.getByRole('region', { name: 'Your day', exact: true })
  }

  /** A line of Your day ("where you'll be"), by its meeting's subject. */
  dayRow(subject) {
    return this.yourDay.getByRole('listitem', { name: subject, exact: true })
  }

  /** A link of a line of Your day (its minutes, its prep); a text anywhere in Your day. */
  dayButton(subject, name) {
    return this.dayRow(subject).getByRole('button', { name, exact: true })
  }

  /** A line's menu (⋯, as a person does: over the line): Calendar's menu for its meeting. */
  async dayMenu(subject) {
    await this.dayRow(subject).hover()
    await this.dayRow(subject).getByRole('button', { name: `${subject} — actions`, exact: true }).click()
    const menu = this.page.getByRole('menu')
    await menu.waitFor()
    return menu
  }

  /** An item of a line's menu, run: "Prepare", "Record when it starts", "Don't record", …. */
  async dayMenuItem(subject, name) {
    await (await this.dayMenu(subject)).getByRole('menuitem', { name, exact: true }).click()
  }

  daySays(text) {
    return this.yourDay.getByText(text)
  }

  get nowLine() {
    return this.yourDay.getByRole('separator', { name: /^Now / })
  }

  /** What Your day lists, in order: each row's subject and the line now ("Now 01:43 PM"). */
  async dayLines() {
    const names = []
    for (const l of await this.yourDay.getByRole('listitem').or(this.nowLine).all()) names.push(await l.getAttribute('aria-label'))
    return names.filter((n) => !n.startsWith('Your work: '))
  }

  /** Your day's rows, by their subjects, in order. */
  async dayRows() {
    const names = []
    for (const l of await this.yourDay.getByRole('listitem').all()) names.push(await l.getAttribute('aria-label'))
    return names.filter((n) => !n.startsWith('Your work: '))
  }

  /** The line now's colour (its time and its rule). */
  async nowColour() {
    return this.nowLine.evaluate((el) => getComputedStyle(el).color)
  }

  /** A task's card in My work, by its title (its name holds its due chip, its title, its pile and its thread). */
  taskCard(title) {
    return this.main.getByRole('button', { name: new RegExp(`^(?!Claude: |Close )(.* )?${literal(title)} `) })
  }

  /** A task's card clicked on its empty area (its corner), not on its title. */
  async openTaskCard(title) {
    await this.taskCard(title).click({ position: { x: 4, y: 4 } })
  }

  /** I'll do it and Won't do, from ⌘K's Needs you now group, by the sender. */
  async illDoIt(sender) {
    await this.action(`${this.data.text.palette.illDoIt}: ${sender}`)
  }

  async wontDo(sender) {
    await this.action(`${this.data.text.palette.wontDo}: ${sender}`)
  }

  // ── A card sent to Today (S-29), by its title: what it says, its button, its heading and list ──

  card(title) {
    return this.main.getByRole('region', { name: title, exact: true })
  }

  cardSays(title, text) {
    return this.card(title).getByText(text, { exact: true })
  }

  cardButton(title, name) {
    return this.card(title).getByRole('button', { name, exact: true })
  }

  cardHeading(title, name) {
    return this.card(title).getByRole('heading', { name, exact: true })
  }

  cardItems(title) {
    return this.card(title).getByRole('listitem')
  }

  /** The words a card draws bold. */
  cardBold(title, text) {
    return this.card(title).getByRole('strong').filter({ hasText: text })
  }

  /** A text of Today, as it reads (the calendar's notice). */
  says(text) {
    return this.main.getByText(text, { exact: true })
  }

  /** Your day's buttons of a name ("Open the call", "Record when it starts"). */
  dayButtons(name) {
    return this.yourDay.getByRole('button', { name, exact: true })
  }
}
