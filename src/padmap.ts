// Traduction entre la manette vue par le navigateur (disposition « standard » de l'API Gamepad)
// et les codes SDL que melonDS enregistre dans [Instance0.Joystick].
// Encodage melonDS : bouton = numéro ; croix = 0x100 | (n° << 4) | direction ;
// axe = 0x10000 | (n° << 24) | (sens << 20) | 0xFFFF, sens 0 positif, 1 négatif, 2 gâchette.
// Les tables supposent les pilotes par défaut de SDL 2 sous Windows : XInput pour les manettes Xbox,
// HIDAPI pour les manettes PlayStation (ordre des boutons de SDL_GameController).

export type PadType = "xbox" | "ps" | "other";

/** Ce que l'utilisateur a pressé : un bouton standard, ou un stick dans un sens. */
export type PadInput = { button: number } | { axis: number; dir: 1 | -1 };

const btn = (n: number) => n;
const hat = (dir: number) => 0x100 | dir;
const axis = (n: number, mode: 0 | 1 | 2) => 0x10000 | (n << 24) | (mode << 20) | 0xffff;

// Bouton standard (index) vers code SDL
const XBOX: number[] = [
  btn(0), btn(1), btn(2), btn(3), btn(4), btn(5), axis(4, 2), axis(5, 2),
  btn(6), btn(7), btn(8), btn(9), hat(1), hat(4), hat(8), hat(2), btn(10),
];
const PS: number[] = [
  btn(0), btn(1), btn(2), btn(3), btn(9), btn(10), axis(4, 2), axis(5, 2),
  btn(4), btn(6), btn(7), btn(8), btn(11), btn(12), btn(13), btn(14), btn(5),
];

const NAMES: Record<PadType, string[]> = {
  xbox: ["A", "B", "X", "Y", "LB", "RB", "LT", "RT", "Back", "Start", "Clic stick gauche", "Clic stick droit",
    "Flèche haut", "Flèche bas", "Flèche gauche", "Flèche droite", "Xbox"],
  ps: ["Croix", "Rond", "Carré", "Triangle", "L1", "R1", "L2", "R2", "Share", "Options", "L3", "R3",
    "Flèche haut", "Flèche bas", "Flèche gauche", "Flèche droite", "PS"],
  other: ["Bas", "Droite", "Gauche", "Haut", "Épaule gauche", "Épaule droite", "Gâchette gauche", "Gâchette droite",
    "Select", "Start", "Clic stick gauche", "Clic stick droit", "Flèche haut", "Flèche bas", "Flèche gauche", "Flèche droite", "Accueil"],
};

const STICKS = ["Stick gauche", "Stick gauche", "Stick droit", "Stick droit"];

const table = (t: PadType) => (t === "ps" ? PS : XBOX);

/** Type de manette d'après le nom donné par Windows. */
export function padType(id: string): PadType {
  const s = id.toLowerCase();
  // Xbox d'abord : « Xbox Wireless Controller » contient aussi « wireless controller »
  if (/xinput|xbox|045e/.test(s)) return "xbox";
  if (/054c|dualsense|dualshock|playstation|wireless controller/.test(s)) return "ps";
  return "other";
}

export const PAD_TYPE_LABEL: Record<PadType, string> = {
  ps: "Manette PlayStation",
  xbox: "Manette Xbox",
  other: "Manette non reconnue (traitée comme une manette Xbox)",
};

/** Code SDL enregistré par melonDS pour ce que l'utilisateur a pressé. */
export function toSdl(t: PadType, input: PadInput): number | null {
  if ("button" in input) return table(t)[input.button] ?? null;
  if (input.axis > 3) return null;
  return axis(input.axis, input.dir > 0 ? 0 : 1);
}

/** Nom lisible d'un code SDL, selon le type de manette. Un code peut combiner un bouton et un axe. */
export function sdlName(t: PadType, code: number): string {
  if (code < 0) return "Non assignée";
  if (code & 0x10000 && (code & 0xffff) !== 0xffff) {
    return `${sdlName(t, code & 0xffff)} + ${sdlName(t, (code & ~0xffff) | 0xffff)}`;
  }
  const i = table(t).indexOf(code);
  if (i >= 0) return NAMES[t][i];
  if (code & 0x10000) {
    const n = (code >> 24) & 0xf;
    const mode = (code >> 20) & 0xf;
    if (n <= 3) {
      const vertical = n % 2 === 1;
      const dir = mode === 0 ? (vertical ? "vers le bas" : "vers la droite") : vertical ? "vers le haut" : "vers la gauche";
      return `${STICKS[n]} ${dir}`;
    }
    return `Axe ${n}`;
  }
  if (code & 0x100) return `Croix ${code & 0xf}`;
  return `Bouton ${code & 0xffff}`;
}

export type Directions = "dpad" | "stick" | "both";

// Stick gauche : haut, bas, gauche, droite
const STICK_DIRS = [axis(1, 1), axis(1, 0), axis(0, 1), axis(0, 0)];

/** Une direction : la flèche, le stick, ou les deux dans la même touche (melonDS lit les deux parties). */
function direction(t: PadType, i: number, d: Directions): number {
  const arrow = table(t)[12 + i];
  const stick = STICK_DIRS[i];
  if (d === "dpad") return arrow;
  if (d === "stick") return stick;
  return (stick & ~0xffff) | (arrow & 0xffff);
}

/** Disposition conseillée : les boutons de la DS à la même place sur la manette. */
export function suggested(t: PadType, d: Directions = "both"): Record<string, number> {
  const m = table(t);
  return {
    A: m[1], B: m[0], X: m[3], Y: m[2], L: m[4], R: m[5], Start: m[9], Select: m[8],
    Up: direction(t, 0, d), Down: direction(t, 1, d), Left: direction(t, 2, d), Right: direction(t, 3, d),
    HK_FastForward: m[7], HK_Pause: -1, HK_FullscreenToggle: -1, HK_Reset: -1, HK_SwapScreens: -1,
  };
}

/** Lit ce qui est pressé sur la première manette branchée (boutons et sticks). */
export function readPad(): { id: string; pressed: PadInput[] } | null {
  const pad = navigator.getGamepads().find((p) => p?.connected);
  if (!pad) return null;
  const pressed: PadInput[] = [];
  pad.buttons.forEach((b, i) => {
    if (b.pressed || b.value > 0.5) pressed.push({ button: i });
  });
  pad.axes.slice(0, 4).forEach((v, i) => {
    if (Math.abs(v) > 0.6) pressed.push({ axis: i, dir: v > 0 ? 1 : -1 });
  });
  return { id: pad.id, pressed };
}
