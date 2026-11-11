// `echoctl window --selftest`'s terminal check (selftestTerm, #419): what it types into the shell and how it knows the
// command ran — on Windows (cmd.exe; PowerShell too) without the POSIX `$((…))` macOS and Linux keep. Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/selftest-terminal.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const from = '// ── selftest terminal', to = '// end of the selftest-terminal block'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'selftest-terminal block not found in app.js')
const { selftestTerm } = new Function(`${src.slice(start, end)}\nreturn { selftestTerm }`)()

// An xterm.js buffer as the check reads it: lines, each maybe a wrapped continuation of the one before.
const buffer = (lines) => ({
  length: lines.length,
  getLine: (i) => {
    const l = lines[i]
    const text = typeof l === 'string' ? l : l.text
    return { isWrapped: typeof l !== 'string' && !!l.wrapped, translateToString: (trim) => (trim ? text.replace(/\s+$/, '') : text.padEnd(80)) }
  },
})

const tests = []
const test = (name, f) => tests.push([name, f])

test('macOS and Linux type what they always did', () => {
  assert.equal(selftestTerm(false).input, 'echo self$((40+2))test\r')
  assert.equal(selftestTerm(false).ran(buffer(['% echo self$((40+2))test', 'self42test', '%'])), true)
  assert.equal(selftestTerm(false).ran(buffer(['% echo self$((40+2))test'])), false)
})

test('Windows types no POSIX arithmetic', () => {
  assert.ok(!selftestTerm(true).input.includes('$(('))
  assert.ok(selftestTerm(true).input.endsWith('\r'))
})

test("Windows: cmd.exe's output line proves the command ran", () => {
  const t = selftestTerm(true)
  assert.equal(t.ran(buffer(['Microsoft Windows [Version 10.0.26300]', '', 'C:\data>echo self42test', 'self42test', '', 'C:\data>'])), true)
})

test("Windows: PowerShell's output line proves it too", () => {
  assert.equal(selftestTerm(true).ran(buffer(['PS C:\data> echo self42test', 'self42test', 'PS C:\data>'])), true)
})

test('Windows: the typed command alone, echoed back, is not output', () => {
  assert.equal(selftestTerm(true).ran(buffer(['C:\data>echo self42test'])), false)
})

test('Windows: a long prompt wrapping the typed command is not output either', () => {
  assert.equal(selftestTerm(true).ran(buffer([`C:\${'x'.repeat(73)}>echo `, { text: 'self42test', wrapped: true }])), false)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} of ${tests.length} failed`); process.exit(1) }
console.log(`all ${tests.length} passed`)
