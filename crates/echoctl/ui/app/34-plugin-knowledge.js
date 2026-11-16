// Native-backed vault clients use this explicit context; no app state or terminal objects escape it.
function knowledgePluginCtx(id) {
  const copy = (v) => v == null ? v : JSON.parse(JSON.stringify(v))
  const vaultKey = (key) => {
    if (!/^claude:vault:[\p{L}\p{N}_-]+$/u.test(key)) throw new Error('Not a vault terminal')
    return key
  }
  return {
    read: async (path) => {
      const r = await get(`api/kn-plugin/${encodeURIComponent(id)}/read?path=${encodeURIComponent(path)}`)
      if (r && r.error) throw new Error(r.error)
      return r
    },
    send: (action) => {
      const cmd = action && action.cmd
      if (!(typeof cmd === 'string' && (cmd.startsWith('kn_') || ['open_url', 'sign_in'].includes(cmd)))) throw new Error('Not a Knowledge action')
      return send({ ...action, plugin: id })
    },
    register: (view) => Kn.register(id, view),
    tabs: () => knowledgeTabs().map(({ tab, label, plugin }) => ({ tab, label, plugin })),
    profiles: () => ({ default: ai().default, profiles: (ai().profiles || []).map(({ id, name, kind, notes_model, notes_model_label }) => ({ id, name, kind, notes_model, notes_model_label })) }),
    context: () => ({
      recall: brainOn() && !!(S.state && S.state.brain), notice: S.notice,
      selected: S.selected,
      call: S.call ? copy({ name: S.call.name, notes: !!S.call.notes, knowledge: S.call.knowledge }) : null,
      projects: copy(S.pj && S.pj.data ? {
        projects: (S.pj.data.projects || []).map(({ id, name, group }) => ({ id, name, group })),
        clients: (S.pj.data.clients || []).map(({ id, name }) => ({ id, name })),
      } : null),
    }),
    closeCallMenu: () => { S.menuOpen = false },
    refreshCall: () => S.selected ? loadCall(S.selected) : Promise.resolve(),
    terminal: {
      input: (key, data) => {
        const t = Terms.get(vaultKey(key))
        if (t && t.id != null && !t.exited) return send({ cmd: 'pty_input', id: t.id, data, plugin: id })
      },
      get: (key) => { const t = Terms.get(vaultKey(key)); return t ? { id: t.id, exited: t.exited, starting: t.starting, from: t.from } : null },
      open: () => claudePanel('claude-vault'),
      start: (key, opt) => termStart('claude-vault', vaultKey(key), { vault: opt.vault }),
      restart: (key, opt, conversation) => termRestart('claude-vault', vaultKey(key), { vault: opt.vault }, conversation),
      view: (key, label, intro, opt) => termView(vaultKey(key), 'claude-vault', label, intro, { vault: opt.vault }),
      saved: (key) => !!(S.sessions && S.sessions.has(key)),
    },
  }
}
