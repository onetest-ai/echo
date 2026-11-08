// The window's script as it ships: the parts in `ui/app/*.js` joined in name order, as build.rs joins them.
import { readFileSync, readdirSync } from 'node:fs'

const dir = new URL('./app/', import.meta.url)
export const appSource = () =>
  readdirSync(dir).filter((f) => f.endsWith('.js')).sort().map((f) => readFileSync(new URL(f, dir), 'utf8')).join('')
