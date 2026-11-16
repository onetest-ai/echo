// The test plugin's window half (tests/cases/today/TC-033): its own page for the "board" widget, drawn from the data
// its host half gives, every text through ctx.ui.esc.
export function apply(ctx) {
  ctx.ui.style('.wd-board{display:flex;gap:10px}.wd-board div{flex:1;border:1px solid var(--line);border-radius:8px;padding:8px;text-align:center}.wd-board b{display:block;font-size:22px}')
  ctx.widgets.draw('board', ({ data, size, theme }) =>
    `<div class="wd-board" data-size="${ctx.ui.esc(size)}" data-theme="${ctx.ui.esc(theme)}">${(data.cols || []).map((c) => `<div><b>${ctx.ui.esc(String(c.count))}</b>${ctx.ui.esc(c.name)}</div>`).join('')}</div>`)
}
