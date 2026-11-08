// ── ⌘K palette: ranking what was typed (no DOM: `palette.test.mjs`) ────────────────────────
// Each item: `{label, group, keywords?, disabled?}`. A query matches when its words all appear
// in the label (or keywords), in any order; whole-word and earlier matches rank first, and
// unavailable items after available ones. With no query, the items keep their own order.
const Palette = (() => {
  const norm = (s) => String(s ?? '').toLowerCase().normalize('NFKD').replace(/[\u0300-\u036f]/g, '')
  const atStart = (hay, i) => i === 0 || /[\s·:,/(-]/.test(hay[i - 1])
  // A word anywhere in the label; in the keywords only at a word's start ("min" isn't "terminal").
  function score(item, words) {
    const label = norm(item.label), keys = norm(item.keywords)
    let total = 0
    for (const w of words) {
      const i = label.indexOf(w)
      if (i >= 0) { total += (atStart(label, i) ? 30 : 10) - Math.min(i, 20) * 0.5; continue }
      const k = keys.split(/\s+/).some((x) => x.startsWith(w))
      if (!k) return -1
      total += 5
    }
    return total
  }
  return {
    /** The items for `query`, best first (at most `limit`). */
    rank(items, query, limit = 50) {
      const words = norm(query).split(/\s+/).filter(Boolean)
      if (!words.length) return items.slice(0, limit)
      return items
        .map((it, i) => ({ it, i, s: score(it, words) }))
        .filter((x) => x.s >= 0)
        .sort((a, b) => (!!a.it.disabled - !!b.it.disabled) || (b.s - a.s) || (a.i - b.i))
        .slice(0, limit)
        .map((x) => x.it)
    },
    /** The same items with each group kept together, groups in the order of their best item. */
    grouped(items) {
      const order = [...new Set(items.map((x) => x.group))]
      return order.flatMap((g) => items.filter((x) => x.group === g))
    },
  }
})()
// end of the palette block

