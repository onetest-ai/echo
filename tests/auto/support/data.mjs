// A spec file's made-up data (tests/fixtures/serve.sh): built before its first test, stopped after its last; the CLI
// on it and, with `--ai virtual`, the virtual model's script and log.
import { execFileSync, spawn, spawnSync } from 'node:child_process'
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
    // Anything `data.start` began and a failed or timed-out test left running, before its data goes.
    for (const r of started) {
      r.kill('SIGKILL')
      await r.done
    }
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
  /** `echoctl …` as `cli`, with a few variables more (`{ ECHOCTL_DEBUG: '1' }`). */
  data.cliWith = (extra, ...args) => {
    const r = spawnSync(BIN, args, { cwd: ROOT, env: { ...data.env, ...extra }, encoding: 'utf8', timeout: 300_000 })
    return { code: r.status, out: r.stdout, err: r.stderr, json: () => JSON.parse(r.stdout) }
  }
  /** `echoctl …` as `cli`, given this text on its standard input (a message read with `--file /dev/stdin`). */
  data.cliIn = (input, ...args) => {
    const r = spawnSync(BIN, args, { cwd: ROOT, env: data.env, encoding: 'utf8', input, timeout: 120_000 })
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
  /** `echoctl …` started in the background with the data's environment, as `cli` (a capture stopped by hand): what it
   *  printed so far (`out`, `err`), a signal sent to it (`kill('SIGINT')`, nothing once it has ended), and `done`, how
   *  it ended (`{ code, signal }`). */
  const started = []
  data.start = (...args) => startIn(data.env, args)
  /** The data's environment with some variables left out (`['ECHOCTL_HOME']`) and others added. */
  const envWithout = (unset = [], extra = {}) => {
    const env = { ...data.env, ...extra }
    for (const k of unset) delete env[k]
    return env
  }
  const startIn = (env, args) => {
    const p = spawn(BIN, args, { cwd: ROOT, env, stdio: ['ignore', 'pipe', 'pipe'] })
    const r = { out: '', err: '', ended: false, pid: p.pid }
    p.stdout.on('data', (b) => { r.out += b })
    p.stderr.on('data', (b) => { r.err += b })
    r.done = new Promise((resolve) => p.on('close', (code, signal) => { r.ended = true; resolve({ code, signal }) }))
    r.kill = (signal) => { if (!r.ended) p.kill(signal) }
    /** How it ended, waited for at most `ms`: past that it is killed (SIGKILL) and the wait fails. */
    r.ends = async (ms = 30_000) => {
      let timer
      const late = new Promise((resolve) => { timer = setTimeout(() => resolve(null), ms) })
      const end = await Promise.race([r.done, late])
      clearTimeout(timer)
      if (end) return end
      r.kill('SIGKILL')
      await r.done
      throw new Error(`echoctl ${args.join(' ')} didn't end within ${ms / 1000} s: killed`)
    }
    started.push(r)
    return r
  }
  /** `echoctl …` as `cli`, with some of the data's variables left out (`['ECHOCTL_HOME']`: the folder `home.json`
   *  names, as an app started without it reads it) and others added. */
  data.cliWithout = (unset, extra, ...args) => {
    const r = spawnSync(BIN, args, { cwd: ROOT, env: envWithout(unset, extra), encoding: 'utf8', timeout: 120_000 })
    return { code: r.status, out: r.stdout, err: r.stderr, json: () => JSON.parse(r.stdout) }
  }
  /** `echoctl …` as `cliWithout`, awaited without blocking this process: a command that waits for a process this one
   *  started to end (`app quit`, `home set` restarting the app) can see it end — a blocking `cli` leaves it unreaped. */
  data.cliAwait = async (unset, ...args) => {
    const p = startIn(envWithout(unset), args)
    const { code } = await p.ends(120_000)
    return { code, out: p.out, err: p.err, json: () => JSON.parse(p.out) }
  }
  /** `echoctl status`, read as JSON (`watcher` the running app's, `armed` the arm waiting); `unset` as `cliWithout`. */
  data.status = (unset = []) => data.cliWithout(unset, {}, 'status').json()
  /** The data's events (`<home>/.echo/events.jsonl`), each read as JSON, from the `from`th on: `data.events().length`
   *  before a step, `data.events(n)` after it. */
  data.events = (from = 0) => {
    const f = path.join(data.home, '.echo', 'events.jsonl')
    return fs.existsSync(f) ? fs.readFileSync(f, 'utf8').split('\n').filter(Boolean).slice(from).map((l) => JSON.parse(l)) : []
  }
  /** The Echo app on this data: `echoctl app run --no-tray --background` (this checkout's debug build, the data's
   *  file-fed calls; `unset` as `cliWithout`), started as `data.start` starts a process, once its watcher answers
   *  (30 s). `quit()` is `echoctl app quit` (its result), then the process's end (30 s, else killed and failed). */
  data.app = async ({ unset = [] } = {}) => {
    const app = startIn(envWithout(unset), ['app', 'run', '--no-tray', '--background'])
    const t0 = Date.now()
    while ((data.status(unset).watcher || {}).pid !== app.pid) {
      if (app.ended || Date.now() - t0 > 30_000) {
        app.kill('SIGKILL')
        await app.done
        throw new Error(`the Echo app didn't start: ${app.err || data.appLog()}`)
      }
      await new Promise((r) => setTimeout(r, 250))
    }
    app.quit = async () => {
      const r = await data.cliAwait(unset, 'app', 'quit')
      await app.ends()
      return r
    }
    return app
  }
  /** The pids of the `echoctl app run` processes on this data's config (the user's own Echo, on another, left out). */
  data.apps = () => {
    const pids = spawnSync('pgrep', ['-f', 'echoctl app run'], { encoding: 'utf8' }).stdout.split('\n').filter(Boolean)
    return pids.filter((p) => spawnSync('ps', ['eww', '-o', 'command=', '-p', p], { encoding: 'utf8' }).stdout.split(/\s/).includes(`ECHOCTL_CONFIG_DIR=${data.env.ECHOCTL_CONFIG_DIR}`)).map(Number)
  }
  /** The commands of a process's children (`pgrep -P`): what the app has started (a dialog's `osascript`). */
  data.childrenOf = (pid) => spawnSync('pgrep', ['-P', String(pid)], { encoding: 'utf8' }).stdout.split('\n').filter(Boolean)
    .map((p) => spawnSync('ps', ['-o', 'command=', '-p', p], { encoding: 'utf8' }).stdout.trim()).filter(Boolean)
  /** What the app logged (`--background`: `<home>/.echo/app.log`), or nothing yet. */
  data.appLog = () => (data.exists(data.home, '.echo', 'app.log') ? data.read(data.home, '.echo', 'app.log') : '')
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
