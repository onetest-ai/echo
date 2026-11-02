# Vendored files

Compiled into echoctl (`include_str!`) and served from `echo://` — no CDN, no npm at build time.

| File | Package | License | SHA-256 |
|---|---|---|---|
| `xterm.js` | `@xterm/xterm@5.5.0` `lib/xterm.js` (source map line removed) | MIT (`xterm.LICENSE`) | `7a88e46a58847d3f17109e3d48cc8aa056a01997d00ceb76035f63370712d858` |
| `xterm.css` | `@xterm/xterm@5.5.0` `css/xterm.css` | MIT | `ba8e6985669488981ccf40c0cefe3aba80722cb6c92de7ad628b0bd717faf2b6` |
| `xterm-addon-fit.js` | `@xterm/addon-fit@0.10.0` `lib/addon-fit.js` (source map line removed) | MIT | `3e9ae6f6965ee1269d5574b905cf1bf559c2f44c784255b0034a71387e7719c9` |
| `mermaid.min.js` | mermaid (minutes page diagrams) | MIT | — |
| `sigma.min.js` | `sigma@4.0.4` `dist/sigma.min.js` (the Brain's graphs, WebGL) | MIT (`sigma.LICENSE`) | `85fa00f99cccbe6d80aaf16eee21dd42c2aaf3f8b02bb21a81b986649ce27416` |
| `graphology.umd.min.js` | `graphology@0.26.0` `dist/graphology.umd.min.js` | MIT (`graphology.LICENSE`) | `e65803eba4826a52a84ec3fedebda3e2434ff25d86b3e2309bfe6575141ab2ff` |
| `graphology-library.min.js` | `graphology-library@0.8.0` `dist/graphology-library.min.js` (ForceAtlas2 run in the page, never its worker) | MIT | `a414340a2feb9e859cba7d983022d9ad0cb4f5892c52c0814ac842ac82cc3a47` |

The npm tarballs matched the registry's `dist.integrity` (sha512) when downloaded (xterm 2026-10-29; sigma and graphology 2026-11-02).
