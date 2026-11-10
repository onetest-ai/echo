// What every spec file shares: fresh made-up data served for the file (tests/fixtures/serve.sh), the CLI on it, the
// virtual model's log, and the link of each test to its functional cases and features (tests/feature-map/).
import { execFileSync, spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const BIN = path.join(ROOT, 'target/debug/echoctl')
let port = 48200 + (process.pid % 50) * 10

/** A test's links: the functional cases it automates (`area/TC-NNN`) and the features it checks (`F-…`). They go into
 *  the run's results, where `feature-map.py auto-report` reads them; `feature-map.py check` checks they exist. */
export function covers(...ids) {
  return {
    annotation: ids.map((id) => ({ type: id.startsWith('F-') ? 'feature' : 'case', description: id })),
  }
}

/** Fresh data for this spec file: `serve.sh up` before its first test (set, `--ai`, extra make_data args), `down`
 *  after its last. Returns the handle the tests use (filled in by then). */
export function useData(test, { set = 'small', ai, args = [] } = {}) {
  const echo = { dir: '', base: '', env: {} }
  test.beforeAll(async () => {
    // The real path (macOS's temporary folder is a link): Echo names folders by it.
    echo.dir = fs.mkdtempSync(path.join(fs.realpathSync(os.tmpdir()), 'echo-auto-'))
    fs.rmdirSync(echo.dir) // serve.sh wants a folder that doesn't exist yet
    echo.port = port++
    const extra = ['--set', set, ...(ai ? ['--ai', ai] : []), ...args]
    const out = execFileSync(path.join(ROOT, 'tests/fixtures/serve.sh'), ['up', echo.dir, String(echo.port), ...extra], { cwd: ROOT, encoding: 'utf8' })
    echo.base = /base_url=(\S+)/.exec(out)[1]
    echo.env = readEnv(path.join(echo.dir, 'env'))
    // Never the real data folder: the data's own, under the run's temporary folder.
    if (!echo.env.ECHOCTL_HOME || !echo.env.ECHOCTL_HOME.startsWith(echo.dir)) throw new Error(`ECHOCTL_HOME isn't the test data's: ${echo.env.ECHOCTL_HOME}`)
    echo.home = echo.env.ECHOCTL_HOME
  })
  test.afterAll(async () => {
    if (!echo.dir) return
    spawnSync(path.join(ROOT, 'tests/fixtures/serve.sh'), ['down', echo.dir], { cwd: ROOT })
    if (!process.env.ECHO_AUTO_KEEP) fs.rmSync(echo.dir, { recursive: true, force: true })
  })
  /** `echoctl …` on the data (the debug build of this checkout): exit code, output, and its JSON when it printed some. */
  echo.cli = (...args) => {
    // Exactly the shell's environment after `. <dir>/env` (it unsets Claude Code's own variables; merging this
    // process's back would undo that).
    const r = spawnSync(BIN, args, { cwd: ROOT, env: echo.env, encoding: 'utf8', timeout: 120_000 })
    return { code: r.status, out: r.stdout, err: r.stderr, json: () => JSON.parse(r.stdout) }
  }
  /** Open the window at a hash, its cookie taken first. */
  echo.open = async (page, hash = '') => {
    await page.goto(echo.base + '/')
    await page.goto(echo.base + '/' + hash)
  }
  echo.call = (suffix) => fs.readdirSync(path.join(echo.home, 'calls')).find((n) => n.endsWith(suffix))
  echo.callDir = (suffix) => path.join(echo.home, 'calls', echo.call(suffix))
  echo.read = (...p) => fs.readFileSync(path.join(...p), 'utf8')
  /** The virtual model (`--ai virtual`): its script, and the requests it answered since `clear()`. */
  echo.model = {
    script: (file) => fs.copyFileSync(path.join(ROOT, 'tests/fixtures/model-scripts', file), path.join(echo.dir, 'model/model-script.json')),
    write: (obj) => fs.writeFileSync(path.join(echo.dir, 'model/model-script.json'), JSON.stringify(obj)),
    clear: () => fs.writeFileSync(path.join(echo.dir, 'model/model-log.jsonl'), ''),
    log: () => fs.readFileSync(path.join(echo.dir, 'model/model-log.jsonl'), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)),
  }
  return echo
}

/** The variables `<dir>/env` exports, as the shell sees them after sourcing it. */
function readEnv(file) {
  const out = execFileSync('/bin/sh', ['-c', `. "${file}" && env -0`], { encoding: 'utf8' })
  const env = {}
  for (const kv of out.split('\0')) {
    const i = kv.indexOf('=')
    if (i > 0) env[kv.slice(0, i)] = kv.slice(i + 1)
  }
  return env
}

/** The text of a message's content (a string or blocks). */
export const textOf = (c) => (typeof c === 'string' ? c : (c || []).map((b) => b.text || '').join(' '))
