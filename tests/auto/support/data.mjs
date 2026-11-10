// A spec file's made-up data (tests/fixtures/serve.sh): built before its first test, stopped after its last; the CLI
// on it and, with `--ai virtual`, the virtual model's script and log.
import { execFileSync, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import net from 'node:net'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
const BIN = path.join(ROOT, 'target/debug/echoctl')
const SERVE = path.join(ROOT, 'tests/fixtures/serve.sh')

/** Fresh data for this spec file: `useData(test, { set: 'standard', ai: 'virtual' })`. The handle is filled in
 *  before the first test runs. */
export function useData(test, { set = 'small', ai, args = [] } = {}) {
  // The set's names, titles and ids (tests/auto/data/<set>.json): the specs never spell them themselves.
  const read = (name) => JSON.parse(fs.readFileSync(path.join(ROOT, 'tests/auto/data', `${name}.json`), 'utf8'))
  // The app's own wording the tests check (data/texts.json), apart from the set's.
  const data = { dir: '', base: '', env: {}, home: '', set: read(set), text: read('texts') }
  test.beforeAll(async () => {
    // The real path (macOS's temporary folder is a link): Echo names folders by it. serve.sh wants it not to exist yet.
    data.dir = fs.mkdtempSync(path.join(fs.realpathSync(os.tmpdir()), 'echo-auto-'))
    fs.rmdirSync(data.dir)
    const port = await freePort()
    const r = spawnSync(SERVE, ['up', data.dir, String(port), '--set', set, ...(ai ? ['--ai', ai] : []), ...args], { cwd: ROOT, encoding: 'utf8' })
    if (r.status !== 0) {
      // make_data's own reason (e.g. "build test data between 06:00 and 22:50"), not just "serve.sh failed".
      throw new Error(`the test data couldn't be built: ${(r.stderr || r.stdout).trim().split('\n').at(-1)}`)
    }
    data.base = /base_url=(\S+)/.exec(r.stdout)[1]
    data.env = readEnv(path.join(data.dir, 'env'))
    // Never the user's own folders: data, settings and Claude Code's config all inside this run's folder.
    for (const k of ['ECHOCTL_HOME', 'ECHOCTL_CONFIG_DIR', 'CLAUDE_CONFIG_DIR']) {
      if (!(data.env[k] || '').startsWith(data.dir + path.sep)) throw new Error(`${k} isn't the test data's: ${data.env[k]}`)
    }
    data.home = data.env.ECHOCTL_HOME
  })
  // A failed test keeps the data (its runs' claude.log, the model's log) for finding why; the path is printed.
  test.afterEach(async ({}, info) => {
    if (info.status !== info.expectedStatus) data.failed = true
  })
  test.afterAll(async () => {
    if (!data.dir) return
    spawnSync(SERVE, ['down', data.dir], { cwd: ROOT })
    if (data.failed) console.log(`test data kept for the failure: ${data.dir}`)
    else if (!process.env.ECHO_AUTO_KEEP) fs.rmSync(data.dir, { recursive: true, force: true })
  })

  /** `echoctl …` (the debug build of this checkout) with exactly the data's environment: `<dir>/env` unsets Claude
   *  Code's own variables, so nothing of this process's comes back. */
  data.cli = (...args) => {
    const r = spawnSync(BIN, args, { cwd: ROOT, env: data.env, encoding: 'utf8', timeout: 120_000 })
    return { code: r.status, out: r.stdout, err: r.stderr, json: () => JSON.parse(r.stdout) }
  }
  /** Any command on the data (a fixture script: `recordings.py copy …`), with the data's environment; its output. A
   *  last `{ timeout }` (ms) for a long one. */
  data.run = (cmd, ...args) => {
    const opts = typeof args.at(-1) === 'object' ? args.pop() : {}
    const r = spawnSync(cmd, args, { cwd: ROOT, env: data.env, encoding: 'utf8', timeout: opts.timeout || 120_000 })
    if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${(r.stderr || r.stdout).trim()}`)
    return r.stdout.trim()
  }
  /** A call's folder now, by the stamp its name starts with (a run may retitle and rename it). */
  data.callByStamp = (stamp) => path.join(data.home, 'calls', fs.readdirSync(path.join(data.home, 'calls')).find((n) => n.startsWith(stamp)))
  data.callName = (suffix) => fs.readdirSync(path.join(data.home, 'calls')).find((n) => n.endsWith(suffix))
  data.callDir = (suffix) => path.join(data.home, 'calls', data.callName(suffix))
  data.read = (...p) => fs.readFileSync(path.join(...p), 'utf8')
  data.json = (...p) => JSON.parse(data.read(...p))
  data.exists = (...p) => fs.existsSync(path.join(...p))
  /** The prompts Yolo runs were given (`<config>/yolo/<run>/prompt.md`), oldest first. */
  data.yoloPrompts = () => {
    const dir = path.join(data.env.ECHOCTL_CONFIG_DIR, 'yolo')
    if (!fs.existsSync(dir)) return []
    return fs.readdirSync(dir).sort().map((r) => path.join(dir, r, 'prompt.md')).filter((f) => fs.existsSync(f)).map((f) => fs.readFileSync(f, 'utf8'))
  }
  /** Every file of the data folder (its servers' logs left out) with its content's hash: two of them, before and after
   *  a step, show what it changed (`changed(before, after)`). */
  data.files = () => {
    const out = {}
    const walk = (d) => {
      for (const e of fs.readdirSync(d, { withFileTypes: true })) {
        const p = path.join(d, e.name)
        if (e.isDirectory()) walk(p)
        else if (e.isFile() && !['serve.log', 'server.log'].includes(e.name)) out[path.relative(data.dir, p)] = createHash('sha1').update(fs.readFileSync(p)).digest('hex')
      }
    }
    walk(data.dir)
    return out
  }
  /** `echoctl project ledger <thread> …`, its output read as JSON. */
  data.ledger = (thread, ...args) => {
    const r = data.cli('project', 'ledger', thread, ...args)
    if (r.code !== 0) throw new Error(`project ledger ${thread} ${args.join(' ')}: ${r.err}`)
    return r.json()
  }
  /** The last event of a thread's work ledger. */
  data.lastEvent = (thread) => JSON.parse(data.read(data.home, 'projects', thread, 'ledger/events.jsonl').trim().split('\n').at(-1))

  /** The virtual model (`ai: 'virtual'`): its script and the requests it answered since `clear()`. */
  data.model = {
    use: (file, fill = {}) => {
      let s = fs.readFileSync(path.join(ROOT, 'tests/fixtures/model-scripts', file), 'utf8')
      for (const [k, v] of Object.entries(fill)) s = s.replaceAll(k, v)
      fs.writeFileSync(path.join(data.dir, 'model/model-script.json'), s)
    },
    clear: () => fs.writeFileSync(path.join(data.dir, 'model/model-log.jsonl'), ''),
    /** Every answer held back this long (seconds): a run that can be caught while it writes. */
    delay: (seconds) => {
      const f = path.join(data.dir, 'model/model-script.json')
      const s = JSON.parse(fs.readFileSync(f, 'utf8'))
      s.model = { ...(s.model || {}), delay: seconds }
      fs.writeFileSync(f, JSON.stringify(s))
    },
    requests: () => fs.readFileSync(path.join(data.dir, 'model/model-log.jsonl'), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)),
    /** The turns one rule of the script answered (a run's own, not Claude Code's side requests). */
    turns: (rule) => data.model.requests().filter((e) => e.rule === rule),
    /** What the model was told back about each of its tool calls: true for a refusal, null for a success (of one
     *  rule's turns, or of every request). */
    toolResults: (rule) => (rule === undefined ? data.model.requests() : data.model.turns(rule)).flatMap((e) => {
      const last = e.messages?.at(-1)?.content
      return Array.isArray(last) ? last.filter((b) => b.type === 'tool_result').map((b) => b.is_error ?? null) : []
    }),
  }
  return data
}

/** `git status --short` of this checkout: a run Echo started changes nothing in it. */
export const gitStatus = () => execFileSync('git', ['status', '--short'], { cwd: ROOT, encoding: 'utf8' })

/** The files added, changed or removed between two `data.files()`. */
export const changed = (before, after) => [...new Set([...Object.keys(before), ...Object.keys(after)])].filter((f) => before[f] !== after[f]).sort()

/** The app's wording with its blanks filled: `fill('Synced {time}', { time })`. */
export const fill = (s, vars) => s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? vars[k] : m))

/** A string as a regular expression's literal part (a name with "(" or "." in it matches as written). */
export const literal = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/** The text of a model message's content (a string or blocks). */
export const textOf = (c) => (typeof c === 'string' ? c : (c || []).map((b) => b.text || '').join(' '))

/** A port nothing listens on now (the system's choice). */
function freePort() {
  return new Promise((resolve, reject) => {
    const s = net.createServer().listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => resolve(port)) })
    s.on('error', reject)
  })
}

/** The variables `<dir>/env` exports, as a shell sees them after sourcing it. */
function readEnv(file) {
  const out = execFileSync('/bin/sh', ['-c', '. "$1" && env -0', 'sh', file], { encoding: 'utf8' })
  return Object.fromEntries(out.split('\0').filter((kv) => kv.includes('=')).map((kv) => [kv.slice(0, kv.indexOf('=')), kv.slice(kv.indexOf('=') + 1)]))
}
