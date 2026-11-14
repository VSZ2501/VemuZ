// Format des touches propre à chaque émulateur : capture, affichage et valeurs conseillées.
// Le backend indique le format de chaque connecteur (EmuInfo.bindings).

import { SUGGESTED, qtName, toQt } from "./keys";
import { sdlName, suggested as sdlSuggested, toSdl, type Directions, type PadInput, type PadType } from "./padmap";
import { VK_SUGGESTED, diName, diSuggested, toDi, toVk, vkName } from "./desmume_keys";

export type Codec = {
  keyFromEvent: (e: KeyboardEvent) => number | null;
  keyName: (code: number) => string;
  keySuggested: Record<string, number>;
  padFromInput: (t: PadType, input: PadInput) => number | null;
  padName: (t: PadType, code: number) => string;
  padSuggested: (t: PadType, d: Directions) => Record<string, number>;
  /** Clavier et manette partagent un seul emplacement par bouton */
  shared: boolean;
  /** Une même touche peut combiner la croix et le stick */
  bothDirections: boolean;
  /** Précision affichée sous les touches du clavier */
  keyNote?: string;
};

export const CODECS: Record<string, Codec> = {
  melonds: {
    keyFromEvent: toQt,
    keyName: qtName,
    keySuggested: SUGGESTED,
    padFromInput: toSdl,
    padName: sdlName,
    padSuggested: sdlSuggested,
    shared: false,
    bothDirections: true,
  },
  desmume: {
    keyFromEvent: toVk,
    keyName: vkName,
    keySuggested: VK_SUGGESTED,
    padFromInput: toDi,
    padName: diName,
    padSuggested: diSuggested,
    shared: true,
    bothDirections: false,
    keyNote:
      "L'avance rapide va aussi vite que ton PC le permet. Accélérer et Ralentir changent la vitesse du jeu par paliers (réglage Vitesse du jeu, dans Avancé).",
  },
};
