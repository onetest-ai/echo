// The window's script as it ships: the parts in `ui/app/*.js` joined in name order, by build.rs's rule
// (`join_parts`): only files named `NN-<area>.js`, each ending with a newline, any other .js file an error.
import { readFileSync, readdirSync } from 'node:fs'

const dir = new URL('./app/', import.meta.url)
export const appSource = () =>
  readdirSync(dir, { withFileTypes: true }).filter((d) => d.isFile() && d.name.endsWith('.js')).map((d) => d.name).sort().map((f) => {
    if (!/^\d\d-[a-z0-9-]+\.js$/.test(f)) throw new Error(`ui/app/${f}: a part's name is NN-<area>.js`)
    const text = readFileSync(new URL(f, dir), 'utf8')
    if (!text.endsWith('\n')) throw new Error(`ui/app/${f} must end with a newline`)
    return text
  }).join('')
