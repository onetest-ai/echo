// How Send to Claude ranks the skills for a task (app.js, rankSkills). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/skill-fit.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const src = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const start = src.indexOf('// ── skill fit:')
const end = src.indexOf('// end of the skill fit block', start)
assert.ok(start >= 0 && end > start, 'the skill fit block not found in app.js')
const { rankSkills } = new Function(`${src.slice(start, end)}\nreturn { rankSkills }`)()

const SKILLS = [
  { id: 'follow-up-mail', description: 'Use when a task is to write a follow-up mail after a meeting — thanks, what was agreed, who does what by when, the next step — and send it once the user agrees.' },
  { id: 'status-report', description: 'Use when a task is to send a client the weekly status — done, next, risks, asks.' },
  { id: 'zvit', description: 'Use when a task is a звіт для клієнта — підсумки тижня.' },
]
const top = (text, used) => rankSkills(SKILLS, text, used).map((r) => [r.x.id, r.fit])

// The task's words in a skill's name and description, stemmed.
assert.equal(top('Send the weekly status reports to Acme')[0][0], 'status-report')
assert.equal(top('Follow up with Dana after the workshop')[0][0], 'follow-up-mail')
// Common words make no fit: "send" alone isn't a reason to pick a skill.
assert.ok(top('Send the pricing deck to Acme').every(([, fit]) => fit === 0), JSON.stringify(top('Send the pricing deck to Acme')))
// Any alphabet, Ukrainian letters included.
assert.equal(top('Підготувати звіт для клієнта')[0][0], 'zvit')
// Equal fit: the one chosen more often first.
assert.deepEqual(top('Pricing deck', { 'status-report': 3 }).map(([id]) => id)[0], 'status-report')
console.log('skill-fit: ok')
