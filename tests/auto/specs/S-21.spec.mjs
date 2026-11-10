// S-21 Living with Echo — its stages with fixed checks (small data, the fake Claude Code): the Echo app itself, this
// checkout's debug build run on the data (`echoctl app run --no-tray --background`, started in the prelude) — a second
// start refused (stage 1), quit and start again (2), another data folder (3: its own app, started twice) — and the
// window following the system's light and dark (4). Stage 3 records a call fed from files and transcribes it: the
// speech model comes only from the test's own folder (ECHOCTL_TEST_MODELS); without it, stage 3 skips.
// Not here: stages 5–12 are a person's (start at login, `app start`, the native window and its Dock icon, updates from
// GitHub, offline, the network).
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { SettingsPage } from '../pages/settings.page.mjs'

const data = useData(test, { set: 'small', ai: 'fake' })

const a = () => data.text.app
const sp = () => data.text.speech
const set = () => data.set.s21
/** The test's own speech models (ECHOCTL_TEST_MODELS), if set. */
const models = () => process.env[sp().testModels]
/** The calls' files of a data folder and their hashes (`data.files()` keys are relative to the data's folder). */
const callFiles = (home) => {
  const prefix = path.relative(data.dir, path.join(home, 'calls')) + path.sep
  return Object.fromEntries(Object.entries(data.files()).filter(([f]) => f.startsWith(prefix)))
}
let app // the prelude's app: stage 1 finds it running, stage 2 quits it

scenario(test, 'S-21', 'Living with Echo: quit, one instance, another folder, appearance; login, updates, the native window, offline, the network', () => {
  // The prelude: no dialog and no run of the app's own, then the app.
  test.beforeAll(async () => {
    for (const args of [['config', '--unscheduled', 'manual', '--scheduled', 'manual', '--auto-minutes', 'false'], ['ai', 'live', 'off']]) {
      const r = data.cli(...args)
      if (r.code !== 0) throw new Error(`${args.join(' ')}: ${r.err}`)
    }
    app = await data.app()
  })

  test('1 · A second start of Echo is refused at once, naming the one running; one app runs on the config', covers('lifecycle/TC-004', 'F-LIFE-04'), async () => {
    const second = data.start('app', 'run', '--no-tray')
    const end = await second.ends(10_000)
    expect(end.code).toBe(1)
    expect(second.err.trim()).toBe(fill(a().alreadyRunning, { pid: app.pid }))
    expect(data.apps()).toEqual([app.pid])
  })

  test('2 · Quit leaves no process; Echo starts again with no first-run question, and `up` finds it', covers('lifecycle/TC-003', 'F-LIFE-03'), async () => {
    const quit = await app.quit()
    expect(quit.code, quit.err).toBe(0)
    expect(quit.out.trim()).toBe(fill(a().stopped, { pid: app.pid }))
    expect(data.apps()).toEqual([])
    expect(data.status().watcher).toBeNull()

    const again = await data.app()
    let up, children, end
    try {
      expect(data.status().watcher.pid).toBe(again.pid)
      children = data.childrenOf(again.pid)
      up = data.cli('up')
    } finally {
      end = await again.quit()
    }
    expect(children.filter((c) => c.includes(a().dialogProcess))).toEqual([])
    expect(up.code, up.err).toBe(0)
    const u = up.json()
    expect(u.home_chosen).toBe(true)
    expect(u.app_pid).toBe(again.pid)
    // No speech model chosen on this data: `up` asks for the doctor skill first. A file-fed run needs no permission:
    // the process running the tests may lack System Audio Recording, then the only one missing.
    expect(u.model).toBeNull()
    expect(u.next).toBe(a().up.doctor)
    expect([[true, []], [false, a().up.systemAudio]]).toContainEqual([u.ready, u.failing])
    expect(end.code, end.err).toBe(0)
    expect(end.out.trim()).toBe(fill(a().stopped, { pid: again.pid }))
    expect(data.apps()).toEqual([])
  })

  test('3 · Another data folder restarts Echo on it: a call records there, and the old folder is left as it was', covers('lifecycle/TC-005', 'F-LIFE-05'), async () => {
    test.skip(!models(), `${sp().testModels} isn't set: the speech model comes only from the test's own folder, never the installed Echo's`)
    test.setTimeout(3 * 60_000)
    const s = set()
    // This stage runs without ECHOCTL_HOME (it would pin the data folder): the folder is the one home.json names.
    const free = ['ECHOCTL_HOME']
    const cli = (...args) => data.cliWithout(free, {}, ...args)
    const home2 = path.join(data.dir, s.newHome)
    const watcher = () => data.status(free).watcher
    fs.symlinkSync(models(), path.join(data.env.ECHOCTL_CONFIG_DIR, sp().models))
    let first, set1, set2, info, armed, old, folder, calls2, restarted, quitted
    try {
      for (const args of [['model', 'use', s.model], ['config', '--languages', s.language]]) expect(data.cli(...args).code).toBe(0)
      set1 = cli('home', 'set', data.home)
      info = cli('home').json()
      old = callFiles(data.home)
      first = await data.app({ unset: free })
      expect(watcher().pid).toBe(first.pid)

      fs.writeFileSync(path.join(data.dir, data.text.recording.openLog), '')
      set2 = await data.cliAwait(free, 'home', 'set', home2)
      await first.ends(15_000)
      // The debug build restarted itself on the new folder (never `open` of an installed Echo, #142).
      await expect.poll(() => (watcher() || {}).pid, { timeout: 30_000 }).toBeTruthy()
      restarted = watcher().pid
      expect(restarted).not.toBe(first.pid)
      expect(data.read(data.dir, data.text.recording.openLog)).toBe('')
      expect(cli('home').json().home).toBe(home2)

      armed = cli('arm', '--title', s.call.title)
      await expect.poll(() => watcher().armed, { timeout: 10_000 }).toBe(true)
      data.run('python3', data.text.recording.call, 'start', s.call.conversation)
      const done = () => fs.readdirSync(path.join(home2, 'calls')).filter((n) => n.endsWith(`-${s.call.slug}`) && fs.existsSync(path.join(home2, 'calls', n, sp().transcript)))
      await expect.poll(done, { timeout: 60_000 }).toHaveLength(1)
      calls2 = fs.readdirSync(path.join(home2, 'calls'))
      folder = done()[0]
    } finally {
      data.run('python3', data.text.recording.call, 'end')
      const quit = await data.cliAwait(free, 'app', 'quit')
      if (first) await first.ends()
      // The restarted app isn't this test's own child: whatever is left on the config stops here.
      for (const pid of data.apps()) process.kill(pid, 'SIGKILL')
      cli('home', 'set', data.home)
      fs.unlinkSync(path.join(data.env.ECHOCTL_CONFIG_DIR, sp().models))
      quitted = quit
    }
    expect(quitted.code, quitted.err).toBe(0)
    expect(set1.out.trim()).toBe(fill(a().dataFolder, { path: data.home }))
    expect(info.home_source).toBe(a().chosen)
    expect(set2.out.trim().split('\n')).toEqual([fill(a().stopped, { pid: first.pid }), fill(a().dataFolder, { path: home2 }), fill(a().started, { pid: restarted })])
    expect(armed.out.trim()).toBe(a().armed)
    expect(calls2).toEqual([folder])
    expect(folder).toMatch(new RegExp(`-${literal(s.call.slug)}$`))
    expect(callFiles(data.home)).toEqual(old)
    expect(data.apps()).toEqual([])
  })

  test('4 · With Appearance on System, the window follows the system from light to dark and back, live', covers('window/TC-015', 'F-SET-04'), async ({ page }) => {
    const s = data.text.settings
    const a = s.appearance
    const settings = new SettingsPage(page, data)
    const heading = settings.heading(s.general.heading)
    /** The window and its headings drawn as the system's look, within a second of the system's change. */
    const drawn = async (look) => {
      await expect.poll(() => settings.windowColour(), { timeout: 1_000 }).toBe(a.window[look])
      await expect(heading).toHaveCSS('color', a.text[look], { timeout: 1_000 })
    }
    await page.emulateMedia({ colorScheme: 'light' })
    await settings.open(s.general.route)
    await expect(settings.choice(a.group, a.system)).toHaveAttribute('aria-pressed', 'true')
    expect(await settings.themeChosen()).toBeNull()
    await drawn('light')
    // No reload: the open page follows.
    await page.emulateMedia({ colorScheme: 'dark' })
    await drawn('dark')
    await page.emulateMedia({ colorScheme: 'light' })
    await drawn('light')
    await expect(settings.choice(a.group, a.system)).toHaveAttribute('aria-pressed', 'true')
  })
})
