// Today: Needs you now (its items and their I'll do it / Won't do dialogs) and Asks.
import { AppPage } from './app.page.mjs'

export class TodayPage extends AppPage {
  constructor(page, data) {
    super(page, data)
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
  }

  async open() {
    await this.goto('#today')
  }

  /** An item of Needs you now, as it reads: where it came from and what it says. */
  item(where, says) {
    return this.needs.getByRole('listitem', { name: `${where} — ${says}`, exact: true })
  }

  async illDoIt(where, says) {
    await this.item(where, says).getByRole('button', { name: "I'll do it" }).click()
  }

  async wontDo(where, says) {
    await this.item(where, says).getByRole('button', { name: "Won't do" }).click()
  }
}
