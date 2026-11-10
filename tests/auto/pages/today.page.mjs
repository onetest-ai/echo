// Today: Needs you now (its items; ✦ and My work's Claude dialog; I'll do it and Won't do from ⌘K) and Asks.
import { AppPage } from './app.page.mjs'

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
    // The last Yolo's outcome, above My work.
    this.askBar = page.getByRole('status', { name: 'Yolo' })
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
