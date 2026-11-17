// The plugins' tabs of Knowledge (app.js, the knowledge tabs block): each plugin's tabs drawn as a group headed by its
// name, after Recall's, in their `order`; a hidden tab (`visible()` false) left out, one whose visible/badge throws
// handled; a tab's badge '' | n | '!'; the activity bar's badge the sum, or '!'; `#brain/<plugin>:<tab>/<rest>` parsed
// with its rest as written; the tab shown when the address names none, a hidden one or a Recall tab with Recall off;
// old single `knowledge.tab` items (no icon, no order) still a tab. Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/knowledge-tabs.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── knowledge tabs:'), b = src.indexOf('// end of the knowledge tabs block', a)
assert.ok(a >= 0 && b > a, 'the knowledge tabs block not found in app.js')
const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
const K = new Function('esc', `${src.slice(a, b)}\nreturn { knowledgeGroups, knowledgeBadge, knowledgeHash, knowledgeTab, knowledgeTabRow, knowledgeHeaders }`)(esc)
assert.ok(/SLOTS = \[[^\]]*'knowledge\.header'/.test(src), 'knowledge.header is one of SLOTS')

const tests = []
const test = (name, f) => tests.push([name, f])
const keys = (groups) => groups.map((g) => [g.name, g.tabs.map((t) => t.key)])

let vault = false
const items = () => [
  { plugin: 'kb', id: 'graph', label: 'Graph', icon: '<svg/>', order: 30, render: () => '' },
  { plugin: 'kb', id: 'home', label: 'Home', icon: '<svg/>', order: 10, visible: () => vault, render: () => '' },
  { plugin: 'old', id: 'notes', label: 'Notes', render: () => '' },
  { plugin: 'kb', id: 'vaults', label: 'Vaults', order: 90, badge: () => (vault ? '' : '!'), render: () => '' },
  { plugin: 'kb', id: 'ingest', label: 'Ingest', order: 40, visible: () => vault, badge: () => 3, render: () => '' },
  { plugin: 'kb', id: 'pages', label: 'Pages', icon: '<svg/>', order: 20, visible: () => vault, badge: () => '2', render: () => '' },
]
const names = { kb: 'Knowledge base', old: 'Obsidian <vaults>' }

test("each plugin's tabs are a group headed by its name, in the order the plugins added them, its tabs by `order`", () => {
  vault = true
  assert.deepEqual(keys(K.knowledgeGroups(items(), names, () => {})), [
    ['Knowledge base', ['kb:home', 'kb:pages', 'kb:graph', 'kb:ingest', 'kb:vaults']],
    ['Obsidian <vaults>', ['old:notes']],
  ])
})

test('a tab whose visible() is false is left out: no vault, only Vaults and Graph', () => {
  vault = false
  assert.deepEqual(keys(K.knowledgeGroups(items(), names, () => {})), [['Knowledge base', ['kb:graph', 'kb:vaults']], ['Obsidian <vaults>', ['old:notes']]])
})

test("a plugin whose every tab is hidden has no group; one with no name known is headed by its id", () => {
  const g = K.knowledgeGroups([{ plugin: 'kb', id: 'a', label: 'A', visible: () => false, render: () => '' }, { plugin: 'x', id: 'b', label: 'B', render: () => '' }], {}, () => {})
  assert.deepEqual(keys(g), [['x', ['x:b']]])
})

test('a tab without order keeps the place it was added in, after the ordered ones', () => {
  const g = K.knowledgeGroups([{ plugin: 'p', id: 'one', label: '1', render: () => '' }, { plugin: 'p', id: 'two', label: '2', order: 5, render: () => '' }, { plugin: 'p', id: 'three', label: '3', render: () => '' }], {}, () => {})
  assert.deepEqual(keys(g), [['p', ['p:two', 'p:one', 'p:three']]])
})

test("a tab whose visible() or badge() throws: hidden, or shown without a badge — logged, the others drawn", () => {
  const log = []
  const g = K.knowledgeGroups([
    { plugin: 'p', id: 'a', label: 'A', visible: () => { throw new Error('no') }, render: () => '' },
    { plugin: 'p', id: 'b', label: 'B', badge: () => { throw new Error('bad badge') }, render: () => '' },
    { plugin: 'p', id: 'b', label: 'B again', render: () => '' },
  ], {}, (m) => log.push(m))
  assert.deepEqual(keys(g), [['p', ['p:b']]])
  assert.equal(g[0].tabs[0].badge, '')
  assert.equal(g[0].tabs[0].label, 'B', 'the second tab with the same id is left out')
  assert.equal(log.length, 3)
  assert.ok(log.some((m) => m.includes('a second tab')))
})

test("a tab's badge: '!' stays, a positive number is shown, nothing, zero or text none", () => {
  const badge = (v) => K.knowledgeGroups([{ plugin: 'p', id: 'a', label: 'A', badge: () => v, render: () => '' }], {}, () => {})[0].tabs[0].badge
  assert.deepEqual([badge('!'), badge(4), badge('7'), badge(0), badge(''), badge(null), badge(undefined), badge('soon'), badge(-2), badge(2.6)], ['!', 4, 7, '', '', '', '', '', '', 2])
})

test("the activity bar's badge: the plugins' badges summed, '!' when any is '!', nothing when none", () => {
  vault = true
  assert.equal(K.knowledgeBadge(K.knowledgeGroups(items(), names, () => {})), 5)
  vault = false
  assert.equal(K.knowledgeBadge(K.knowledgeGroups(items(), names, () => {})), '!')
  assert.equal(K.knowledgeBadge([]), '')
  assert.equal(K.knowledgeBadge(K.knowledgeGroups([{ plugin: 'p', id: 'a', label: 'A', render: () => '' }], {}, () => {})), '')
})

test('the address: `#brain/<plugin>:<tab>/<rest>`, the rest as written (its parts still encoded)', () => {
  assert.deepEqual(K.knowledgeHash('#brain/kb:pages/acme/Q3%20plan%2Fdraft.md'), { tab: 'kb:pages', rest: 'acme/Q3%20plan%2Fdraft.md' })
  assert.deepEqual(K.knowledgeHash('#brain/kb:pages'), { tab: 'kb:pages', rest: '' })
  assert.deepEqual(K.knowledgeHash('#brain/kb%3Apages/x'), { tab: 'kb:pages', rest: 'x' })
  assert.deepEqual(K.knowledgeHash('#brain/sources'), { tab: 'sources', rest: '' })
  assert.deepEqual(K.knowledgeHash('#brain'), { tab: '', rest: '' })
  assert.deepEqual(K.knowledgeHash('brain/obsidian:notes/echo/a%20b'), { tab: 'obsidian:notes', rest: 'echo/a%20b' })
  assert.equal(K.knowledgeHash('#brainy'), null)
  assert.equal(K.knowledgeHash('#calls'), null)
  assert.deepEqual(K.knowledgeHash('#brain/kb%E0:x'), { tab: 'kb%E0:x', rest: '' }, 'a broken %-sequence stays as written')
})

const RECALL = ['sources', 'terms', 'people']
test('the tab shown: the one the address names when it is there; a Recall tab only with Recall on', () => {
  vault = true
  const g = K.knowledgeGroups(items(), names, () => {})
  assert.equal(K.knowledgeTab('kb:pages', true, g, RECALL), 'kb:pages')
  assert.equal(K.knowledgeTab('terms', true, g, RECALL), 'terms')
  assert.equal(K.knowledgeTab('old:notes', false, g, RECALL), 'old:notes', 'an old single tab, Recall off')
})

test('nothing named, or a Recall tab with Recall off: Recall’s first tab when it is on, else the first visible plugin tab', () => {
  vault = true
  const g = K.knowledgeGroups(items(), names, () => {})
  assert.equal(K.knowledgeTab('', true, g, RECALL), 'sources')
  assert.equal(K.knowledgeTab('', false, g, RECALL), 'kb:home')
  assert.equal(K.knowledgeTab('sources', false, g, RECALL), 'kb:home')
  vault = false
  assert.equal(K.knowledgeTab('', false, K.knowledgeGroups(items(), names, () => {}), RECALL), 'kb:graph')
  assert.equal(K.knowledgeTab('', false, [], RECALL), null, 'Recall off and no plugin tab: nothing (the section is gone)')
})

test("a hidden or unknown tab of a plugin: that plugin's first visible tab; a plugin not loaded: as if none was named", () => {
  vault = false
  const g = K.knowledgeGroups(items(), names, () => {})
  assert.equal(K.knowledgeTab('kb:pages', false, g, RECALL), 'kb:graph')
  assert.equal(K.knowledgeTab('kb:nope', true, g, RECALL), 'kb:graph')
  assert.equal(K.knowledgeTab('gone:tab', true, g, RECALL), 'sources')
  assert.equal(K.knowledgeTab('gone:tab', false, g, RECALL), 'kb:graph')
})

test("the tab row: Recall's tabs first, then each plugin's group under its name (escaped), icons, badges, the one shown selected", () => {
  vault = true
  const g = K.knowledgeGroups(items(), names, () => {})
  const html = K.knowledgeTabRow([['sources', 'Sources', 12], ['terms', 'Dictionary', 0]], g, 'kb:pages')
  assert.ok(html.indexOf('data-br-tab="sources"') < html.indexOf('data-br-tab="kb:home"'), "Recall's tabs before the plugins'")
  assert.ok(html.includes('<span class="n">12</span>') && !html.includes('Dictionary<span'), 'a count only when there is one')
  assert.ok(html.includes('aria-label="Knowledge base"') && html.includes('Obsidian &lt;vaults&gt;'), "each group named, the plugin's name escaped")
  assert.ok(/data-br-tab="kb:pages" aria-selected="true">/.test(html) && /data-br-tab="sources" aria-selected="false">/.test(html))
  assert.ok(html.includes('data-br-tab="kb:pages" aria-selected="true"><svg/>Pages<span class="n">2</span>'), "a tab's icon and its badge")
  vault = false
  const off = K.knowledgeTabRow([], K.knowledgeGroups(items(), names, () => {}), 'kb:vaults')
  assert.ok(off.includes('Vaults<span class="n kn-alert" title="Needs you">!</span>'), "'!' marked")
  assert.ok(!off.includes('data-br-tab="sources"'), "Recall off: none of Recall's tabs")
})

test("the header strip: one per plugin, the first it added; another plugin's never used", () => {
  const log = []
  const h = K.knowledgeHeaders([{ plugin: 'kb', header: () => 'A' }, { plugin: 'kb', header: () => 'B' }, { plugin: 'x', header: () => 'X' }], (m) => log.push(m))
  assert.equal(h.kb.header(), 'A')
  assert.equal(h.x.header(), 'X')
  assert.equal(h.old, undefined)
  assert.equal(log.length, 1)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.error(`not ok - ${name}\n${e.stack}`) }
}
if (failed) { console.error(`${failed} failed`); process.exit(1) }
