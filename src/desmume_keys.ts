// Codes de touches de DeSmuME (version Windows), enregistrés dans [Controls] et [Hotkeys] de desmume.ini.
// Clavier : code de touche virtuelle Windows (VK), qui correspond à KeyboardEvent.keyCode.
// Manette : 0x8000 | (n° de manette << 8) | code DirectInput ; bouton n = 8 + n, axe X 0 et 1, axe Y 2 et 3,
// croix 4 gauche, 5 droite, 6 haut, 7 bas, axe Z 41 et 42. DeSmuME lit la manette avec DirectInput.

import type { Directions, PadInput, PadType } from "./padmap";

const JOY = 0x8000;

const VK_NAMES: Record<number, string> = {
  8: "Retour arrière",
  9: "Tab",
  13: "Entrée",
  16: "Maj",
  17: "Ctrl",
  18: "Alt",
  19: "Pause",
  20: "Verr. maj",
  27: "Échap",
  32: "Espace",
  33: "Page haut",
  34: "Page bas",
  35: "Fin",
  36: "Début",
  37: "Flèche gauche",
  38: "Flèche haut",
  39: "Flèche droite",
  40: "Flèche bas",
  45: "Inser",
  46: "Suppr",
  106: "Pavé num. *",
  107: "Pavé num. +",
  109: "Pavé num. -",
  110: "Pavé num. .",
  111: "Pavé num. /",
  160: "Maj gauche",
  161: "Maj droite",
  162: "Ctrl gauche",
  163: "Ctrl droit",
  164: "Alt gauche",
  165: "Alt droit",
  // Touches de ponctuation : libellé du clavier américain, la position varie selon la disposition
  186: "Touche ;",
  187: "Touche =",
  188: "Touche ,",
  189: "Touche -",
  190: "Touche .",
  191: "Touche /",
  192: "Touche `",
  219: "Touche [",
  220: "Touche \\",
  221: "Touche ]",
  222: "Touche '",
};
for (let i = 0; i <= 9; i++) VK_NAMES[96 + i] = `Pavé num. ${i}`;
for (let i = 1; i <= 12; i++) VK_NAMES[111 + i] = `F${i}`;

/** Code de touche Windows d'un appui, ou null si la touche n'est pas prise en charge. */
export function toVk(e: KeyboardEvent): number | null {
  const k = e.keyCode;
  return k > 0 && k < 0xff ? k : null;
}

/** Nom lisible d'un code de touche Windows. */
export function vkName(code: number): string {
  if (code <= 0) return "Non assignée";
  if (VK_NAMES[code]) return VK_NAMES[code];
  if ((code >= 48 && code <= 57) || (code >= 65 && code <= 90)) return String.fromCharCode(code);
  return `Touche ${code}`;
}

/** Disposition conseillée au clavier, proche de celle proposée pour melonDS. */
export const VK_SUGGESTED: Record<string, number> = {
  A: 88, // X
  B: 90, // Z
  X: 83, // S
  Y: 65, // A
  L: 81, // Q
  R: 87, // W
  Start: 13,
  Select: 8,
  Up: 38,
  Down: 40,
  Left: 37,
  Right: 39,
  HK_Pause: 19, // Pause
  HK_FastForward: 9, // Tab
  HK_SwapScreens: 34, // Page bas
  HK_SpeedUp: 107, // + du pavé numérique
  HK_SpeedDown: 109, // - du pavé numérique
};

const di = (code: number) => JOY | code;
const button = (n: number) => di(8 + n);

// Bouton standard de l'API Gamepad (index) vers code DirectInput, -1 si DirectInput ne le voit pas.
// Xbox : A B X Y LB RB Back Start LS RS = boutons 0 à 9, gâchettes sur l'axe Z, croix sur le chapeau.
const XBOX: number[] = [
  button(0), button(1), button(2), button(3), button(4), button(5), di(42), di(41),
  button(6), button(7), button(8), button(9), di(6), di(7), di(4), di(5), -1,
];
// PlayStation : Carré Croix Rond Triangle L1 R1 L2 R2 Share Options L3 R3 PS = boutons 0 à 12
const PS: number[] = [
  button(1), button(2), button(0), button(3), button(4), button(5), button(6), button(7),
  button(8), button(9), button(10), button(11), di(6), di(7), di(4), di(5), button(12),
];

const table = (t: PadType) => (t === "ps" ? PS : XBOX);

const PAD_NAMES: Record<PadType, string[]> = {
  xbox: ["A", "B", "X", "Y", "LB", "RB", "LT", "RT", "Back", "Start", "Clic stick gauche", "Clic stick droit",
    "Flèche haut", "Flèche bas", "Flèche gauche", "Flèche droite", "Xbox"],
  ps: ["Croix", "Rond", "Carré", "Triangle", "L1", "R1", "L2", "R2", "Share", "Options", "L3", "R3",
    "Flèche haut", "Flèche bas", "Flèche gauche", "Flèche droite", "PS"],
  other: ["Bas", "Droite", "Gauche", "Haut", "Épaule gauche", "Épaule droite", "Gâchette gauche", "Gâchette droite",
    "Select", "Start", "Clic stick gauche", "Clic stick droit", "Flèche haut", "Flèche bas", "Flèche gauche", "Flèche droite", "Accueil"],
};

// Stick gauche : axes X et Y, seul stick lu de façon fiable par DeSmuME
const STICK: Record<number, string> = {
  0: "Stick gauche vers la gauche",
  1: "Stick gauche vers la droite",
  2: "Stick gauche vers le haut",
  3: "Stick gauche vers le bas",
};

/** Code DirectInput pour ce que l'utilisateur a pressé. */
export function toDi(t: PadType, input: PadInput): number | null {
  if ("button" in input) {
    const c = table(t)[input.button];
    return c === undefined || c < 0 ? null : c;
  }
  if (input.axis === 0) return di(input.dir > 0 ? 1 : 0);
  if (input.axis === 1) return di(input.dir > 0 ? 3 : 2);
  return null;
}

/** Nom lisible d'un code de manette DeSmuME. */
export function diName(t: PadType, code: number): string {
  if (code < JOY) return "Non assignée";
  const pad = (code >> 8) & 0xf;
  const i = table(t).indexOf(code & ~0xf00);
  const base = code & 0xff;
  const name =
    i >= 0 ? PAD_NAMES[t][i] : STICK[base] ?? (base >= 8 && base <= 40 ? `Bouton ${base - 7}` : `Commande ${base}`);
  return pad > 0 ? `${name} (manette ${pad + 1})` : name;
}

/** Disposition conseillée : les boutons de la DS à la même place sur la manette.
 *  DeSmuME ne garde qu'une commande par bouton : « Les deux » revient à la croix. */
export function diSuggested(t: PadType, d: Directions): Record<string, number> {
  const m = table(t);
  const dirs = d === "stick" ? [di(2), di(3), di(0), di(1)] : [m[12], m[13], m[14], m[15]];
  return {
    A: m[1], B: m[0], X: m[3], Y: m[2], L: m[4], R: m[5], Start: m[9], Select: m[8],
    Up: dirs[0], Down: dirs[1], Left: dirs[2], Right: dirs[3],
  };
}
