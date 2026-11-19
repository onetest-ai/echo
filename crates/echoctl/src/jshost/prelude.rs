//! The host half's `ctx`: the JavaScript every plugin's host half runs first (moved from jshost.rs as is).

/// The `ctx` every host half gets, built in JavaScript over `__host` / `__hostAsync`.
pub(super) const PRELUDE: &str = r#"
(() => {
  const host = globalThis.__host, hostAsync = globalThis.__hostAsync
  const commands = {}, timers = [], pending = {}, handlers = {}, providers = {}
  // A way to send this plugin adds (`ctx.deliver.provide`): Echo hands it the message.
  commands.__deliver = ({ kind, msg }) => {
    const p = providers[kind]
    if (!p) throw new Error(`this plugin doesn't send “${kind}”`)
    return p.send(msg)
  }
  const call = (op, arg) => {
    const out = JSON.parse(host(op, JSON.stringify(arg === undefined ? null : arg)))
    if (out && out.error) throw new Error(out.error)
    return out ? out.value : null
  }
  const later = (op, arg) => new Promise((resolve, reject) => { pending[hostAsync(op, JSON.stringify(arg === undefined ? null : arg))] = [resolve, reject] })
  globalThis.__settle = (token, json) => {
    const p = pending[token]; delete pending[token]; if (!p) return
    const out = JSON.parse(json); out.error ? p[1](new Error(out.error)) : p[0](out.value)
  }
  globalThis.__call = (id, name, args, files) => {
    const fn = commands[name]
    if (!fn) return host('reply', JSON.stringify({ call: id, error: `no command “${name}”` }))
    Promise.resolve().then(() => fn(JSON.parse(args), { files: JSON.parse(files) })).then(
      (v) => host('reply', JSON.stringify({ call: id, value: v === undefined ? null : v })),
      (e) => host('reply', JSON.stringify({ call: id, error: String((e && e.message) || e) })))
  }
  globalThis.__tick = (now) => {
    for (const t of timers) if (now >= t.next) {
      t.next = now + t.every
      Promise.resolve().then(t.fn).catch((e) => call('log', ['error', `timer: ${(e && e.message) || e}`]))
    }
  }
  // A WebSocket's events waiting for their handler (set after the await), every connection's together: at most 1,000
  // and 8 MB of their data (the heap is every plugin's). Past that the connection is closed, an error waiting after
  // what came; once it closed, what still waits no longer counts.
  let waiting = 0, waitingBytes = 0
  const hold = (id, q, ev) => {
    if (ev.kind === 'close') return q.push(ev)
    if (q.over) return
    const size = String(ev.data || ev.message || '').length
    if (waiting >= 1000 || waitingBytes + size > (8 << 20)) {
      q.over = true
      try { call('net.close', [id, 1000, '']) } catch (e) {}
      return q.push({ kind: 'error', message: 'more than 8 MB or 1,000 messages waited for this WebSocket\'s handlers, so it is closed: set onMessage, onClose and onError right after the await' })
    }
    ev.size = size; waiting++; waitingBytes += size; q.push(ev)
  }
  const letGo = (ev) => { if (ev.size !== undefined) { waiting--; waitingBytes -= ev.size; ev.size = undefined } return ev }
  // A connection's events (`net.rs`): each to its own handler, by id; a handler's error is logged.
  const conns = {}, fire = (fn, ...a) => Promise.resolve().then(() => fn && fn(...a)).catch((e) => call('log', ['error', `net: ${(e && e.message) || e}`]))
  globalThis.__net = (id, json) => { const c = conns[id]; if (c) c(JSON.parse(json)) }
  globalThis.__event = (name, payload) => {
    for (const fn of handlers[name] || []) Promise.resolve().then(() => fn(JSON.parse(payload))).catch((e) => call('log', ['error', `${name}: ${(e && e.message) || e}`]))
  }
  globalThis.__ctx = {
    id: call('id'),
    /** True in a terminal (`echoctl plugin call`): a command runs and the process ends — skip start-up work there. */
    terminal: call('terminal'),
    log: (...a) => call('log', ['info', a.map(String).join(' ')]),
    command: (name, fn) => { commands[name] = fn },
    every: (seconds, fn) => { timers.push({ every: Math.max(10, seconds) * 1000, next: 0, fn }) },
    /** Hear an event (`uses.events.on`): a promise, refused (and the handler dropped) for one not asked for. */
    on: (event, fn) => {
      try {
        call('events.on', String(event))
      } catch (e) {
        call('log', ['error', `ctx.on(${event}): ${e.message}`])
        const refused = Promise.reject(e)
        refused.catch(() => {}) // the plugin may not look: then it is only logged
        return refused
      }
      (handlers[event] = handlers[event] || []).push(fn)
      return Promise.resolve(true)
    },
    store: {
      /** `{lossy: true}`: bytes that aren't UTF-8 become U+FFFD instead of throwing. */
      read: (path, opts) => call('store.read', opts ? [path, opts] : path),
      /** `opts`: {ifSha} written only while the file is still the one with that sha256 (else "changed since you read
       *  it"), {createNew} never over a file. Every store write takes the plugin's lock across processes. */
      write: (path, text, opts) => call('store.write', [path, String(text), opts || {}]),
      /** Read, change, write — only over what was read: `fn(text | null)` gives the new text (undefined or the same:
       *  nothing written); when another process wrote meanwhile it reads again and calls `fn` again. → the text kept. */
      update: (path, fn) => {
        for (let i = 0; i < 50; i++) {
          const cur = call('store.read', path)
          const next = fn(cur)
          if (next === undefined || next === null || String(next) === cur) return cur
          try {
            call('store.write', [path, String(next), cur === null ? { createNew: true } : { ifSha: call('crypto.sha256', cur) }])
            return String(next)
          } catch (e) {
            if (!/changed since you read it|is there already/.test(String(e && e.message))) throw e
          }
        }
        throw new Error(`${path} kept changing while it was updated: try again`)
      },
      list: (dir) => call('store.list', dir || ''),
      remove: (path) => call('store.remove', path),
      path: (path) => call('store.path', path || ''),
      /** What it keeps in its store (`dir`: one folder of it): {bytes, files} — its own files only, not its settings. */
      size: (dir) => call('store.size', dir || ''),
      /** The same, as `usage`. */
      usage: (dir) => call('store.size', dir || ''),
      /** A file handed to it (a handle) copied as it is into its store (50 MB at most), never over a file unless
       *  `createNew: false`: {path, size}. */
      copyIn: (file, path, opts) => later('store.copy_in', [file, String(path), opts || {}]),
      /** As ctx.fs.walk, under its store's folder `path` ('' the store): [{path, size, mtime, sha?}]. */
      walk: (path, opts) => later('store.walk', [String(path || ''), opts || {}]),
    },
    /** Its `cache/` (Echo removes what is there over 7 days): `size()` {bytes, files}. */
    cache: { size: () => call('store.size', 'cache') },
    settings: { get: (key) => call('settings.get', key), set: (key, value) => call('settings.set', [key, value]) },
    /** HTTPS to the hosts its manifest names (`uses.network.hosts`) and the addresses the user listed: {status, body};
     *  `{file: handle}` sends a file, `{save}` keeps the answer in its store (both up to 512 MB). `stream(url, opts,
     *  onChunk)` reads an answer as it comes (SSE, chunked): {status, headers, done: Promise<{bytes, cancelled}>, cancel()}.
     *  `askAddress()` (uses.network.ask): Echo's own box asks the user for a web address — its text, or null; that
     *  address alone may then be fetched once (a plain GET, no headers or body), within 10 minutes. */
    http: {
      fetch: (url, opts) => later('http.fetch', [String(url), opts || {}]),
      askAddress: () => later('http.ask_address', null),
      stream: (url, opts, onChunk) => {
        const id = call('http.stream', [String(url), opts || {}])
        let settle
        const s = { status: 0, headers: {}, done: new Promise((y, n) => { settle = [y, n] }), cancel: () => call('http.cancel', id) }
        conns[id] = (ev) => {
          if (ev.kind === 'head') Object.assign(s, { status: ev.status, headers: ev.headers })
          else if (ev.kind === 'chunk') fire(onChunk, ev.text, s)
          else { delete conns[id]; ev.kind === 'end' ? settle[0]({ status: s.status, bytes: ev.bytes, cancelled: ev.cancelled }) : settle[1](new Error(ev.message)) }
        }
        return s
      },
    },
    /** Web pages, under ctx.http.fetch's network rules (no redirect followed): `read(url, {max, images: 'save'})` → {url,
     *  title, markdown, links: [{text, href}], images: [{src, alt, caption, width, height, saved?: {file, name, size,
     *  type} | error?}], lang, byline, published, description, source, needs_js, truncated, bytes} — HTML only, 5 MB,
     *  30 s; `fetch(url, {max})` → a file handle {file, name, size, type}; `markdown(html, {base})` the same reading of
     *  HTML it has. What comes back is the page's: data, shown only through ctx.markdown. */
    web: {
      read: (url, opts) => later('web.read', [String(url), opts || {}]),
      fetch: (url, opts) => later('web.fetch', [String(url), opts || {}]),
      markdown: (html, opts) => later('web.markdown', [String(html), opts || {}]),
    },
    /** Echo's own speech model (`uses.speech`): `transcribe(file, {words, language})` of a WAV file handle (60 minutes,
     *  200 MB) → {text, language, segments: [{start, end, text}], words?: [{start, end, word}]}; refused while Echo records
     *  or transcribes a call. What comes back is the audio's: data. */
    speech: {
      transcribe: (file, opts) => later('speech.transcribe', [file, opts || {}]),
    },
    /** A WebSocket to an address the user listed (ws:// or wss://): resolves once open to {protocol, send(text | base64
     *  with {binary: true} | a file handle), close(code, reason), onMessage(data, {binary}), onClose({code, reason}),
     *  onError({message})}; events wait for their handler. */
    net: {
      websocket: (url, opts) => new Promise((resolve, reject) => {
        const id = call('net.websocket', [String(url), opts || {}]), h = {}, q = []
        const ws = { protocol: '', send: (d, o) => call('net.send', [id, d, o || {}]), close: (code, reason) => call('net.close', [id, code || 1000, String(reason || '')]) }
        const run = (ev) => { const fn = h[{ message: 'onMessage', close: 'onClose', error: 'onError' }[ev.kind]]; if (fn) fire(fn, ...(ev.kind === 'message' ? [ev.data, { binary: !!ev.binary }] : [ev])); return !!fn }
        for (const k of ['onMessage', 'onClose', 'onError']) Object.defineProperty(ws, k, { get: () => h[k], set: (fn) => { h[k] = fn; while (q.length && run(q[0])) letGo(q.shift()) } })
        let open = false
        conns[id] = (ev) => {
          if (ev.kind === 'close') { delete conns[id]; q.forEach(letGo) }
          if (ev.kind === 'open') { open = true; ws.protocol = ev.protocol; return resolve(ws) }
          if (!open) return ev.kind === 'error' && reject(new Error(ev.message))
          if (q.length || !run(ev)) hold(id, q, ev)
        }
      }),
    },
    /** The user's Microsoft 365 through Echo's sign-in, read-only (uses.m365.read: mail, chats): search(query, {kinds, days,
     *  limit}), mailThread(message) → {subject, picked, messages}, chat(chat, {since, until}) → {id, topic, members, url, messages}. */
    m365: { search: (q, o) => later('m365.search', [String(q), o || {}]), mailThread: (m) => later('m365.mail_thread', String(m)), chat: (c, o) => later('m365.chat', [String(c), o || {}]) },
    package: { read: (path) => call('package.read', path), list: (dir) => call('package.list', dir || '') },
    markdown: Object.assign((md, opts) => call('markdown', opts === undefined ? String(md) : [String(md), opts]), { links: (md, opts) => call('markdown.links', opts === undefined ? String(md) : [String(md), opts]) }),
    /** Hashing in Rust: the sha256 of the text's UTF-8 bytes, in hex. */
    crypto: { sha256: (text) => call('crypto.sha256', String(text)) },
    notify: (title, body) => call('notify', [String(title), String(body || '')]),
    inbox: { set: (key, item) => call('inbox.set', [key, item]), clear: (key) => call('inbox.clear', key) },
    /** Today's widgets it declares (`widgets` in its manifest): `provide` answers Echo's ask ({size}) with the data,
     *  `set` gives it when the plugin learns something, `clear` takes it off Today until the next. */
    widgets: {
      provide: (id, fn) => { commands['__widget:' + id] = (a) => fn(a || {}) },
      set: (id, data) => call('widgets.set', [String(id), data === undefined ? null : data]),
      clear: (id) => call('widgets.clear', String(id)),
      /** It read something new: Echo asks its provider again now — widget `id`'s, or each one's — every copy (Today's
       *  and each dashboard's) with its own settings. Not heard from inside a provider; at most once in 10 s a widget. */
      changed: (id) => call('widgets.changed', id === undefined || id === null ? null : String(id)),
    },
    /** Its items on Today → My work and in threads' Work (uses.work): `provide(({scope, thread}) => rows)` answers
     *  Echo's ask — scope "me" (My work) or "thread" (that thread's Work tab) — with [{id, title, kind, where?, url?,
     *  due?, late?, thread?}] (200 at most); `changed()` has Echo ask again. */
    work: {
      provide: (fn) => { call('work.provide'); commands.__work = (a) => fn(a || {}) },
      changed: () => call('work.changed'),
    },
    /** Its own values per thread (no grant: its data, by thread id): `get(thread, key)` → value | null, `set(thread,
     *  key, value)` (JSON, 16 KB at most, 64 keys a thread, 4 MB in all; null removes it), `list(key)` → [{thread,
     *  value}] of the threads that have it. They follow the thread: gone when it is deleted (hear it: `threads_changed` in
     *  uses.events.on — {thread, born, change: renamed, name, from} | {…merged, into, dropped: its keys that didn't fit} |
     *  {…deleted, values: its own}; born: the thread's first stage time, another thread's if its id is used again). */
    threads: {
      get: (thread, key) => call('threads.get', [String(thread), String(key)]),
      set: (thread, key, value) => call('threads.set', [String(thread), String(key), value === undefined ? null : value]),
      list: (key) => call('threads.list', String(key)),
      /** A thread's stage (uses.echo.write: thread_stage): a stage id, with why — {applied, why?}; once the user changed
       *  the stage away from this plugin's, not applied until they let it set the stage again. */
      setStage: (thread, stage, reason) => call('threads.set_stage', [String(thread), String(stage), reason == null ? null : String(reason)]),
    },
    /** Its own values per client (no grant: its data, by client id), as `threads`: `get(client, key)` → value | null,
     *  `set(client, key, value)` (JSON, 16 KB at most, 64 keys a client, 4 MB in all; null removes it; a key without a
     *  dot), `list(key)` → [{client, value}] of the clients that have it. Kept when a client is renamed, gone when it is
     *  removed. */
    clients: {
      get: (client, key) => call('clients.get', [String(client), String(key)]),
      set: (client, key, value) => call('clients.set', [String(client), String(key), value === undefined ? null : value]),
      list: (key) => call('clients.list', String(key)),
    },
    /** Values the plugins of its marketplace share, one for the computer: get(key) → the value or null, set(key, value)
     *  (null removes it; created when it isn't there). `key` is `<ns>.<name>`, `ns` the marketplace's declared name in
     *  lower case; every plugin reads it, only its marketplace's plugins set it. ctx.threads takes such a key too: one
     *  value per thread for all of them. */
    shared: {
      get: (key) => call('shared.get', String(key)),
      set: (key, value) => call('shared.set', [String(key), value === undefined ? null : value]),
    },
    profiles: () => call('profiles'),
    /** Its package skills' scripts declared in uses.scripts: run(skill, script, args, {timeout}) → {status, stdout,
     *  stderr, truncated?, timed_out?} — no shell, args a list of strings, timeout at most the declared one. */
    scripts: {
      run: (skill, script, args, opts) => later('scripts.run', [String(skill), String(script), (args || []).map(String), opts || {}]),
    },
    claude: {
      /** opts.onText(chunk): Claude's answer piece by piece as it comes; the result is the same. */
      run: (opts) => {
        const o = Object.assign({}, opts || {}), onText = o.onText
        delete o.onText
        if (typeof onText !== 'function') return later('claude.run', o)
        const n = o.stream = call('claude.stream')
        conns[n] = (ev) => Promise.resolve().then(() => onText(ev.text)).catch((e) => call('log', ['error', `claude.run onText: ${(e && e.message) || e}`]))
        return later('claude.run', o).finally(() => { delete conns[n] })
      },
      /** Its conversation `resume` forgotten: its next run begins a new one; true, or false when there was none. */
      forget: (resume) => later('claude.forget', String(resume)),
      /** Its run under `key` (run's `key`) stopped: true, or false when none was running; that run fails "stopped". */
      stop: (key) => later('claude.stop', String(key)),
      /** Is a Claude Code plugin ("name@marketplace") installed for an AI profile ('' = the default)? Only its channel's. */
      hasPlugin: (spec, profile) => later('claude.has_plugin', [spec, profile || '']),
      /** Install it (user scope of the default profile's Claude Code): only the one `uses.channel.plugin` names. */
      installPlugin: (spec) => later('claude.install_plugin', [spec, '']),
    },
    /** The Claude · Echo folder session listens to a Claude Code channel (the one the manifest names) while
     *  it is open, with `env` (the channel's own keys; Claude and Anthropic ones are refused). */
    sessions: {
      channel: (opts) => call('sessions.channel', opts), clearChannel: () => call('sessions.clear_channel'),
      /** Skills for Echo's own Claude sessions (never the user's Claude Code): [{name, description, body}], text
       *  only — Echo writes the Claude Code plugin around them. [] takes them away. */
      skills: (list) => call('sessions.skills', list || []),
      /** Echo's sessions may read these of the folders the user gave it. */
      read: (folders) => call('sessions.read', folders || []),
    },
    /** A line for Claude when a client or thread is open in the window: [{client | thread, text}] (replaces all). */
    view: { hints: (list) => call('view.hints', list || []) },
    /** Its part for Claude in Echo (the manifest's `agent`): {summary?, entries: [{name, summary, guide}]} — each
     *  entry's guide (Markdown) a file Claude reads before working with it, a row of the core's table; replaces all. */
    agent: { set: (part) => call('agent.set', part || {}) },
    /** Echo's own data, read-only: 'clients' | 'threads' | 'people' | 'dictionary' | 'sources' (opts: kind, thread,
     *  client, q, since, limit) | 'source' (an id: call:<folder> | mail:<source>; opts: transcript, chat — false leaves
     *  it out) | 'calls' (opts: q, since, until, thread, client, limit) | 'calendar' (opts: from, to; the meetings Echo
     *  last read) | 'me' ({name, email}) | 'recording' ({state, call, stamp}). Its events (`ctx.on`, uses.events.on —
     *  minutes_written and sources_changed come with reading sources): `plugins::EVENTS`. */
    /** About me (Settings → About me), read-only (uses.me): `me({kinds})` → null when it is missing, empty or still the
     *  template, else {text, name, parts: {identity, people, terms, style, patterns}, hash} — only the kinds asked (all
     *  but patterns when none are named); `hash` changes when what it gets changes. Name and email alone: echo.read('me'). */
    me: (opts) => call('me', opts || {}),
    echo: {
      read: (kind, arg, opts) => call('echo.read', arg === undefined ? kind : opts === undefined ? [kind, arg] : [kind, arg, opts]),
      /** Names for one thing into the user's dictionary (uses.echo.write: dictionary): {name, synonyms, spellings, kind, client}. */
      dictionary: { add: (term) => call('echo.dictionary.add', term) },
    },
    /** Its own tasks in a thread (uses.echo.write: tasks), by its key: upsert {key, title, due?, owner?, state?, url?,
     *  reason?, propose?} → {task, applied, proposed, ignored, why?}; again with the same values writes nothing; a
     *  field the user set stays theirs (ignored). propose: the user's Yes first. */
    tasks: {
      upsert: (thread, task) => call('tasks.upsert', [String(thread), task || {}]),
      close: (thread, key, reason, opts) => call('tasks.close', [String(thread), String(key), reason == null ? null : String(reason), !!(opts && opts.propose)]),
    },
    /** The folders the user gave this plugin (Echo's picker), read-only: paths relative to one of them. */
    fs: {
      folders: () => call('fs.folders'),
      forget: (root) => call('fs.forget', String(root)),
      list: (root, path) => call('fs.list', [String(root), path || '']),
      /** `max` bytes, or `{max, strict}`: strict throws "not UTF-8: <path>" instead of replacing bad bytes. */
      read: (root, path, max) => { const o = max && typeof max === 'object' ? max : { max }; return call('fs.read', [String(root), String(path), o.max || 0, !!o.strict]) },
      /** Many files at once, off the plugin's own time slice (a vault's notes): [text | null] in order (`max` or
       *  `{max, strict}`; strict: a file that isn't UTF-8 is null). */
      readMany: (root, paths, max) => { const o = max && typeof max === 'object' ? max : { max }; return later('fs.read_many', [String(root), (paths || []).map(String), o.max || 0, !!o.strict]) },
      /** Every file under `path` (dot folders left out unless `hidden`): [{path, size, mtime, sha?}] up to `max`
       *  (`sha`: each file's sha256, up to 64 MB). */
      walk: (root, opts) => later('fs.walk', [String(root), opts || {}]),
      /** What is at `path`: {kind: file | dir | link | other, size, mtime, real, sha?} or null — a link never followed;
       *  `real` its place relative to the root (null when that is outside); `{sha: true}` hashes a file. */
      stat: (root, path, opts) => call('fs.stat', [String(root), String(path || ''), opts || {}]),
      /** Lines holding `query` as typed (no pattern), off its time slice: [{path, line, text}] — `path`, `ext`,
       *  `hidden`, `max` (200, at most 1,000), `caseSensitive`. */
      search: (root, query, opts) => later('fs.search', [String(root), String(query), opts || {}]),
      /** A copy of the folder in its store's cache/snapshots/<id>/ to work on: {path, files, bytes, skipped, why} — no
       *  link followed, never a .git folder or .obsidian/plugins at any depth, `exclude` paths (100 at most), dot names
       *  with `hidden`; 20,000 files and 512 MB at most (`maxFiles`, `maxBytes` less), a file over 50 MB or unreadable
       *  left out; one at a time, 3 kept and 2 GB in all. ctx.store.remove(path) removes it. */
      snapshot: (root, opts) => later('fs.snapshot', [String(root), opts || {}]),
      /** With `uses.folders.write`: write a file (atomically; {ifSha} only over the file it read, up to 64 MB;
       *  {createNew} or {ifSha: null} never over one), add to one's end (append, under its lock), make folders (mkdir), move one (never over another), delete
       *  one or an empty folder — inside the folders the user gave it, never through a link. */
      write: (root, path, text, opts) => call('fs.write', [String(root), String(path), String(text), opts || {}]),
      append: (root, path, text) => call('fs.append', [String(root), String(path), String(text)]),
      mkdir: (root, path) => call('fs.mkdir', [String(root), String(path)]),
      move: (root, from, to) => call('fs.move', [String(root), String(from), String(to)]),
      remove: (root, path) => call('fs.remove', [String(root), String(path)]),
      /** A file handed to it (a handle) copied as it is into a folder it was given (`uses.folders.write`, 50 MB at
       *  most), never over a file unless `createNew: false`: {path, size}. */
      copyIn: (file, root, path, opts) => later('fs.copy_in', [file, String(root), String(path), opts || {}]),
    },
    git: Object.fromEntries(['info', 'status', 'log', 'show', 'init', 'commit'].map((v) => [v, (root, ...a) => later(`git.${v}`, [String(root), ...a])])), /** Git in a folder it was given, off its time slice: info, status, log, show; init and commit with folders.write. */
    /** Files the user handed it — picked, dropped on its view, passed to a command — as handles {file, name, size}
     *  (an hour): `read(file, {max})` → text (1 MB by default, 4 MB at most). */
    files: {
      read: (file, opts) => call('handles.read', [file, (opts && opts.max) || 0]),
      /** The text of a file (PDF, Word, PowerPoint, spreadsheet, HTML, plain text), off its time slice: {text, pages,
       *  truncated}; `opts.pages` "3" or "2-5" of a PDF or deck, `pages` of the answer its page count (null if none). */
      text: (file, opts) => later('handles.text', [file, opts || {}]),
    },
    system: {
      which: (name) => call('system.which', name),
      /** Another app's file its manifest names in `uses.files.read` (that very entry, e.g. "~/…/obsidian.json"):
       *  its text (1 MB at most), or null when it isn't on this computer. */
      readFile: (entry) => call('files.read', String(entry)),
      /** The system's folder picker (the user can make a new folder in it); the folder picked is given to this plugin.
       *  Its path, or null. */
      pickFolder: (prompt) => later('system.pick_folder', String(prompt || 'Choose a folder')),
      /** The system's file picker: [{file, name, size}] handles of the files picked ([] cancelled); `multiple`, `ext`. */
      pickFiles: (prompt, opts) => later('system.pick_files', [String(prompt || 'Choose files'), opts || {}]),
    },
    /** The user's own sign-in to a host its manifest names in `uses.signin.hosts`: the one Echo keeps for it (with
     *  `uses.signin.keep`), else the GitHub CLI's or git's on this computer: `get` resolves {token, from: 'echo' | 'gh'
     *  | 'git', user?} or null when there is none (kept a minute; `{fresh: true}` asks again, e.g. after the token was
     *  refused); `sources` says which of the two programs are here ({gh, git}), reading no sign-in. With
     *  `uses.signin.keep`, `keep(host, token)` has Echo keep a sign-in you made (outside your store) and `forget(host)`
     *  drops it (true: there was one). */
    signin: {
      get: (host, opts) => later('signin.get', [String(host), !!(opts && opts.fresh)]),
      sources: (host) => call('signin.sources', String(host)),
      keep: (host, token) => call('signin.keep', [String(host), token]),
      forget: (host) => call('signin.forget', String(host)),
    },
    deliver: {
      status: () => call('deliver.status'),
      send: (kind, msg) => later('deliver.send', [kind, msg]),
      /** Add a way Echo sends to the user: `send(msg)` gets {title, markdown, about, ai}; tell Echo
       *  whether it can send now with `ready(kind, yes, why)`. */
      provide: (kind, opts) => { providers[kind] = opts; call('deliver.provide', { kind, label: opts.label || kind }) },
      ready: (kind, yes, why) => call('deliver.ready', { kind, ready: !!yes, why: why || '' }),
    },
  }
})()
"#;
