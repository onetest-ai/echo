// Settings: its sections (the list on the left) and Layout (where each panel view opens, Reset).
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

export class SettingsPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.sections = this.main.getByRole('complementary')
    this.reset = this.main.getByRole('button', { name: 'Reset', exact: true })
    // Layout's rows: a view's Right and Bottom, in a group named as the view.
    this.layoutRows = this.main.getByRole('group').filter({ has: page.getByRole('button', { name: 'Bottom', exact: true }) })
  }

  async open(route) {
    await this.goto(route)
  }

  /** A section of the list on the left, by its name. */
  section(name) {
    return this.sections.getByRole('button', { name, exact: true })
  }

  /** Layout: a view's place button ("Right", "Bottom"). */
  place(view, where) {
    return this.main.getByRole('group', { name: view, exact: true }).getByRole('button', { name: where, exact: true })
  }

  // ── Search, the sections' controls ──

  get search() {
    return this.sections.getByRole('textbox', { name: 'Search settings' })
  }

  /** The sections the list on the left shows, in order (Health's name carries its warnings: "Health ⚠ 1"). */
  get sectionButtons() {
    return this.sections.getByRole('button')
  }

  /** The settings a search picked out (their titles, marked). */
  get hits() {
    return this.main.getByRole('mark')
  }

  /** A group of buttons, by its name ("Appearance", "Calendar meetings"), and one of its buttons. */
  choice(group, name) {
    return this.main.getByRole('group', { name: group, exact: true }).getByRole('button', { name, exact: true })
  }

  /** A switch, a select, a text box, a button, a text, by its name. */
  switch(name) {
    return this.main.getByRole('switch', { name, exact: true })
  }

  select(name) {
    return this.page.getByRole('combobox', { name, exact: true })
  }

  box(name) {
    return this.main.getByRole('textbox', { name, exact: true })
  }

  button(name) {
    return this.main.getByRole('button', { name, exact: true })
  }

  says(text) {
    return this.main.getByText(text, { exact: true })
  }

  /** A select's chosen option. */
  chosen(select, text) {
    return this.select(select).getByRole('option', { name: text, exact: true, selected: true })
  }

  // ── Templates ──

  tab(name) {
    return this.main.getByRole('tab', { name: new RegExp(`^${literal(name)}`) })
  }

  /** A row of a list (a template, a meeting type), by its name; it reads "<name> <tag> <use>". */
  row(name) {
    return this.main.getByRole('button', { name: new RegExp(`^${literal(name)} (Built in|Edited|Yours) `) })
  }

  /** The editor of a template or type, by its title ("Status sync", "New template"). */
  editor(title) {
    return this.main.getByRole('region', { name: title, exact: true })
  }

  /** A section's heading boxes in an editor, in order. */
  headings(title) {
    return this.editor(title).getByRole('textbox', { name: 'Heading', exact: true })
  }

  // ── The vocabulary panel ──

  /** A learned term's row in the panel (its Always and Remove, its line). */
  learned(term) {
    return this.rightPanel.getByRole('group', { name: term, exact: true })
  }
}
