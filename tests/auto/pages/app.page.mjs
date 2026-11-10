// The Echo window as a whole: opening it on the data, and what every view shares (the status bar).
export class AppPage {
  /** @param {import('@playwright/test').Page} page  @param data the spec file's data (support/data.mjs) */
  constructor(page, data) {
    this.page = page
    this.data = data
    this.statusBar = page.getByRole('contentinfo')
  }

  /** A button of the status bar, by the word its name starts with ("Actions", "Claude"). */
  statusBarButton(word) {
    return this.statusBar.getByRole('button', { name: new RegExp(`(^|\\s)${word}\\b`) })
  }

  /** The window at a route (`#today`, `#calls`, …): its cookie first, as a browser gets it. */
  async goto(hash = '') {
    await this.page.goto(this.data.base + '/')
    await this.page.goto(this.data.base + '/' + hash)
  }

  /** The view's own content (the primary area's main element). */
  get main() {
    return this.page.getByRole('main').first()
  }

  /** Let a minute pass on the page's clock (after `installClock`): timers fire, the view's minute-based lines move. */
  async installClock() {
    await this.page.clock.install()
  }

  async minutePasses() {
    await this.page.clock.fastForward('01:05')
  }
}
