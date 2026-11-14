// Lecture de la manette via l'API Gamepad du navigateur (disposition standard).
// La boucle ne tourne que si une manette est branchée, la fenêtre a le focus et la page est visible :
// pendant un jeu (fenêtre réduite, melonDS au premier plan), rien ne tourne.

export type PadAction = "up" | "down" | "left" | "right" | "accept" | "back" | "prev" | "next" | "search" | "details";

const DIRECTIONS: PadAction[] = ["up", "down", "left", "right"];
const REPEAT_DELAY = 350;
const REPEAT_RATE = 120;
const DEADZONE = 0.5;

// Boutons de la disposition standard
const BUTTONS: [number, PadAction][] = [
  [0, "accept"],
  [1, "back"],
  [2, "details"],
  [3, "search"],
  [4, "prev"],
  [5, "next"],
  [12, "up"],
  [13, "down"],
  [14, "left"],
  [15, "right"],
];

function pressed(): Set<PadAction> {
  const set = new Set<PadAction>();
  for (const pad of navigator.getGamepads()) {
    if (!pad || !pad.connected) continue;
    for (const [i, action] of BUTTONS) if (pad.buttons[i]?.pressed) set.add(action);
    const [x = 0, y = 0] = pad.axes;
    if (x < -DEADZONE) set.add("left");
    if (x > DEADZONE) set.add("right");
    if (y < -DEADZONE) set.add("up");
    if (y > DEADZONE) set.add("down");
  }
  return set;
}

const hasPad = () => navigator.getGamepads().some((p) => p?.connected);
const active = () => !document.hidden && document.hasFocus();

/** Démarre l'écoute de la manette. Renvoie une fonction d'arrêt. */
export function startGamepad(onAction: (a: PadAction) => void): () => void {
  let frame = 0;
  // Action maintenue vers l'instant du prochain déclenchement (Infinity : pas de répétition)
  let held = new Map<PadAction, number>();

  function loop(now: number) {
    frame = 0;
    if (!active() || !hasPad()) return;
    const current = pressed();
    for (const a of [...held.keys()]) if (!current.has(a)) held.delete(a);
    for (const a of current) {
      const next = held.get(a);
      if (next === undefined) {
        held.set(a, DIRECTIONS.includes(a) ? now + REPEAT_DELAY : Infinity);
        onAction(a);
      } else if (now >= next) {
        held.set(a, now + REPEAT_RATE);
        onAction(a);
      }
    }
    frame = requestAnimationFrame(loop);
  }

  function start() {
    if (frame || !active() || !hasPad()) return;
    // Les boutons déjà enfoncés au retour (par exemple après un jeu) sont ignorés jusqu'au relâchement
    held = new Map([...pressed()].map((a) => [a, Infinity]));
    frame = requestAnimationFrame(loop);
  }

  function stop() {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
  }

  const onVisibility = () => (document.hidden ? stop() : start());
  window.addEventListener("gamepadconnected", start);
  window.addEventListener("focus", start);
  window.addEventListener("blur", stop);
  document.addEventListener("visibilitychange", onVisibility);
  start();

  return () => {
    stop();
    window.removeEventListener("gamepadconnected", start);
    window.removeEventListener("focus", start);
    window.removeEventListener("blur", stop);
    document.removeEventListener("visibilitychange", onVisibility);
  };
}

type Dir = "up" | "down" | "left" | "right";

/** Élément focalisable le plus proche dans la direction donnée, d'après la position à l'écran. */
export function nearest(from: Element, candidates: HTMLElement[], dir: Dir): HTMLElement | null {
  const a = from.getBoundingClientRect();
  const ax = a.left + a.width / 2;
  const ay = a.top + a.height / 2;
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  for (const el of candidates) {
    if (el === from) continue;
    const b = el.getBoundingClientRect();
    const bx = b.left + b.width / 2;
    const by = b.top + b.height / 2;
    // Distance dans l'axe demandé (bords), décalage sur l'autre axe (centres),
    // et chevauchement sur l'autre axe (même colonne ou même ligne)
    const vertical = dir === "up" || dir === "down";
    let main: number;
    if (dir === "down") main = b.top - a.bottom;
    else if (dir === "up") main = a.top - b.bottom;
    else if (dir === "right") main = b.left - a.right;
    else main = a.left - b.right;
    if (main < -2) continue;
    const side = vertical ? Math.abs(bx - ax) : Math.abs(by - ay);
    const overlap = vertical ? b.left < a.right && b.right > a.left : b.top < a.bottom && b.bottom > a.top;
    // Un élément aligné passe toujours avant un élément en diagonale
    const score = overlap ? Math.max(main, 0) + side * 0.1 : 100000 + Math.max(main, 0) + side * 2;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  return best;
}
