// The Inbox (the right panel's first tab, from the top bar): its chips (Open, Blocking, Done) and what it says.
import { AppPage } from './app.page.mjs'

export class InboxPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    const t = data.text.inbox
    this.t = t
    this.panel = this.rightPanel
    this.tab = this.panelTab(this.panel, data.text.shell.inbox)
    // The count on the Inbox button while something waits ("2 open").
    this.badge = this.topBar.getByLabel(new RegExp(t.badge))
  }

  /** A chip at the panel's top ("Open", "Done", "Blocking"), pressed while it is the one shown. */
  chip(name) {
    return this.panel.getByRole('button', { name, exact: true })
  }

  /** A text of the panel, as it reads. */
  says(text) {
    return this.panel.getByText(text, { exact: true })
  }
}
