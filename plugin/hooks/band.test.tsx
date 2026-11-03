import { expect, test } from 'claude-code/testing'

import type { Band } from '../types'

const RECORDING: Band = {
  watching: true, recording: true, armed: false, next: null, muted: false,
  dir: '/x/calls/2026-11-03_100000-weekly', title: 'Weekly', startedAt: Date.now() - 125_000,
}

for (const surface of ['terminal', 'desktop'] as const) {
  test(`the status line says what is recorded, with a button to Echo (${surface})`, async ($, on) => {
    on('state.get', () => ({ value: { value: RECORDING, version: 0 } }))
    const ui = await $.ui.mount({ plugin: 'echo', surface, component: 'AbovePrompt', props: { hasSurvey: false } } as never)
    expect(await ui.find({ type: 'Button', key: 'open-echo' })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /Recording · Weekly · 2 min/ })).toBeDefined()
    await ui.unmount()
  })

  test(`nothing while the app isn't running (${surface})`, async ($, on) => {
    on('ui.render', ($, e) => {
      const { Text } = $.ui.resolve(e)
      return <Text>engine</Text>
    })
    const ui = await $.ui.mount({ plugin: 'echo', surface, component: 'AbovePrompt', props: { hasSurvey: false } } as never)
    expect((await ui.find({ type: 'Text' }))?.text).toBe('engine')
    await ui.unmount()
  })
}
