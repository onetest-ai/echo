// ── Colleagues' photos in place of initials (`api/photos`, the app's `photos`): asked once per name, in batches; the
// app answers with the names it keeps a photo for, and fetches those of the user's calls and meetings it doesn't keep
// yet in the background (only colleagues in the user's own domain, only when the Microsoft 365 sign-in may read
// photos): a name waiting to be fetched (null) is asked again, a little later each time, up to every 80 s.

// ── photos
const PHOTO = new Map() // name → its photo's address, or false (none), or 0 (asked, waiting)
const photoAsk = new Set() // names asked from what the app keeps
const photoGet = new Set() // names the app may fetch: people of the user's calls and meetings
const photoGot = new Set() // names already asked so
let photoTimer = null
/** A name's photo address if the app has one, else null. `meet`: a person of the user's calls or meetings — their photo
 *  is fetched when not kept yet; anyone else's face shows one only once it is kept. */
function photoOf(name, meet) {
  if (!name) return null
  if (meet && !photoGot.has(name)) { photoGot.add(name); photoGet.add(name); if (!PHOTO.get(name)) PHOTO.set(name, 0) }
  else if (!PHOTO.has(name)) { PHOTO.set(name, 0); photoAsk.add(name) }
  else return PHOTO.get(name) || null
  clearTimeout(photoTimer)
  photoTimer = setTimeout(() => { const kept = [...photoAsk].filter((n) => !photoGet.has(n)); askPhotos([...photoGet], true); askPhotos(kept, false); photoGet.clear(); photoAsk.clear() }, 250)
  return PHOTO.get(name) || null
}
async function askPhotos(names, fetch, tries = 0) {
  if (!names.length) return
  const later = (them) => setTimeout(() => askPhotos(them, fetch, tries + 1), Math.min(5000 * 2 ** tries, 80000))
  let got
  try { got = await get(`api/photos?n=${encodeURIComponent(JSON.stringify(names))}${fetch ? '&fetch=1' : ''}`) } catch { return later(names) }
  let any = false
  const wait = []
  for (const n of names) {
    const v = got ? got[n] : undefined
    if (typeof v === 'string') { PHOTO.set(n, v); any = true } else if (v === null) wait.push(n); else PHOTO.set(n, false)
  }
  if (any) render()
  // Waiting to be fetched (in the queue, or out a pause): asked again at 5, 10, 20, 40, then every 80 s.
  if (wait.length) later(wait)
}
// The switch turned: every name is asked afresh.
document.addEventListener('click', (e) => {
  if (e.target.closest && e.target.closest('[data-set="people_photos"]')) setTimeout(() => { PHOTO.clear(); photoGot.clear(); render() }, 400)
})
/** The class and style a face takes for its photo: ` ph` and `--ph:url(…)`, or nothing. */
function photoFace(name, meet) {
  const e = photoOf(name, meet)
  // From the root: a url() in a custom property is read against the stylesheet (assets/), not the page.
  return e ? { cls: ' ph', style: `--ph:url(&quot;/api/photo?e=${encodeURIComponent(e)}&quot;)` } : { cls: '', style: '' }
}
