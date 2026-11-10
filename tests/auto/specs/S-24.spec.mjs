// S-24 The calendar, preps and the Inbox on a busy day — its stages with fixed checks (standard data).
import { test, expect } from '@playwright/test'
import { useData } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { MeetingPage } from '../pages/meeting.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'

const data = useData(test, { set: 'standard' })

scenario(test, 'S-24', 'The calendar, preps and the Inbox on a busy day', () => {
  test("13 · A meeting's header shows each person's answer, the most important first, who declined last; a hover and +N more tell the rest", covers('prep/TC-008', 'F-PREP-02'), async ({ page }) => {
    const m = data.set.meetings.globexWeekly
    const meeting = new MeetingPage(page, data)
    await meeting.open(m.prep)
    await expect(meeting.answers(m.answers)).toBeVisible()
    expect(await meeting.people()).toEqual(m.people)
    expect(await meeting.isBold(m.key)).toBe(true)
    expect(await meeting.isBold(m.notKey)).toBe(false)
    await expect(meeting.person(m.newcomer)).toContainText('new')
    await expect(meeting.person(m.key)).not.toContainText('new')
    const card = await meeting.hoverCard(m.key)
    for (const line of m.card) await expect(card).toContainText(line)
    expect(await meeting.morePeople()).toEqual(m.more)
  })

  test("14 · A prep reads in one column, with no Who's coming", covers('prep/TC-009', 'F-PREP-04'), async ({ page }) => {
    const meeting = new MeetingPage(page, data)
    await meeting.open(data.set.meetings.sponsorUpdate.prep)
    await expect(meeting.section(data.text.prep.inShort)).toBeVisible()
    await expect(meeting.section(data.text.prep.whosComing)).toHaveCount(0)
    await expect(meeting.prepFooter).toBeVisible()
  })

  test("36 · Won't do needs a comment; Cancel and Escape keep the item and decide nothing", covers('today/TC-007', 'F-TODAY-04'), async ({ page }) => {
    const { chat, chatLine } = data.set.needs
    const today = new TodayPage(page, data)
    await today.open()
    await today.wontDo(chat, chatLine)
    await expect(today.wontConfirm).toBeDisabled()
    await today.wontComment.fill('   ')
    await expect(today.wontConfirm).toBeDisabled()
    await today.wontComment.fill('x')
    await expect(today.wontConfirm).toBeEnabled()
    await today.wontCancel.click()
    await expect(today.wontDialog).toBeHidden()
    await today.wontDo(chat, chatLine)
    await page.keyboard.press('Escape')
    await expect(today.wontDialog).toBeHidden()
    await expect(today.item(chat, chatLine)).toBeVisible()
    await expect(today.asks).toHaveCount(0)
    expect(data.json(data.home, data.text.needs.file).decided || {}).toEqual({})
  })

  // Not a stage of the manual S-24 (it takes an item its Won't do stages need); here it runs last, after them.
  test("after · I'll do it adds the task in the chosen thread; the dialog keeps what was typed while Today is drawn again", covers('today/TC-013', 'F-TODAY-04'), async ({ page }) => {
    const { needs: n, task: t } = data.set
    const today = new TodayPage(page, data)
    await today.installClock()
    await today.open()
    await today.illDoIt(n.mail, n.mailTitle)
    await expect(today.taskTitle).toHaveValue(n.mailTitle)
    await today.taskTitle.fill(t.title)
    await today.taskThread.selectOption({ label: t.thread.name })
    // A minute on, Today is drawn again (its line "now" moves): its content is replaced, the dialog isn't.
    const content = await today.main.elementHandle()
    const button = await today.addToTasks.elementHandle()
    await today.minutePasses()
    await expect.poll(() => content.evaluate((el) => el.isConnected)).toBe(false)
    expect(await button.evaluate((el) => el.isConnected)).toBe(true)
    await expect(today.taskTitle).toHaveValue(t.title)
    await expect(today.taskThread).toHaveValue(t.thread.id)
    await today.addToTasks.click()
    await expect(today.taskDialog).toBeHidden()
    await page.reload()
    await expect(today.item(n.chat, n.chatLine)).toBeVisible()
    await expect(today.item(n.mail, n.mailTitle)).toHaveCount(0)
    const description = data.text.needs.taskDescription.replace('{from}', n.mailFrom).replace('{where}', n.mail).replace('{title}', n.mailTitle)
    expect(data.lastEvent(t.thread.id)).toMatchObject({ type: 'create', kind: 'task', fields: { title: t.title, description } })
  })
})
