import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
const shim = readFileSync(new URL('./knowledge.js', import.meta.url), 'utf8')
const context = readFileSync(new URL('./app/34-plugin-knowledge.js', import.meta.url), 'utf8')
let routed = 0, drawn = 0
const { Kn, knParseRoute } = new Function('route', 'render', `${shim}; return { Kn, knParseRoute }`)(() => routed++, () => drawn++)
assert.deepEqual(knParseRoute('kn/twin/pages/knowledge%2Facme.md'), { vault: 'twin', tab: 'pages', path: 'knowledge/acme.md' })
assert.equal(knParseRoute('kn/../pages/x'), null)
assert.match(Kn.view(), /Install or enable Knowledge/)
assert.equal(Kn.callChip({}), '')
const calls = []
Kn.route({ vault: 'twin', tab: 'ask', path: 'conversation' })
const view = { view: () => 'plugin view', route: (r) => calls.push(r), load: async () => calls.push('load'), active: () => true, setSource: (s) => calls.push(s), editing: () => true }
Kn.register('knowledge', view)
assert.equal(Kn.view(), 'plugin view')
assert.equal(Kn.editing(), true)
assert.deepEqual(calls[1], { vault: 'twin', tab: 'ask', path: 'conversation' })
assert.throws(() => Kn.register('other', view), /already registered/)
await new Promise((ok) => setTimeout(ok, 0))
assert.equal(routed, 1)
assert.equal(drawn, 1)
Kn.unload('other')
assert.equal(Kn.view(), 'plugin view')
Kn.unload('knowledge')
assert.equal(Kn.editing(), false)
assert.match(Kn.view(), /Install or enable Knowledge/)
assert.equal(Kn.vaultId(), '')
await Kn.tick()
// A context returns detached data and exposes no mutable Echo/terminal state.
const state = { notice: null, state: {brain: true}, call: {name:'call',notes:'minutes',knowledge:{vaults:[{id:'twin'}]}}, selected:'call', pj:{data:{projects:[{id:'acme',name:'Acme',group:'client',notes:'private note',people:['synthetic'],sources:['private source'],brief:'private brief'}],clients:[{id:'acme',name:'Acme',notes:'private client note'}]}}, sessions:new Set() }
const sent = [], terminals = new Map()
const env = { S:state, get:async()=>({}), send:(v)=>sent.push(v), ai:()=>({profiles:[{id:'system',name:'System',kind:'system',secret:'never shared'}]}), brainOn:()=>true, knowledgeTabs:()=>[], Terms:terminals, Kn, loadCall:async()=>{}, claudePanel:()=>{}, termStart:()=>{}, termRestart:()=>{}, termView:()=>'' }
const make = new Function(...Object.keys(env), `${context}; return knowledgePluginCtx`)(...Object.values(env))
const ctx = make('knowledge')
ctx.context().call.knowledge.vaults[0].id = 'changed'
assert.equal(state.call.knowledge.vaults[0].id, 'twin')
assert.equal(ctx.profiles().profiles[0].secret, undefined)
assert.throws(()=>ctx.terminal.get('claude:call:other'), /Not a vault terminal/)
assert.throws(()=>ctx.send({cmd:'delete_call'}), /Not a Knowledge action/)
console.log('knowledge bridge: route, ownership, unload, context isolation and terminal boundaries passed')

assert.deepEqual(ctx.context().projects, {projects:[{id:'acme',name:'Acme',group:'client'}],clients:[{id:'acme',name:'Acme'}]})
ctx.context().projects.projects[0].name = 'changed'
assert.equal(state.pj.data.projects[0].name, 'Acme')
assert.throws(()=>ctx.send({cmd:'pty_input',id:999999,data:'synthetic-no-execution'}), /Not a Knowledge action/)
assert.throws(()=>ctx.terminal.input('claude:call:other', 'synthetic'), /Not a vault terminal/)
ctx.terminal.input('claude:vault:acme', 'missing')
terminals.set('claude:vault:acme', {id:42,exited:true})
ctx.terminal.input('claude:vault:acme', 'exited')
assert.deepEqual(sent, [])
terminals.set('claude:vault:acme', {id:43,exited:false})
ctx.terminal.input('claude:vault:acme', 'synthetic\r')
assert.deepEqual(sent, [{cmd:'pty_input',id:43,data:'synthetic\r',plugin:'knowledge'}])
terminals.set('claude:vault:acme', {id:44,exited:false})
ctx.terminal.input('claude:vault:acme', 'current terminal')
assert.equal(sent[1].id, 44)
console.log('knowledge security: minimal picker data, raw input refusal and current vault terminal routing passed')
