// How a project is drawn and its stages (app.js, ProjectArt). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/projects.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const src = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const start = src.indexOf('// ── projects: how a project is drawn')
const end = src.indexOf('// end of the projects block', start)
assert.ok(start >= 0 && end > start, 'the projects block not found in app.js')
const { ProjectArt } = new Function(`${src.slice(start, end)}\nreturn { ProjectArt }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

test('an identicon is 5×5, mirrored left to right, and the same for the same id', () => {
  for (const id of ['aurora', 'helix-data-platform', 'orbit-hiring', 'x', 'globex-agent-hub']) {
    const c = ProjectArt.cells(id)
    assert.equal(c.length, 25)
    for (let r = 0; r < 5; r++) {
      assert.equal(c[r * 5], c[r * 5 + 4], `${id} row ${r}`)
      assert.equal(c[r * 5 + 1], c[r * 5 + 3], `${id} row ${r}`)
    }
    assert.deepEqual(ProjectArt.cells(id), c)
  }
})

test('never (nearly) empty or full', () => {
  for (let i = 0; i < 2000; i++) {
    const c = ProjectArt.cells('p' + i)
    const half = [0, 1, 2, 3, 4].flatMap((r) => [0, 1, 2].map((k) => c[r * 5 + k]))
    const on = half.filter(Boolean).length
    assert.ok(on >= 3 && on <= 12, `p${i}: ${on} of 15`)
  }
})

test('different ids mostly look different', () => {
  const seen = new Set()
  for (let i = 0; i < 500; i++) seen.add(ProjectArt.cells('project-' + i).map(Number).join(''))
  assert.ok(seen.size > 450, `${seen.size} distinct of 500`)
})

test('stages: label and group, unknown ones kept as they are', () => {
  assert.deepEqual(ProjectArt.stage('on_hold'), { id: 'on_hold', label: 'On hold', group: 'Delivery', gid: 'delivery' })
  assert.equal(ProjectArt.stage('negotiation').gid, 'pursuit')
  assert.equal(ProjectArt.stage('lost').group, 'Closed')
  assert.equal(ProjectArt.STAGES.flatMap(([, , xs]) => xs).length, 10)
  assert.equal(ProjectArt.stage('won').label, 'won')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`projects: ${tests.length} passed`)
