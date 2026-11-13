// Conversion entre les touches du navigateur et les codes de touche Qt utilisés par melonDS.

const KEYPAD = 0x20000000; // modificateur Qt du pavé numérique

// Touches nommées (KeyboardEvent.key) vers code Qt
const NAMED: Record<string, number> = {
  Escape: 0x01000000,
  Tab: 0x01000001,
  Backspace: 0x01000003,
  Enter: 0x01000004,
  Insert: 0x01000006,
  Delete: 0x01000007,
  Pause: 0x01000008,
  Home: 0x01000010,
  End: 0x01000011,
  ArrowLeft: 0x01000012,
  ArrowUp: 0x01000013,
  ArrowRight: 0x01000014,
  ArrowDown: 0x01000015,
  PageUp: 0x01000016,
  PageDown: 0x01000017,
  Shift: 0x01000020,
  Control: 0x01000021,
  Alt: 0x01000023,
  CapsLock: 0x01000024,
  " ": 0x20,
};
for (let i = 1; i <= 12; i++) NAMED[`F${i}`] = 0x01000030 + (i - 1);

const NAMES: Record<number, string> = {
  0x01000000: "Échap",
  0x01000001: "Tab",
  0x01000003: "Retour arrière",
  0x01000004: "Entrée",
  0x01000005: "Entrée",
  0x01000006: "Inser",
  0x01000007: "Suppr",
  0x01000008: "Pause",
  0x01000010: "Début",
  0x01000011: "Fin",
  0x01000012: "Flèche gauche",
  0x01000013: "Flèche haut",
  0x01000014: "Flèche droite",
  0x01000015: "Flèche bas",
  0x01000016: "Page haut",
  0x01000017: "Page bas",
  0x01000020: "Maj",
  0x01000021: "Ctrl",
  0x01000023: "Alt",
  0x01000024: "Verr. maj",
  0x20: "Espace",
};
for (let i = 1; i <= 12; i++) NAMES[0x01000030 + (i - 1)] = `F${i}`;

/** Code Qt d'un appui de touche, ou null si la touche n'est pas prise en charge. */
export function toQt(e: KeyboardEvent): number | null {
  const pad = e.location === 3 ? KEYPAD : 0;
  if (e.key === "Enter" && pad) return 0x01000005 | KEYPAD; // Entrée du pavé numérique
  if (NAMED[e.key] !== undefined) return NAMED[e.key] | pad;
  if (e.key.length === 1) {
    const code = e.key.toUpperCase().codePointAt(0)!;
    if (e.key.toUpperCase().length === 1 && code < 0x10000) return code | pad;
  }
  return null;
}

/** Nom lisible d'un code Qt. */
export function qtName(code: number): string {
  if (code < 0) return "Non assignée";
  const pad = (code & KEYPAD) !== 0;
  const base = code & ~KEYPAD;
  const name = NAMES[base] ?? (base > 0 && base < 0x10000 ? String.fromCodePoint(base) : `Touche ${code}`);
  return pad ? `Pavé num. ${name}` : name;
}

/** Disposition conseillée, proche des valeurs par défaut de melonDS. */
export const SUGGESTED: Record<string, number> = {
  A: 0x58, // X
  B: 0x5a, // Z
  X: 0x53, // S
  Y: 0x41, // A
  L: 0x51, // Q
  R: 0x57, // W
  Start: 0x01000004,
  Select: 0x01000003,
  Up: 0x01000013,
  Down: 0x01000015,
  Left: 0x01000012,
  Right: 0x01000014,
  HK_FullscreenToggle: 0x0100003a, // F11
  HK_Pause: 0x50, // P
  HK_FastForward: 0x01000001, // Tab
  HK_Reset: 0x01000034, // F5
  HK_SwapScreens: 0x0100003b, // F12
};

export const KEY_ROWS: { title: string; items: { name: string; label: string }[] }[] = [
  {
    title: "Boutons de la console",
    items: [
      { name: "Up", label: "Haut" },
      { name: "Down", label: "Bas" },
      { name: "Left", label: "Gauche" },
      { name: "Right", label: "Droite" },
      { name: "A", label: "A" },
      { name: "B", label: "B" },
      { name: "X", label: "X" },
      { name: "Y", label: "Y" },
      { name: "L", label: "L" },
      { name: "R", label: "R" },
      { name: "Start", label: "Start" },
      { name: "Select", label: "Select" },
    ],
  },
  {
    title: "Raccourcis",
    items: [
      { name: "HK_FullscreenToggle", label: "Plein écran" },
      { name: "HK_Pause", label: "Pause" },
      { name: "HK_FastForward", label: "Avance rapide" },
      { name: "HK_Reset", label: "Redémarrer le jeu" },
      { name: "HK_SwapScreens", label: "Échanger les écrans" },
    ],
  },
];
