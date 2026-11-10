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

  /** The colour of a meeting's left edge in the grid (green when recorded, orange when set to record). */
  async edge(subject) {
    return this.event(subject).evaluate((el) => getComputedStyle(el).borderLeftColor)
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

  /** The grid scrolled by the wheel, over a day's column. */
  async scrollGrid(column, dy) {
    const box = await column.boundingBox()
    await this.page.mouse.move(box.x + 4, Math.max(box.y, 0) + 200)
    await this.page.mouse.wheel(0, dy)
  }

  /** Where a meeting's card is: inside the window, beside its meeting (right or left of it, apart), level with it as
   *  far as the window lets it (12 px from its edges), with its main button and Prepare on one line. */
  async cardPlace(subject, record, prepare) {
    const card = await this.card(subject).boundingBox()
    const ev = await this.event(subject).boundingBox()
    const a = await this.cardButton(subject, record).boundingBox()
    const b = await this.cardButton(subject, prepare).boundingBox()
    const { width, height } = this.page.viewportSize()
    const level = Math.min(Math.max(12, ev.y), Math.max(12, height - card.height - 12))
    return {
      inside: card.x >= 0 && card.y >= 0 && card.x + card.width <= width && card.y + card.height <= height,
      beside: card.x >= ev.x + ev.width || card.x + card.width <= ev.x,
      level: Math.abs(card.y - level) <= 1,
      oneLine: Math.abs(a.y + a.height / 2 - (b.y + b.height / 2)) <= 2,
    }
  }
}
