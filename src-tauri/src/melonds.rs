//! Connecteur melonDS (Nintendo DS). Le fichier melonDS.toml est lu et écrit par `melonds_config`.

use crate::emulator::{
    scan, usual_bases, Binding, Bindings, Emulator, Kind, LaunchOptions, Preset, SettingDef, Values,
};
use crate::melonds_config::{self as cfg, EmuConfig};
use crate::saves::SaveLayout;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub struct MelonDs;

impl MelonDs {
    #[cfg(windows)]
    pub const EXE: &'static str = "melonDS.exe";
    #[cfg(not(windows))]
    pub const EXE: &'static str = "melonDS";
}

fn config(dir: &Path) -> Result<PathBuf, String> {
    cfg::config_path(dir).ok_or_else(|| {
        "Configuration de melonDS introuvable : ouvre melonDS une fois, puis ferme-le pour qu'il la crée.".to_string()
    })
}

fn to_values(c: &EmuConfig) -> Values {
    match serde_json::to_value(c) {
        Ok(Value::Object(m)) => m,
        _ => Values::new(),
    }
}

/// Valeurs d'un préréglage, depuis un objet JSON écrit à la main.
fn values(v: Value) -> Values {
    match v {
        Value::Object(m) => m,
        _ => Values::new(),
    }
}

fn def(id: &'static str, group: &'static str, label: &'static str, help: Option<&'static str>, kind: Kind) -> SettingDef {
    SettingDef { id, label, help, group, advanced: false, kind }
}

/// Réglage pointu, affiché dans la section Avancé.
fn adv(id: &'static str, label: &'static str, help: Option<&'static str>, kind: Kind) -> SettingDef {
    SettingDef { id, label, help, group: "Avancé", advanced: true, kind }
}

impl Emulator for MelonDs {
    fn id(&self) -> &'static str {
        "melonds"
    }

    fn name(&self) -> &'static str {
        "melonDS"
    }

    fn console(&self) -> &'static str {
        "ds"
    }

    fn executable(&self, dir: &Path) -> Option<PathBuf> {
        let exe = dir.join(Self::EXE);
        exe.is_file().then_some(exe)
    }

    fn launch(&self, dir: &Path, rom: Option<&Path>, options: &LaunchOptions) -> Result<Child, String> {
        let exe = self
            .executable(dir)
            .ok_or_else(|| format!("{} introuvable dans {}", Self::EXE, dir.display()))?;
        let mut cmd = Command::new(exe);
        cmd.current_dir(dir);
        if options.fullscreen {
            cmd.arg("-f");
        }
        if let Some(rom) = rom {
            cmd.arg(rom);
        }
        cmd.spawn().map_err(|e| format!("Lancement impossible : {e}"))
    }

    fn detect(&self) -> Vec<PathBuf> {
        scan(self, &usual_bases(), "melonds")
    }

    fn download_url(&self) -> Option<&'static str> {
        Some("https://melonds.kuribo64.net/downloads.php")
    }

    fn settings_schema(&self) -> Vec<SettingDef> {
        use Kind::*;
        const SCREENS: &str = "Écrans";
        const GRAPHICS: &str = "Graphismes";
        const AUDIO: &str = "Son et langue";
        vec![
            def(
                "screen_layout",
                SCREENS,
                "Disposition",
                None,
                Choice {
                    options: vec![
                        (json!(0), "Naturelle"),
                        (json!(1), "Verticale (haut et bas)"),
                        (json!(2), "Horizontale (côte à côte)"),
                        (json!(3), "Hybride (un grand écran)"),
                    ],
                },
            ),
            def("screen_swap", SCREENS, "Échanger les deux écrans", None, Toggle),
            def("screen_filter", SCREENS, "Adoucir l'image", None, Toggle),
            def(
                "fast_renderer",
                GRAPHICS,
                "Rendu 3D",
                Some("Rapide : utilise la carte graphique. Compatible : plus lent, mais fonctionne partout."),
                Choice { options: vec![(json!(false), "Compatible (logiciel)"), (json!(true), "Rapide (OpenGL)")] },
            ),
            def(
                "resolution",
                GRAPHICS,
                "Définition de la 3D",
                Some("Rend les jeux en 3D plus nets. Seulement avec le rendu rapide."),
                Choice {
                    options: vec![
                        (json!(1), "D'origine"),
                        (json!(2), "x2"),
                        (json!(3), "x3"),
                        (json!(4), "x4"),
                        (json!(6), "x6"),
                        (json!(8), "x8"),
                    ],
                },
            ),
            def("volume", AUDIO, "Volume", None, Range { min: 0, max: 100, step: 1, unit: "%" }),
            def(
                "language",
                AUDIO,
                "Langue des jeux",
                Some("Les jeux disponibles en plusieurs langues utilisent celle-ci."),
                Choice {
                    options: vec![
                        (json!(1), "Anglais"),
                        (json!(2), "Français"),
                        (json!(3), "Allemand"),
                        (json!(4), "Italien"),
                        (json!(5), "Espagnol"),
                        (json!(0), "Japonais"),
                    ],
                },
            ),
            adv(
                "jit",
                "Accélération JIT",
                Some("Rend l'émulation bien plus rapide. À désactiver si un jeu plante."),
                Toggle,
            ),
            adv("integer_scaling", "Agrandissement sans flou", Some("Multiples entiers de la taille d'origine."), Toggle),
            adv("limit_fps", "Limiter à la vitesse normale", Some("60 images par seconde."), Toggle),
            adv(
                "fast_forward",
                "Vitesse de l'avance rapide",
                Some(
                    "S'applique tant que la touche d'avance rapide est enfoncée. 100 % est la vitesse normale ; si ton ordinateur ne suit pas, la vitesse reste limitée par sa puissance.",
                ),
                Range { min: 100, max: 1000, step: 25, unit: "%" },
            ),
            adv("pause_on_blur", "Mettre en pause quand la fenêtre perd le focus", None, Toggle),
            adv(
                "direct_boot",
                "Lancer directement le jeu",
                Some(
                    "Saute le menu de la console. Décoché, melonDS démarre via le firmware, un fichier de ta propre console que VemuZ ne fournit pas.",
                ),
                Toggle,
            ),
        ]
    }

    fn presets(&self) -> Vec<Preset> {
        vec![
            Preset {
                id: "recommended",
                label: "Conseillé",
                help: "Fonctionne sur tous les PC, image d'origine.",
                values: values(json!({
                    "fast_renderer": false, "resolution": 1, "jit": false,
                    "screen_filter": true, "limit_fps": true,
                })),
            },
            Preset {
                id: "performance",
                label: "Performance",
                help: "Pour un PC modeste : rendu rapide et accélération JIT.",
                values: values(json!({
                    "fast_renderer": true, "resolution": 1, "jit": true,
                    "screen_filter": false, "limit_fps": true,
                })),
            },
            Preset {
                id: "quality",
                label: "Qualité",
                help: "3D quatre fois plus nette, demande une carte graphique correcte.",
                values: values(json!({
                    "fast_renderer": true, "resolution": 4, "jit": true,
                    "screen_filter": true, "limit_fps": true,
                })),
            },
        ]
    }

    fn read_settings(&self, dir: &Path) -> Result<Values, String> {
        Ok(to_values(&cfg::read(&config(dir)?)?))
    }

    fn write_settings(&self, dir: &Path, values: &Values) -> Result<Values, String> {
        let path = config(dir)?;
        // Les valeurs absentes gardent celles du fichier
        let mut merged = to_values(&cfg::read(&path)?);
        merged.extend(values.clone());
        let c: EmuConfig = serde_json::from_value(Value::Object(merged)).map_err(|_| "Réglage invalide.".to_string())?;
        cfg::write(&path, &c)?;
        self.read_settings(dir)
    }

    fn bindings_format(&self) -> Option<&'static str> {
        Some("melonds")
    }

    fn read_bindings(&self, dir: &Path, kind: Binding) -> Result<Bindings, String> {
        let path = config(dir)?;
        match kind {
            Binding::Keyboard => cfg::read_keys(&path),
            Binding::Pad => cfg::read_joy(&path),
        }
    }

    fn write_bindings(&self, dir: &Path, kind: Binding, keys: &Bindings) -> Result<Bindings, String> {
        let path = config(dir)?;
        match kind {
            Binding::Keyboard => cfg::write_keys(&path, keys)?,
            Binding::Pad => cfg::write_joy(&path, keys)?,
        }
        self.read_bindings(dir, kind)
    }

    fn save_layout(&self, dir: &Path) -> SaveLayout {
        let (save_dir, state_dir) = cfg::config_path(dir).map(|c| cfg::asset_dirs(&c, dir)).unwrap_or((None, None));
        SaveLayout { save_dir, state_dir, save_ext: "sav", state_ext: "ml" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Values {
        to_values(&EmuConfig {
            screen_layout: 0,
            screen_swap: false,
            integer_scaling: false,
            screen_filter: true,
            fast_renderer: false,
            resolution: 1,
            limit_fps: true,
            jit: false,
            volume: 100,
            language: 1,
            direct_boot: true,
            pause_on_blur: false,
            fast_forward: 200,
        })
    }

    #[test]
    fn chaque_reglage_correspond_a_un_champ() {
        let sample = sample();
        let schema = MelonDs.settings_schema();
        assert_eq!(schema.len(), sample.len(), "un réglage affiché par champ");
        for s in schema {
            assert!(sample.contains_key(s.id), "champ absent : {}", s.id);
        }
    }

    #[test]
    fn prereglages_valides() {
        for p in MelonDs.presets() {
            let mut merged = sample();
            merged.extend(p.values.clone());
            assert!(serde_json::from_value::<EmuConfig>(Value::Object(merged)).is_ok(), "{} invalide", p.id);
        }
    }
}
