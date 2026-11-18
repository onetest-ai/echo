// A plugin's routines on its install or update sheet and on the sheet its setup opens (`ctx.echo.routines.offer`): a
// switch each in Echo's words, off unless the user switches it on, only what the user moved sent (app.js, the plugin
// routine switches block). Run by scripts/check-mod.sh (`node crates/echoctl/ui/plugin-routines.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── plugin routine switches:')
const end = src.indexOf('// end of the plugin routine switches block', start)
assert.ok(start >= 0 && end > start, 'the plugin routine switches block not found in app.js')
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
const names = 'plRoutineSwitches, plSheetLines, plRoutinesStart, plRoutinesChanged, routineOfferPick, routineOfferHtml, routineOfferResult'
const B = new Function('esc', `${src.slice(start, end)}\nreturn { ${names} }`)(esc)

const tests = []
const test = (name, f) => tests.push([name, f])
const mail = { routine: 'mail-knowledge', name: 'Mail → Knowledge', what: 'Files trusted mail into your vault', schedule: 'Every hour · paused at night', on: false, line: 'Has a routine “Mail → Knowledge”, long line', facts: ['acts on its own as you set routines', 'may change your Echo data and send as you', 'auto mode', 'prompt 1a2b3c4d'] }
const vault = { routine: 'vault-health', name: 'Vault health', what: 'Checks links and stale pages', schedule: 'Every day', on: false, line: 'Has a routine “Vault health”, long line', facts: ['no tools: it can only answer', 'prompt 5e6f7a8b'] }

test('the sheet: a switch per routine, off as it opens, with its name, when it runs and what it does', () => {
  const on = B.plRoutinesStart([mail, vault])
  assert.deepEqual(on, { 'mail-knowledge': false, 'vault-health': false })
  const html = B.plRoutineSwitches([mail, vault], on, 'data-pl-sheet-rt')
  assert.ok(html.includes('Routines — off unless you switch them on'))
  assert.equal((html.match(/role="switch"/g) || []).length, 2)
  assert.ok(html.includes('aria-checked="false" aria-label="Mail → Knowledge on" data-pl-sheet-rt="mail-knowledge" data-val="true"'), html)
  for (const t of ['Every hour · paused at night', 'Every day', 'Files trusted mail into your vault', 'Vault health', 'Checks links and stale pages']) assert.ok(html.includes(t), t)
  // Its facts sit under its switch, muted, on one line; no long sentence is repeated.
  assert.ok(html.includes('acts on its own as you set routines · may change your Echo data and send as you · auto mode · prompt 1a2b3c4d'), html)
  assert.ok(html.includes('no tools: it can only answer · prompt 5e6f7a8b'), html)
  assert.ok(!html.includes('1440 min') && !html.includes('long line'), html)
  // Switched on: drawn on, its click sends off.
  const on2 = B.plRoutineSwitches([vault], { 'vault-health': true }, 'data-x')
  assert.ok(on2.includes('class="sw on" role="switch" aria-checked="true"') && on2.includes('data-val="false"'), on2)
  assert.equal(B.plRoutineSwitches([], {}, 'data-x'), '', 'no routines: no block')
  assert.equal(B.plRoutineSwitches(undefined, {}, 'data-x'), '')
})

test('the sheet lists each routine once: its long line leaves "It will be able to", the others stay', () => {
  const r = { uses: ['Reads your calls', mail.line, vault.line], new_uses: [vault.line, 'Sends mail'], routines: [mail, vault] }
  assert.deepEqual(B.plSheetLines(r, false), ['Reads your calls'])
  assert.deepEqual(B.plSheetLines(r, true), ['Sends mail'])
  assert.deepEqual(B.plSheetLines({ uses: ['a'] }, false), ['a'], 'no routines: as the review has it')
})

test("a plugin's words are text, never markup", () => {
  const bad = { routine: 'x"><b', name: '<img src=x onerror=alert(1)>', what: '</div><script>1</script>', schedule: '"', on: false }
  const html = B.plRoutineSwitches([bad], {}, 'data-x') + B.routineOfferHtml('<i>Acme</i>', [bad], {})
  assert.ok(!/<img|<script|<i>/.test(html), html)
  assert.ok(html.includes('data-x="x&quot;&gt;&lt;b"'))
})

test('only what the user moved is sent: a switch left as Echo has it is not', () => {
  assert.deepEqual(B.plRoutinesChanged([mail, vault], B.plRoutinesStart([mail, vault])), {}, 'nothing touched: nothing sent')
  assert.deepEqual(B.plRoutinesChanged([mail, vault], { 'mail-knowledge': false, 'vault-health': true }), { 'vault-health': true })
  // An update's changed routine the user had on: on as the sheet opens; switched off, sent off.
  const kept = { ...vault, on: true }
  assert.deepEqual(B.plRoutinesStart([kept]), { 'vault-health': true })
  assert.deepEqual(B.plRoutinesChanged([kept], { 'vault-health': true }), {})
  assert.deepEqual(B.plRoutinesChanged([kept], { 'vault-health': false }), { 'vault-health': false })
  assert.deepEqual(B.plRoutinesChanged(undefined, {}), {})
})

test("the offer: the plugin's routines it names, in its order, or all; an id it lacks or none at all throws", () => {
  assert.deepEqual(B.routineOfferPick([mail, vault]).map((r) => r.routine), ['mail-knowledge', 'vault-health'])
  assert.deepEqual(B.routineOfferPick([mail, vault], ['vault-health']).map((r) => r.routine), ['vault-health'])
  assert.deepEqual(B.routineOfferPick([mail, vault], 'mail-knowledge').map((r) => r.routine), ['mail-knowledge'])
  assert.deepEqual(B.routineOfferPick([mail, vault], ['vault-health', 'mail-knowledge']).map((r) => r.routine), ['mail-knowledge', 'vault-health'])
  assert.throws(() => B.routineOfferPick([mail], ['nope']), /the plugin has no routine “nope”/)
  assert.throws(() => B.routineOfferPick([mail], []), /no routine named/)
  assert.throws(() => B.routineOfferPick([], null), /the plugin has no routines/)
  assert.throws(() => B.routineOfferPick(undefined), /the plugin has no routines/)
})

test("the offer's sheet: Echo's words and switches, a Done; it resolves to each routine as Echo has it then", () => {
  const html = B.routineOfferHtml('Knowledge', [mail], { 'mail-knowledge': true })
  assert.ok(html.includes('Knowledge’s routines') && html.includes('data-rt-offer="mail-knowledge"') && html.includes('data-rt-offer-done'))
  assert.ok(html.includes('aria-checked="true"'))
  assert.deepEqual(B.routineOfferResult([mail, vault], [{ ...mail, on: true }, { ...vault, on: false }]), [
    { id: 'mail-knowledge', name: 'Mail → Knowledge', on: true }, { id: 'vault-health', name: 'Vault health', on: false },
  ])
  assert.deepEqual(B.routineOfferResult([mail], null), [{ id: 'mail-knowledge', name: 'Mail → Knowledge', on: false }], 'gone meanwhile: off')
})

test("only the user's own click moves a switch; the offer asks for the click before anything", () => {
  assert.ok(/if \(d\.plSheetRt != null\) \{\s*if \(!e\.isTrusted/.test(src), "the sheet's switch: a script's click is ignored")
  assert.ok(/if \(!b \|\| !o\.box\.contains\(b\) \|\| !e\.isTrusted\) return/.test(src), "the offer's switch: a script's click is ignored")
  assert.ok(/offer: async \(ids\) => \{\s*if \(!copyAllowed\(/.test(src), 'ctx.echo.routines.offer asks copyAllowed first')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n${e.stack}`) }
}
if (failed) process.exit(1)
