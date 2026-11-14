// Consoles affichées dans le menu d'accueil. Seules celles qui ont un connecteur sont jouables.
// Les dessins sont des tracés simples, faits pour VemuZ (aucun logo ni image officielle).

export type ConsoleInfo = {
  id: string;
  name: string;
  year: number;
  /** Émulateur utilisé, absent si la console n'est pas encore prise en charge */
  emulator?: string;
  /** Tracés SVG dans un cadre 64 x 48 */
  draw: string;
};

export const CONSOLES: ConsoleInfo[] = [
  {
    id: "nes",
    name: "NES",
    year: 1985,
    draw: `<rect x="6" y="12" width="52" height="26" rx="3"/><path d="M12 26h40M12 31h12"/><rect x="40" y="29" width="5" height="4" rx="1"/><rect x="48" y="29" width="5" height="4" rx="1"/>`,
  },
  {
    id: "gb",
    name: "Game Boy",
    year: 1989,
    draw: `<rect x="18" y="4" width="28" height="40" rx="3"/><rect x="22" y="8" width="20" height="15" rx="1"/><path d="M25 31v6M22 34h6"/><circle cx="37" cy="35" r="1.8"/><circle cx="41" cy="32" r="1.8"/>`,
  },
  {
    id: "snes",
    name: "Super Nintendo",
    year: 1990,
    draw: `<path d="M8 20q0-6 6-6h36q6 0 6 6v12q0 6-6 6H14q-6 0-6-6z"/><rect x="22" y="10" width="20" height="8" rx="1.5"/><circle cx="17" cy="29" r="2"/><circle cx="47" cy="29" r="2"/>`,
  },
  {
    id: "n64",
    name: "Nintendo 64",
    year: 1996,
    draw: `<path d="M6 34l6-16h40l6 16v4H6z"/><path d="M24 18v-6h16v6"/><circle cx="16" cy="31" r="2"/><circle cx="48" cy="31" r="2"/>`,
  },
  {
    id: "gba",
    name: "Game Boy Advance",
    year: 2001,
    draw: `<path d="M4 18q0-6 6-6h44q6 0 6 6v12q0 6-6 6H10q-6 0-6-6z"/><rect x="20" y="15" width="24" height="16" rx="1"/><path d="M11 21v6M8 24h6"/><circle cx="51" cy="22" r="1.8"/><circle cx="54" cy="26" r="1.8"/>`,
  },
  {
    id: "gc",
    name: "GameCube",
    year: 2002,
    draw: `<rect x="14" y="10" width="36" height="32" rx="3"/><circle cx="32" cy="24" r="8"/><circle cx="32" cy="24" r="2"/><path d="M22 10V6h20v4"/>`,
  },
  {
    id: "ds",
    name: "Nintendo DS",
    year: 2005,
    emulator: "melonDS",
    draw: `<rect x="14" y="3" width="36" height="20" rx="2.5"/><rect x="21" y="6" width="22" height="14" rx="1"/><rect x="12" y="25" width="40" height="20" rx="2.5"/><rect x="23" y="28" width="18" height="13" rx="1"/><path d="M16 33v5M13.5 35.5h5"/><circle cx="47" cy="33" r="1.4"/><circle cx="47" cy="38" r="1.4"/>`,
  },
  {
    id: "wii",
    name: "Wii",
    year: 2006,
    draw: `<rect x="24" y="4" width="16" height="40" rx="2.5"/><path d="M28 10h8"/><circle cx="32" cy="38" r="1.5"/><rect x="46" y="14" width="6" height="26" rx="3"/><circle cx="49" cy="19" r="1.2"/>`,
  },
  {
    id: "3ds",
    name: "Nintendo 3DS",
    year: 2011,
    draw: `<rect x="10" y="3" width="44" height="20" rx="2.5"/><rect x="17" y="6" width="30" height="14" rx="1"/><rect x="12" y="25" width="40" height="20" rx="2.5"/><rect x="23" y="28" width="18" height="13" rx="1"/><circle cx="16.5" cy="31" r="2.5"/><path d="M16.5 37v4M14.5 39h4"/><circle cx="47" cy="33" r="1.4"/><circle cx="47" cy="38" r="1.4"/>`,
  },
];
