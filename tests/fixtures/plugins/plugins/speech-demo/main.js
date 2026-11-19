// A made-up plugin for tests: ctx.speech.transcribe on a file handle (`echoctl plugin call … --file`).
export function apply(ctx) {
  ctx.command('transcribe', (a, { files }) => ctx.speech.transcribe(files[0], a || {}))
}
