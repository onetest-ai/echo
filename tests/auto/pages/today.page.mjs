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

  /** I'll do it and Won't do, from ⌘K's Needs you now group, by the sender. */
  async illDoIt(sender) {
    await this.action(`${this.data.text.palette.illDoIt}: ${sender}`)
  }

  async wontDo(sender) {
    await this.action(`${this.data.text.palette.wontDo}: ${sender}`)
  }
}
