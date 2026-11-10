// A meeting still to come (Calls → Coming up today): its header (who is invited, how each answered) and its prep.
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

export class MeetingPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.header = page.getByRole('heading', { level: 2 })
    this.invited = page.getByRole('list', { name: 'Invited' })
    this.more = this.invited.getByRole('button', { name: /^\+\d+ more$/ })
    this.moreList = this.invited.getByRole('list', { name: /^\d+ more$/ })
    this.prepFooter = page.getByText(/^Written /)
    this.prepTab = page.getByRole('tab', { name: 'Prep', exact: true })
    this.prep = page.getByRole('article')
    this.writeAgain = page.getByRole('button', { name: 'Write it again' })
  }

  // ── Its header's threads (+ Thread, a thread's chip and ×), its tabs and the Prep pane before a prep ──

  get addThread() {
    return this.main.getByRole('button', { name: this.data.text.upcoming.addThread, exact: true })
  }

  /** An item of the + Thread menu, by its name ("Globex pilot", "New thread from this meeting…"). */
  threadItem(name) {
    return this.main.getByRole('menu').getByRole('menuitem', { name, exact: true })
  }

  get threadItems() {
    return this.main.getByRole('menu').getByRole('menuitem')
  }

  /** A thread's chip in the header, and its × (named "Take it out of <thread>"). */
  chip(thread) {
    return this.main.getByRole('button', { name: thread, exact: true })
  }

  takeOut(thread) {
    return this.main.getByRole('button', { name: this.data.text.upcoming.takeOut.replace('{thread}', thread), exact: true })
  }

  /** A tab of the meeting ("Prep", "Transcript", "Minutes"). */
  tab(name) {
    return this.main.getByRole('tab', { name, exact: true })
  }

  /** A button of the meeting's page ("Prepare now", "Copy link"), by its name. */
  button(name) {
    return this.main.getByRole('button', { name, exact: true })
  }

  /** A text of the meeting's page, as it reads; or one that starts with these words. */
  says(text) {
    return this.main.getByText(text, { exact: true })
  }

  saysStarting(text) {
    return this.main.getByText(new RegExp(`^${literal(text)}`))
  }

  /** The Prep pane's heading ("No prep yet"). */
  paneHeading(name) {
    return this.main.getByRole('heading', { name, level: 3, exact: true })
  }

  /** The meeting by its prep id (`manifest.json` → preps). */
  async open(prepId) {
    await this.goto(`#upcoming/${prepId}`)
  }

  /** How many answered what, as the date line says it ("4 yes · 2 maybe · …"). */
  answers(text) {
    return this.page.getByText(text, { exact: true })
  }

  /** A person in the header by name: the list item named "Name, Answer". */
  person(name) {
    return this.invited.getByRole('listitem', { name: new RegExp(`^${literal(name)},`) })
  }

  /** The people shown in the header (not those behind "+N more"), as their names read: "Name, Answer". */
  async people() {
    const shown = this.invited.getByRole('listitem', { name: /, / }).filter({ hasNot: this.page.getByRole('list') })
    const names = []
    for (const item of await shown.all()) if (await item.isVisible()) names.push(await item.getAttribute('aria-label'))
    return names
  }

  /** Whether a person's name is drawn bold (the most important in the thread). */
  async isBold(name) {
    return this.person(name).evaluate((el) => Number(getComputedStyle(el).fontWeight) >= 600)
  }

  /** The card a person's hover shows. */
  async hoverCard(name) {
    await this.person(name).hover()
    return this.person(name).getByRole('tooltip')
  }

  /** The people "+N more" lists, while the pointer is on it. */
  async morePeople() {
    await this.more.hover()
    const names = []
    for (const item of await this.moreList.getByRole('listitem').all()) names.push(await item.getAttribute('aria-label'))
    return names
  }

  /** An item the prep lists (Expected from you, Open with these people), by its title. */
  prepItem(title) {
    return this.prep.getByRole('link', { name: title, exact: true })
  }

  /** An item's line in the prep (its link and what follows it, such as its date). */
  prepRow(title) {
    return this.prep.getByRole('listitem').filter({ has: this.page.getByRole('link', { name: title, exact: true }) })
  }

  section(name) {
    return this.page.getByRole('heading', { name, level: 3, exact: true })
  }
}
