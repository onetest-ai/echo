// The Calendar: Day and Week, moving by a week and back to Today, its day columns (today's marked), the line now, the
// meetings drawn in the grid (recorded, set to record) and a meeting's card beside it.
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

export class CalendarPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.title = this.main.getByRole('heading', { level: 1 })
    this.views = this.main.getByRole('group', { name: 'View', exact: true })
    this.earlier = this.main.getByRole('button', { name: 'Earlier', exact: true })
    this.later = this.main.getByRole('button', { name: 'Later', exact: true })
    this.today = this.main.getByRole('button', { name: 'Today', exact: true })
    // The grid's day columns, each named by its day ("Tue 10").
    this.columns = this.main.getByRole('group', { name: /^\S+ \d+$/ })
  }

  /** The calendar on Day or Week (`day`, `week`). */
  async open(mode) {
    await this.goto(`#calendar/${mode}`)
  }

  /** Day or Week, pressed while it is the one shown. */
  view(name) {
    return this.views.getByRole('button', { name, exact: true })
  }

  /** A day's column, by its name ("Tue 10"). */
  column(name) {
    return this.main.getByRole('group', { name, exact: true })
  }

  /** A day's column name as the grid writes it ("Tue 10"): today, or that many days from it. */
  async columnName(days = 0) {
    return this.page.evaluate((n) => {
      const d = new Date()
      d.setHours(12, 0, 0, 0)
      d.setDate(d.getDate() + n)
      return `${d.toLocaleDateString([], { weekday: 'short' })} ${d.getDate()}`
    }, days)
  }

  async columnNames() {
    const names = []
    for (const c of await this.columns.all()) names.push(await c.getAttribute('aria-label'))
    return names
  }

  /** A meeting in the grid, by its subject (its name reads "<subject> <time> …"). */
  event(subject, column) {
    return (column || this.main).getByRole('button', { name: new RegExp(`^${literal(subject)} `) })
  }

  /** The line now in a day's column. */
  now(column) {
    return column.getByRole('separator', { name: 'Now', exact: true })
  }

  /** A text of the view, as it reads (the notice). */
  says(text) {
    return this.main.getByText(text, { exact: true })
  }

  /** A meeting's card, named by its subject, and a button of it ("Close", "Record when it starts"). */
  card(subject) {
    return this.main.getByRole('complementary', { name: subject, exact: true })
  }

  cardButton(subject, name) {
    return this.card(subject).getByRole('button', { name, exact: true })
  }

  /** Close the open meeting card, if any, as a person does before opening another: since #352 a card sits inside the
   *  grid and can cover the next meeting. */
  async closeCard() {
    const open = this.main.getByRole('complementary')
    if (!(await open.count())) return
    await open.first().getByRole('button', { name: this.data.text.cal.close, exact: true }).click()
    await open.first().waitFor({ state: 'detached' })
  }

  /** Open the card of a meeting of a series (of the meetings of that subject, the one whose card offers the series'
   *  choice "Record every meeting"). */
  async openSeries(subject) {
    await this.event(subject).first().waitFor()
    for (const ev of await this.event(subject).all()) {
      await ev.click()
      await this.cardHeading(subject).waitFor()
      if (await this.seriesChoice(subject).waitFor({ timeout: 2_000 }).then(() => true, () => false)) return
    }
    throw new Error(`no meeting "${subject}" of a series`)
  }

  /** Open the card of the meeting of that subject whose card offers this button (of several of one subject: the past
   *  one not recorded offers "Import Teams transcript"). */
  async openWith(subject, button) {
    await this.event(subject).first().waitFor()
    for (const ev of await this.event(subject).all()) {
      await this.closeCard()
      await ev.click()
      await this.cardHeading(subject).waitFor()
      if (await this.cardButton(subject, button).waitFor({ timeout: 2_000 }).then(() => true, () => false)) return
    }
    throw new Error(`no meeting "${subject}" whose card offers ${button}`)
  }

  /** A series' choice in a card (its group "Record every meeting"), a button of it ("Record", "Ask", "Don't record"). */
  seriesChoice(subject) {
    return this.card(subject).getByRole('group', { name: 'Record every meeting', exact: true })
  }

  seriesButton(subject, name) {
    return this.seriesChoice(subject).getByRole('button', { name, exact: true })
  }

  /** A select's chosen option in a card, by its text. */
  cardChosen(subject, select, text) {
    return this.cardSelect(subject, select).getByRole('option', { name: text, exact: true, selected: true })
  }

  /** The card's line of when, which starts with these words ("Today · 02:42 PM–03:12 PM · in 58 min"), as it reads. */
  async cardWhen(subject, words) {
    return (await this.cardStarting(subject, words).innerText()).replace(/\s+/g, ' ').trim()
  }

  /** The card's heading, a select of it ("Kind of meeting"), a text of it as it reads, or one that starts so. */
  cardHeading(subject) {
    return this.card(subject).getByRole('heading', { name: subject, exact: true })
  }

  cardSelect(subject, name) {
    return this.card(subject).getByRole('combobox', { name, exact: true })
  }

  cardSays(subject, text) {
    return this.card(subject).getByText(text, { exact: true })
  }

  cardStarting(subject, words) {
    return this.card(subject).getByText(new RegExp(`^${literal(words)}`))
  }

  /** The meetings in the grid whose time line names their app (a Zoom or Google Meet invite: "… 02:30 PM Zoom"). */
  appTagged(apps) {
    return this.main.getByRole('button', { name: new RegExp(` (${apps.map(literal).join('|')})$`) })
  }

  /** A meeting's menu (a right-click on it): the items it holds, in order. */
  async menuOf(subject) {
    await this.event(subject).first().click({ button: 'right' })
    const menu = this.page.getByRole('menu')
    await menu.waitFor()
    const out = []
    for (const item of await menu.getByRole('menuitem').all()) out.push((await item.innerText()).trim())
    return out
  }

  /** The colour of a meeting's left edge in the grid (green when recorded, orange when set to record). */
  async edge(subject) {
    return this.event(subject).evaluate((el) => getComputedStyle(el).borderLeftColor)
  }

  /** The left edge's colour of the meeting whose card is open (of several of one subject: the one selected). */
  async openEdge(subject) {
    return this.event(subject).and(this.page.locator('[aria-selected="true"]')).evaluate((el) => getComputedStyle(el).borderLeftColor)
  }

  /** The line now's colour and its dot's, in a day's column. */
  async nowColours(column) {
    return this.now(column).evaluate((el) => ({ line: getComputedStyle(el).borderTopColor, dot: getComputedStyle(el, '::before').backgroundColor }))
  }

  /** Whether a column is shaded (today's). */
  async shaded(column) {
    return column.evaluate((el) => getComputedStyle(el).backgroundColor !== 'rgba(0, 0, 0, 0)')
  }

  /** Where the line now sits: below one meeting's top and above the next one's. */
  async nowBetween(column, before, after) {
    const line = await this.now(column).boundingBox()
    const a = await this.event(before, column).boundingBox()
    const b = await this.event(after, column).boundingBox()
    return a.y < line.y && line.y < b.y
  }

  /** How far down the window a meeting is drawn (it moves as the grid scrolls). */
  async top(subject) {
    return (await this.event(subject).boundingBox()).y
  }

  /** The grid scrolled by the wheel, over a day's column: down by `dy`, or up when it is already at its end (late in
   *  the day the grid opens scrolled to the bottom, so a scroll down moves nothing). */
  async scrollGrid(column, dy) {
    const box = await column.boundingBox()
    await this.page.mouse.move(box.x + 4, Math.max(box.y, 0) + 200)
    const down = await column.evaluate((el) => {
      for (let p = el.parentElement; p; p = p.parentElement) {
        if (p.scrollHeight > p.clientHeight + 1 && getComputedStyle(p).overflowY !== 'visible') return p.scrollTop + p.clientHeight < p.scrollHeight - 1
      }
      return true
    })
    await this.page.mouse.wheel(0, down ? dy : -dy)
  }

  /** Where a meeting's card is: inside the window, beside its meeting (right or left of it, apart), level with it as
   *  far as the window lets it (12 px from its edges), with its main button and Prepare on one line. */
  async cardPlace(subject, record, prepare) {
    const card = await this.card(subject).boundingBox()
    const ev = await this.event(subject).boundingBox()
    const a = await this.cardButton(subject, record).boundingBox()
    const b = await this.cardButton(subject, prepare).boundingBox()
    // Inside the calendar's grid (#338), level with its meeting as far as the grid allows.
    const g = await this.page.locator('#calgrid').boundingBox()
    const { height } = this.page.viewportSize()
    const bottom = Math.min(g.y + g.height, height)
    const level = Math.min(Math.max(g.y + 12, ev.y), Math.max(g.y + 12, bottom - card.height - 12))
    return {
      inside: card.x >= g.x && card.y >= g.y && card.x + card.width <= g.x + g.width && card.y + card.height <= bottom,
      beside: card.x >= ev.x + ev.width || card.x + card.width <= ev.x,
      level: Math.abs(card.y - level) <= 1,
      oneLine: Math.abs(a.y + a.height / 2 - (b.y + b.height / 2)) <= 2,
    }
  }
}
