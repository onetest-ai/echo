// A plugin's work row (app.js, the plugin work rows block): its plugin's icon only as an image (or its initial), "From
// <plugin>", its text escaped, its title opening only an https page, no Echo actions. Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/plugin-work-rows.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── plugin work rows:'), b = src.indexOf('// end of the plugin work rows block', a)
assert.ok(a >= 0 && b > a, 'the plugin work rows block not found in app.js')
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
const row = new Function('esc', `${src.slice(a, b)}\nreturn workRowHtml`)(esc)

const r = { id: 'pr-7', title: 'Review: fix login', kind: 'review', where: 'acme-test/api', url: 'https://git.acme.example/pr/7', source: 'gh', source_name: 'GitHub', icon: 'plugin/gh/icon.svg' }
const h = row(r, '<i>due</i>', '')
assert.match(h, /<img class="wr-ic" src="plugin\/gh\/icon.svg" alt="From GitHub" title="From GitHub"/, 'its icon as an image, never inlined')
assert.match(h, /data-open-url="https:\/\/git.acme.example\/pr\/7"/, 'its title opens its page')
assert.match(h, /review · acme-test\/api/)
assert.ok(h.startsWith('<div class="td-task td-plug"') && !/data-tk=|data-today-close|data-claude-task/.test(h), "no Echo task's open, Close or Claude")

const evil = row({ ...r, title: '<img src=x onerror=alert(1)>', kind: '<b>', where: '"q"', url: 'javascript:alert(1)', icon: null, source_name: '<s>' }, '', '')
assert.ok(!/<img src=x|<b>|<s>/.test(evil), 'text escaped')
assert.ok(!evil.includes('data-open-url') && !evil.includes('javascript:'), 'only an https page opens')
assert.match(evil, /class="wr-ic wr-letter" title="From &lt;s&gt;"/, 'no icon: its initial')
assert.ok(row({ ...r, url: 'http://acme.example' }, '', '').indexOf('data-open-url') < 0, 'plain http does not open')
assert.match(row(r, '', '<button>Act</button>'), /<span class="td-icos"><button>Act<\/button><\/span>/, "its plugins' actions")
console.log('plugin-work-rows: ok')
