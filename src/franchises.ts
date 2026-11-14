// Franchises connues sur Nintendo DS, déduites du titre. Le premier nom qui correspond l'emporte,
// d'où l'ordre : les plus précis avant les plus généraux. L'utilisateur peut corriger dans la fiche.

const LIST: [string, string[]][] = [
  ["Pokémon", ["pokemon", "pokepark"]],
  ["Professeur Layton", ["layton"]],
  ["Ace Attorney", ["ace attorney", "phoenix wright", "apollo justice", "miles edgeworth", "gyakuten"]],
  ["Zelda", ["zelda"]],
  ["Wario", ["wario", "warioware"]],
  ["Yoshi", ["yoshi"]],
  ["Donkey Kong", ["donkey kong", "diddy kong"]],
  ["Mario", ["mario", "luigi", "princess peach"]],
  ["Kirby", ["kirby"]],
  ["Metroid", ["metroid"]],
  ["Star Fox", ["star fox", "starfox"]],
  ["Animal Crossing", ["animal crossing"]],
  ["Fire Emblem", ["fire emblem"]],
  ["Advance Wars", ["advance wars"]],
  ["Picross", ["picross"]],
  ["Nintendogs", ["nintendogs"]],
  ["Programme d'entraînement cérébral", ["brain age", "brain training", "entrainement cerebral", "kawashima"]],
  ["Big Brain Academy", ["brain academy"]],
  ["Golden Sun", ["golden sun"]],
  ["Final Fantasy", ["final fantasy", "chocobo"]],
  ["Dragon Quest", ["dragon quest", "dragon warrior"]],
  ["Kingdom Hearts", ["kingdom hearts"]],
  ["Chrono Trigger", ["chrono trigger"]],
  ["Tales of", ["tales of"]],
  ["Shin Megami Tensei", ["megami tensei", "devil survivor", "persona"]],
  ["Etrian Odyssey", ["etrian odyssey"]],
  ["Inazuma Eleven", ["inazuma eleven"]],
  ["Sonic", ["sonic"]],
  ["Castlevania", ["castlevania"]],
  ["Mega Man", ["mega man", "megaman", "rockman"]],
  ["Contra", ["contra"]],
  ["Ninja Gaiden", ["ninja gaiden"]],
  ["Bomberman", ["bomberman"]],
  ["Pac-Man", ["pac man", "pacman"]],
  ["Tetris", ["tetris"]],
  ["Puzzle Quest", ["puzzle quest"]],
  ["Harvest Moon", ["harvest moon", "story of seasons", "rune factory"]],
  ["Cooking Mama", ["cooking mama"]],
  ["Les Sims", ["sims"]],
  ["Rayman", ["rayman", "rabbids", "lapins cretins"]],
  ["Spyro", ["spyro"]],
  ["Crash Bandicoot", ["crash bandicoot", "crash of the titans", "crash nitro"]],
  ["Tony Hawk", ["tony hawk"]],
  ["Lego", ["lego"]],
  ["Star Wars", ["star wars"]],
  ["Harry Potter", ["harry potter"]],
  ["Disney", ["disney"]],
  ["Spider-Man", ["spider man", "spiderman"]],
  ["Batman", ["batman"]],
  ["Ben 10", ["ben 10"]],
  ["Naruto", ["naruto"]],
  ["Dragon Ball", ["dragon ball", "dragonball"]],
  ["One Piece", ["one piece"]],
  ["Bleach", ["bleach"]],
  ["Yu-Gi-Oh!", ["yu gi oh", "yugioh"]],
  ["Digimon", ["digimon"]],
  ["Trauma Center", ["trauma center"]],
  ["Dragon's Lair", ["dragon s lair"]],
];

/** Minuscules, sans accents, ponctuation remplacée par des espaces. */
export function normalize(s: string): string {
  return s
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

const PREPARED = LIST.map(([name, keys]) => [name, keys.map((k) => ` ${normalize(k)} `)] as const);

/** Franchise déduite du titre, ou null si aucune ne correspond. */
export function detectFranchise(title: string): string | null {
  const t = ` ${normalize(title)} `;
  for (const [name, keys] of PREPARED) if (keys.some((k) => t.includes(k))) return name;
  return null;
}

export const OTHERS = "Autres";
