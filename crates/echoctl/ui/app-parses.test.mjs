// The window's whole script as it ships (the parts joined, as build.rs joins them) parses. The other node tests load
// only their own block, so a part that breaks the script elsewhere (a comment eating the rest of a line, a missing
// bracket) passed them all and left the window blank. Run by scripts/check-mod.sh.
import { appSource } from './source.mjs'

try {
  new Function(appSource())
  console.log('app script: parses')
} catch (e) {
  console.error(`app script: ${e.name}: ${e.message}`)
  process.exit(1)
}
