// Petits outils d'animation. Tout s'arrête si Windows demande moins d'animations.

import { cubicOut } from "svelte/easing";
import { fade, fly } from "svelte/transition";

export const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Durée réelle d'une animation : 0 si les animations sont réduites. */
export const dur = (ms: number) => (reduced() ? 0 : ms);

/** Glissement depuis un côté, avec fondu. */
export function slide(node: Element, { x = 0, y = 0, duration = 240 } = {}) {
  return fly(node, { x, y, duration: dur(duration), easing: cubicOut, opacity: 0 });
}

export function fadeIn(node: Element, { duration = 200 } = {}) {
  return fade(node, { duration: dur(duration) });
}

type Transition = { skipTransition: () => void; finished: Promise<void> };
let current: Transition | null = null;

/** Change de vue avec un fondu croisé si le moteur le permet. Un nouveau changement termine le précédent. */
export function withViewTransition(update: () => Promise<void> | void) {
  const doc = document as Document & { startViewTransition?: (cb: () => Promise<void> | void) => Transition };
  if (!doc.startViewTransition || reduced() || document.hidden) return update();
  current?.skipTransition();
  const t = doc.startViewTransition(update);
  current = t;
  // Filet de sécurité : pendant une transition, les clics ne passent pas ; on ne la laisse jamais traîner
  setTimeout(() => current === t && t.skipTransition(), 800);
  t.finished.finally(() => {
    if (current === t) current = null;
  });
}

/** Petite pause avant une action, sautée si les animations sont réduites. */
export const pause = (ms: number) => new Promise((r) => setTimeout(r, dur(ms)));
