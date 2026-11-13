// ── Colleagues' photos in place of initials (`api/photos`, the app's `photos`): asked once per name, in batches; the
// app answers with the names it has a photo for and fetches the others in the background (only colleagues in the
// user's own domain, only when the Microsoft 365 sign-in may read photos): a name it is fetching (null) is asked again,
// a little later each time.

// ── photos
const PHOTO = new Map() // name → its photo's address, or false (none), or 0 (asked, waiting)
const photoAsk = new Set()
let photoTimer = null
/** A name's photo address if the app has one, else null (asking for it the first time). */
function photoOf(name) {
  if (!name) return null
  if (!PHOTO.has(name)) { PHOTO.set(name, 0); photoAsk.add(name); clearTimeout(photoTimer); photoTimer = setTimeout(askPhotos, 250) }
  return PHOTO.get(name) || null
}
async function askPhotos(again, tries = 0) {
  const names = again || [...photoAsk]
  photoAsk.clear()
  if (!names.length) return
  let got
  try { got = await get(`api/photos?n=${encodeURIComponent(JSON.stringify(names))}`) } catch { return }
  let any = false
  const wait = []
  for (const n of names) {
    const v = got ? got[n] : undefined
    if (typeof v === 'string') { PHOTO.set(n, v); any = true } else if (v === null && tries < 5) wait.push(n); else PHOTO.set(n, false)
  }
  if (any) render()
  // Being fetched: asked again at 5, 10, 20, 40, 80 s.
  if (wait.length) setTimeout(() => askPhotos(wait, tries + 1), 5000 * 2 ** tries)
}
// The switch turned: every name is asked afresh.
document.addEventListener('click', (e) => {
  if (e.target.closest && e.target.closest('[data-set="people_photos"]')) setTimeout(() => { PHOTO.clear(); render() }, 400)
})
/** The class and style a face takes for its photo: ` ph` and `--ph:url(…)`, or nothing. */
function photoFace(name) {
  const e = photoOf(name)
  // From the root: a url() in a custom property is read against the stylesheet (assets/), not the page.
  return e ? { cls: ' ph', style: `--ph:url(&quot;/api/photo?e=${encodeURIComponent(e)}&quot;)` } : { cls: '', style: '' }
}
