// The link of an autotest to the manual side. A spec file automates one scenario (`scenario('S-24', …)`, its stages
// run in order on the scenario's data); each test is a stage and names the functional cases it automates
// (`area/TC-NNN`) and the features it checks (`F-…`). The scenario and each case name the spec back (`auto:` in their
// frontmatter); `feature-map.py check` fails when the sides disagree, and every run's report maps both ways.
//
//   scenario(test, 'S-24', 'The calendar, preps and the Inbox on a busy day', () => {
//     test('…', covers('today/TC-013', 'F-TODAY-04'), async ({ page }) => { … })
//   })
export function covers(...ids) {
  return { annotation: ids.map((id) => ({ type: id.startsWith('F-') ? 'feature' : 'case', description: id })) }
}

/** The scenario this file automates: its tests are its stages, in order. */
export function scenario(test, id, title, body) {
  test.describe(`${id} ${title}`, { annotation: [{ type: 'scenario', description: id }] }, () => {
    test.describe.configure({ mode: 'serial' })
    body()
  })
}
