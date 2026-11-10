// The Vocabulary panel (opened from the book beside the Calls list): Every call's terms, the box that adds one, and
// the terms learned from calls, each with its Always and Remove, and their filter past six.
import { AppPage } from './app.page.mjs'
import { fill } from '../support/data.mjs'

export class VocabularyPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    const v = data.text.vocabulary
    this.v = v
    this.panel = this.rightPanel
    this.book = page.getByRole('button', { name: v.panel, exact: true })
    this.tab = this.panelTab(this.panel, v.panel)
    this.addBox = this.panel.getByRole('textbox', { name: v.add })
    this.addButton = this.panel.getByRole('button', { name: v.addButton, exact: true })
    this.everyCall = this.panel.getByRole('heading', { name: v.every, exact: true })
    this.filter = this.panel.getByRole('textbox', { name: v.filter })
    // The learned terms' rows, in their order.
    this.learnedRows = this.panel.getByRole('group')
  }

  /** Calls, freshly loaded, and the panel opened from the book. */
  async open() {
    await this.goto(this.data.text.shell.routes.calls)
    await this.book.click()
  }

  /** A term added to Every call: Enter in the box, or its Add button. */
  async add(term, how = 'enter') {
    await this.addBox.fill(term)
    if (how === 'enter') await this.addBox.press(this.data.text.shell.keys.enter)
    else await this.addButton.click()
  }

  /** A term of Every call: its chip's × button. */
  chip(term) {
    return this.panel.getByRole('button', { name: fill(this.v.removeTerm, { term }), exact: true })
  }

  /** "Learned from calls" with its count. */
  learnedHead(n) {
    return this.panel.getByRole('heading', { name: `${this.v.learned} ${n}`, exact: true })
  }

  /** A learned term's row (its Always and Remove, its line), and one of its buttons. */
  learned(term) {
    return this.panel.getByRole('group', { name: term, exact: true })
  }

  learnedButton(term, name) {
    return this.learned(term).getByRole('button', { name, exact: true })
  }

  /** A text of the panel, as it reads. */
  says(text) {
    return this.panel.getByText(text, { exact: true })
  }
}
