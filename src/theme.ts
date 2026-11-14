// Thème de l'interface. Une copie locale du choix permet de l'appliquer avant le premier affichage
// (pas de flash sombre en mode clair) ; settings.json reste la référence.

export type Theme = "dark" | "light" | "system";

const KEY = "vemuz-theme";

/** Applique le thème donné (ou celui mémorisé si null) et renvoie le réglage retenu. */
export function applyTheme(theme: Theme): "dark" | "light";
export function applyTheme(theme: null): Theme;
export function applyTheme(theme: Theme | null): Theme | "dark" | "light" {
  if (theme === null) {
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(KEY);
    } catch {
      // stockage indisponible : thème sombre
    }
    const t: Theme = saved === "light" || saved === "system" ? saved : "dark";
    applyTheme(t);
    return t;
  }
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    // sans importance : settings.json fait foi
  }
  const shown = theme === "system" ? (matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark") : theme;
  document.documentElement.dataset.theme = shown;
  return shown;
}
