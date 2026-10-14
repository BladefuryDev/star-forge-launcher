import { invoke } from './bridge.js';

let finished = false;
const finish = () => {
  if (finished) return;
  finished = true;
  invoke('splash_finished').catch((error) => console.error('Splash completion failed', error));
};
const sting = document.getElementById('sting');
const emblem = document.getElementById('sting-emblem');
const wordmark = document.getElementById('sting-wordmark');
if (
  matchMedia('(prefers-reduced-motion: reduce)').matches ||
  !sting ||
  !(emblem instanceof HTMLImageElement) ||
  !(wordmark instanceof HTMLImageElement)
) {
  finish();
} else {
  for (const image of [emblem, wordmark]) {
    image.addEventListener('error', finish, { once: true });
    if (image.complete && image.naturalWidth === 0) finish();
  }
  if (!finished) {
    const centerOffset =
      (sting.getBoundingClientRect().width - emblem.getBoundingClientRect().width) / 2;
    emblem.animate(
      [
        { opacity: 0, transform: `translateX(${centerOffset}px) scale(.12)`, offset: 0 },
        { opacity: 1, transform: `translateX(${centerOffset}px) scale(.12)`, offset: 0.1 },
        { opacity: 1, transform: `translateX(${centerOffset}px) scale(1)`, offset: 0.48 },
        { opacity: 1, transform: 'translateX(0) scale(1)', offset: 1 },
      ],
      { duration: 1000, easing: 'cubic-bezier(.2,.7,.25,1)', fill: 'forwards' },
    );
    wordmark.animate(
      [
        { opacity: 0, clipPath: 'inset(0 100% 0 0)', transform: 'translateX(-10px)' },
        { opacity: 1, clipPath: 'inset(0 0 0 0)', transform: 'translateX(0)' },
      ],
      { delay: 720, duration: 450, easing: 'ease-out', fill: 'forwards' },
    );
    const fade = sting.animate([{ opacity: 1 }, { opacity: 0 }], {
      delay: 1950,
      duration: 350,
      easing: 'ease-in-out',
      fill: 'forwards',
    });
    // Only the complete brand's fade releases the launcher, not either child animation.
    fade.addEventListener('finish', finish, { once: true });
    setTimeout(finish, 3000);
  }
}
