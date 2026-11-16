// The actions palette (⌘K): its box, its target chip, its groups and their items.
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

export class PalettePage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.dialog = page.getByRole('dialog', { name: 'Actions' })
    this.box = this.dialog.getByRole('textbox', { name: 'Action' })
    this.targetChip = this.dialog.getByRole('button', { name: /^On: / })
    this.selected = this.dialog.getByRole('option', { selected: true })
    this.first = this.dialog.getByRole('option').first()
  }

  /** The target chip, as it reads ("On: Today ×"). */
  target(text) {
    return this.dialog.getByRole('button', { name: text, exact: true })
  }

  /** A group of items, by its name ("This call", "Go to"). */
  group(name) {
    return this.dialog.getByRole('group', { name, exact: true })
  }

  /** The groups, in their order. */
  groups() {
    return this.dialog.getByRole('group')
  }

  /** An item, by its label (its hint, after it, is part of its name), in a group or anywhere. */
  item(label, group) {
    return (group ? this.group(group) : this.dialog).getByRole('option', { name: new RegExp(`^${literal(label)}( |$)`) })
  }

  /** The items of a group, in their order. */
  items(group) {
    return this.group(group).getByRole('option')
  }

  async open(key) {
    await this.page.keyboard.press(key)
    await this.dialog.waitFor()
  }
}
