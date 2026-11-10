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

  /** An item of Needs you now, by where it came from: "Mail to you", "Chat with Dana Lee". */
  item(where) {
    return this.needs.getByRole('listitem', { name: where })
  }

  async illDoIt(where) {
    await this.item(where).getByRole('button', { name: "I'll do it" }).click()
  }

  async wontDo(where) {
    await this.item(where).getByRole('button', { name: "Won't do" }).click()
  }
}
