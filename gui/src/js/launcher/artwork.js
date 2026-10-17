import { invoke } from './bridge.js';
import { object } from './contracts.js';

/** @param {unknown} value */
function image(value) {
  if (value === null) return null;
  if (typeof value !== 'string' || !/^data:image\/(png|jpeg);base64,[A-Za-z0-9+/=]+$/.test(value))
    throw new Error('Invalid artwork');
  return value;
}

/** Uses Star Conflict's banner from the local Steam library cache; the STAR FORGE logo stays as the wordmark. */
export async function bindArtwork() {
  const art = object(await invoke('get_artwork'));
  const hero = image(art.hero);
  if (hero)
    for (const node of document.querySelectorAll('.artwork, .game-card-art'))
      if (node instanceof HTMLElement) node.style.backgroundImage = `url("${hero}")`;
}
