// What an accessibility snapshot reads, for page objects that check an order (a list's groups and rows).

/** The lines of an accessibility snapshot's top level: each child's name (or its text), in order. */
export function readLines(snapshot) {
  return snapshot.split('\n').slice(1).filter((l) => /^ {2}- /.test(l)).map((l) => {
    const s = l.trim().replace(/^- /, '').replace(/^'(.*)'(:.*)?$/, '$1$2')
    const named = /^\w+ "(.*?)"/.exec(s)
    return named ? named[1] : s.replace(/^text:\s*/, '')
  })
}
