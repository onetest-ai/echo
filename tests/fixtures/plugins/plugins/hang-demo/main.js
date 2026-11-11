// A test plugin whose one widget's provider never resolves: Echo's ask for it fails after its 5 s (today/TC-034).
export function apply(ctx) {
  ctx.widgets.provide('hang', () => new Promise(() => {}))
}
