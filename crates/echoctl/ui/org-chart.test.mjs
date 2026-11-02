// The org chart's layout (app.js, OrgLayout): who sits in the governance band, which workstream column
// each person is in, who leads it. Run by scripts/check-mod.sh (`node crates/echoctl/ui/org-chart.test.mjs`):
// the block is taken from app.js as it ships. Fictions only.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const src = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const start = src.indexOf('// ── org chart layout')
const end = src.indexOf('// end of the org chart layout block', start)
assert.ok(start >= 0 && end > start, 'the org chart layout block not found in app.js')
const { OrgLayout } = new Function(`${src.slice(start, end)}\nreturn { OrgLayout }`)()

const P = (key, role, extra = {}) => ({ key, name: key, place: { role, ...extra } })
const place = (x) => x.place
const hi = { influence: 3, interest: 3 }
const people = [
  P('ann', 'CX stream lead: project charter, sales interviews', hi),
  P('ben', 'Technology stream lead: data platform PoC, access', hi),
  P('pam', 'Project manager: runs the daily sync, owns the plan', hi),
  P('hal', 'Client partner for Globex', hi),
  P('cy', 'CX research: interview guide'),
  P('di', 'Design (CX)'),
  P('ed', 'Automation: as-is notifications'),
  P('fay', 'Automation: gap analysis across journeys'),
  P('gus', 'Engineering', { group: 'Technology' }),
  P('ivy', 'Engineering'),
]
const lay = OrgLayout.assign(people, place)
const at = (k) => lay.get(k)

// A stream's lead leads its column, at 3/3 too; the PM and the partner, with no stream, are the band.
assert.deepEqual([at('ann').stream, at('ann').lead, at('ann').band], ['CX', true, false])
assert.deepEqual([at('ben').stream, at('ben').lead, at('ben').band], ['Technology', true, false], 'Technology keeps a column and its lead')
assert.deepEqual([at('pam').band, at('pam').stream], [true, ''])
assert.equal(at('hal').band, true)
// Members by what their role names; a shared "<X>:" prefix is a stream.
assert.deepEqual(['cy', 'di'].map((k) => at(k).stream), ['CX', 'CX'])
assert.deepEqual(['ed', 'fay'].map((k) => [at(k).stream, at(k).lead]), [['Automation', false], ['Automation', false]])
// A saved workstream wins and isn't "from roles"; nothing names Ivy's.
assert.deepEqual([at('gus').stream, at('gus').fromRoles], ['Technology', false])
assert.deepEqual([at('ivy').stream, at('ivy').band], ['', false])
assert.equal(at('cy').fromRoles, true)
// Governance set by hand wins either way.
const set = OrgLayout.assign([P('ann', 'CX stream lead', { ...hi, governance: true }), P('pam', 'Project manager', { ...hi, governance: false })], place)
assert.deepEqual([set.get('ann').band, set.get('ann').lead, set.get('pam').band], [true, false, false])
assert.equal(OrgLayout.streamOfRole('Senior architect; Data platform stream'), 'Data platform')
console.log('org chart layout: ok')
