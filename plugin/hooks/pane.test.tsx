import { expect, test } from 'claude-code/testing'

const PANE_PROPS = {
  title: 'Echo · live',
  isFocused: false,
  bodyColumns: 120,
  placement: 'dock',
  scroll: { offset: 0, bodyRows: 30 },
  view: {},
} as never

for (const surface of ['terminal', 'desktop'] as const) {
  test(`idle pane shows the call browser and switches to live (${surface})`, async $ => {
    const ui = await $.ui.mount({ plugin: 'echo', surface, component: 'Pane', requestId: 'echo-live', props: PANE_PROPS } as never)
    // Idle: the call browser, with the Live/Calls switch
    expect(await ui.find({ type: 'Button', key: 'mode-calls' })).toBeDefined()
    expect(await ui.find({ text: /No recorded calls yet|Calls/ })).toBeDefined()
    await ui.press({ key: 'mode-live' })
    expect(await ui.find({ text: /Not recording/ })).toBeDefined()
  })
}
