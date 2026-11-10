// A meeting still to come (Calls → Coming up today): its header (who is invited, how each answered) and its prep.
import { AppPage } from './app.page.mjs'

export class MeetingPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.answers = page.getByText(/\d+ yes|\d+ maybe|\d+ no answer|\d+ declined/).first()
    this.invited = page.getByRole('list', { name: 'Invited' })
    this.more = this.invited.getByRole('button', { name: /more$/ })
    this.moreList = page.getByRole('list', { name: /more$/ })
    this.prepFooter = page.getByText(/^Written /)
  }

  /** The meeting by its prep id (`manifest.json` → preps). */
  async open(prepId) {
    await this.goto(`#upcoming/${prepId}`)
  }

  /** The people shown in the header, each as "Name, Answer" in their order. */
  async people() {
    return this.invited.getByRole('listitem').evaluateAll((items) => items.filter((x) => !x.closest('[role=button]')).map((x) => x.getAttribute('aria-label')))
  }

  person(name) {
    return this.invited.getByRole('listitem', { name: new RegExp(`^${name},`) })
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

  /** The people "+N more" lists, after hovering it. */
  async morePeople() {
    await this.more.hover()
    return this.moreList.getByRole('listitem').evaluateAll((items) => items.map((x) => x.getAttribute('aria-label')))
  }

  section(name) {
    return this.page.getByRole('heading', { name, level: 3 })
  }
}
