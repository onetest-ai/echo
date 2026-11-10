// Settings → Plugins: its tabs (Installed, Discover, Marketplaces), the installed plugins (a row, the detail on the
// right), Discover's chips and cards, the install sheet, the marketplaces (a row each, its ⋯ menu) and the Add form.
import { SettingsPage } from './settings.page.mjs'
import { literal } from '../support/data.mjs'

export class PluginsPage extends SettingsPage {
  constructor(page, data) {
    super(page, data)
    const t = data.text.plugins
    this.t = t
    this.pluginSearch = this.main.getByRole('textbox', { name: t.search, exact: true })
    this.categories = this.main.getByRole('group', { name: 'Categories', exact: true })
    this.sheet = page.getByRole('dialog')
    this.menu = this.main.getByRole('menu')
    this.source = this.main.getByRole('textbox', { name: t.source, exact: true })
    this.marketName = this.main.getByRole('textbox', { name: t.name, exact: true })
    this.addMarket = this.main.getByRole('button', { name: t.addButton, exact: true })
  }

  /** Settings → Plugins, on a tab when one is named. */
  async openPlugins(tab) {
    await this.open(this.t.route)
    await this.main.getByRole('heading', { name: this.t.heading, level: 2, exact: true }).waitFor()
    if (tab) await this.pluginTab(tab).click()
  }

  /** A tab, by its name (its count follows it in its name: "Marketplaces1"). */
  pluginTab(name) {
    return this.main.getByRole('tab', { name: new RegExp(`^${literal(name)}\\d*$`) })
  }

  get headingPlugins() {
    return this.main.getByRole('heading', { name: this.t.heading, level: 2, exact: true })
  }

  /** Discover's category chips, in order; one by its name ("All", "Automation"). */
  get chips() {
    return this.categories.getByRole('button')
  }

  /** The open ⋯ menu's items. */
  get menuItems() {
    return this.menu.getByRole('menuitem')
  }

  /** A category chip of Discover ("All", "Automation"). */
  chip(name) {
    return this.categories.getByRole('button', { name, exact: true })
  }

  /** A card of Discover, by the plugin's name. */
  offer(name) {
    return this.main.getByRole('group', { name, exact: true }).filter({ hasNot: this.page.getByRole('switch') })
  }

  /** Discover's cards (its other group, the chips, left out). */
  get offers() {
    return this.main.getByRole('group', { name: /^(?!Categories$)./ }).filter({ hasNot: this.page.getByRole('switch') })
  }

  /** An installed plugin's row, and its detail on the right. */
  installedRow(name) {
    return this.main.getByRole('group', { name, exact: true }).filter({ has: this.page.getByRole('switch') })
  }

  /** An installed row's lines (its name and version, its summary), as drawn: dimmed while the plugin is off. */
  rowLines(name, version, summary) {
    return this.installedRow(name).getByText(new RegExp(`^${literal(name)} ${literal(version)}\\s*${literal(summary)}$`))
  }

  detail(name) {
    return this.main.getByRole('region', { name, exact: true })
  }

  /** A setting of the plugin's own, in its detail: a box, a switch. */
  field(plugin, label) {
    return this.detail(plugin).getByRole('textbox', { name: label, exact: true })
  }

  fieldSwitch(plugin, label) {
    return this.detail(plugin).getByRole('switch', { name: label, exact: true })
  }

  /** A marketplace's row, by its name. */
  market(name) {
    return this.main.getByRole('group', { name, exact: true })
  }

  /** An item of the open ⋯ menu. */
  menuItem(name) {
    return this.menu.getByRole('menuitem', { name, exact: true })
  }

  /** A button or a switch of an area (a card, a row, the detail, the sheet), by its name. */
  buttonIn(where, name) {
    return where.getByRole('button', { name, exact: true })
  }

  switchOf(where, name) {
    return where.getByRole('switch', { name, exact: true })
  }

  /** A text of an area, as it reads; or one it contains. */
  in(where, text) {
    return where.getByText(text, { exact: true })
  }

  inPart(where, text) {
    return where.getByText(text)
  }
}
