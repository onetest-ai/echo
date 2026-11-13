// ── Colleagues' photos in place of initials (`api/photos`, the app's `photos`): asked once per name, in batches; the
// app answers with the names it has a photo for and fetches the others in the background (only colleagues in the
// user's own domain, only when the Microsoft 365 sign-in may read photos), so a name not answered is asked once more.

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
async function askPhotos(again) {
  const names = again || [...photoAsk]
  photoAsk.clear()
  if (!names.length) return
  let got
  try { got = await get(`api/photos?n=${encodeURIComponent(JSON.stringify(names))}`) } catch { return }
  let any = false
  const miss = []
  for (const n of names) {
    if (got && typeof got[n] === 'string') { PHOTO.set(n, got[n]); any = true } else if (again) PHOTO.set(n, false); else miss.push(n)
  }
  if (any) render()
  // The app fetches the ones it hasn't yet: asked once more a little later.
  if (miss.length) setTimeout(() => askPhotos(miss), 6000)
}
/** The class and style a face takes for its photo: ` ph` and `--ph:url(…)`, or nothing. */
function photoFace(name) {
  const e = photoOf(name)
  // From the root: a url() in a custom property is read against the stylesheet (assets/), not the page.
  return e ? { cls: ' ph', style: `--ph:url(&quot;/api/photo?e=${encodeURIComponent(e)}&quot;)` } : { cls: '', style: '' }
}
