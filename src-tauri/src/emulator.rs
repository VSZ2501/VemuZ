//! Interface commune à tous les émulateurs et registre des connecteurs.

use crate::desmume::DeSmuME;
use crate::melonds::MelonDs;
use crate::saves::SaveLayout;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;

/// Options communes de lancement, mémorisées par VemuZ pour chaque émulateur.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct LaunchOptions {
    #[serde(default)]
    pub fullscreen: bool,
}

/// Valeurs des réglages d'un émulateur, par identifiant de réglage.
pub type Values = serde_json::Map<String, serde_json::Value>;

/// Un réglage déclaré par un connecteur, affiché tel quel par l'interface.
#[derive(Serialize)]
pub struct SettingDef {
    pub id: &'static str,
    pub label: &'static str,
    pub help: Option<&'static str>,
    /// Titre de la section des Réglages
    pub group: &'static str,
    /// Réglage pointu, rangé dans la section Avancé repliée
    pub advanced: bool,
    #[serde(flatten)]
    pub kind: Kind,
}

/// Ensemble de valeurs appliqué en un clic (Conseillé, Performance, Qualité).
#[derive(Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    /// Seuls ces réglages changent
    pub values: Values,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Kind {
    Toggle,
    /// Liste de choix : (valeur, libellé), dans l'ordre d'affichage
    Choice { options: Vec<(serde_json::Value, &'static str)> },
    Range { min: i64, max: i64, step: i64, unit: &'static str },
}

/// Touches du clavier ou boutons de la manette.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Binding {
    Keyboard,
    Pad,
}

pub type Bindings = HashMap<String, i64>;

/// Interface commune à tous les émulateurs. Un connecteur par émulateur.
pub trait Emulator: Sync {
    /// Identifiant stable, utilisé dans settings.json
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    /// Console émulée (identifiant de consoles.ts)
    fn console(&self) -> &'static str;

    /// Chemin de l'exécutable dans le dossier d'installation, s'il existe.
    fn executable(&self, dir: &Path) -> Option<PathBuf>;
    /// Lance l'émulateur, avec une ROM ou seul (pour qu'il soit configuré à la main).
    fn launch(&self, dir: &Path, rom: Option<&Path>, options: &LaunchOptions) -> Result<Child, String>;

    /// Dossiers d'installation probables trouvés sur la machine.
    fn detect(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Page officielle de téléchargement, si l'émulateur en a une.
    fn download_url(&self) -> Option<&'static str> {
        None
    }

    /// Le dossier lui-même s'il contient l'exécutable, sinon le premier sous-dossier direct qui le contient
    /// (un zip décompressé crée souvent un dossier dans le dossier).
    fn resolve_dir(&self, dir: &Path) -> Option<PathBuf> {
        if self.executable(dir).is_some() {
            return Some(dir.to_path_buf());
        }
        let mut subs: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        subs.sort();
        subs.into_iter().find(|p| self.executable(p).is_some())
    }

    /// Réglages proposés dans VemuZ, dans l'ordre d'affichage.
    fn settings_schema(&self) -> Vec<SettingDef>;
    /// Préréglages proposés, le premier étant le conseillé.
    fn presets(&self) -> Vec<Preset> {
        Vec::new()
    }
    fn read_settings(&self, dir: &Path) -> Result<Values, String>;
    /// Écrit les réglages puis renvoie les valeurs relues.
    fn write_settings(&self, dir: &Path, values: &Values) -> Result<Values, String>;

    /// Format des codes de touches, pour que l'interface sache les capturer et les afficher.
    /// None : touches non réglables dans VemuZ.
    fn bindings_format(&self) -> Option<&'static str> {
        None
    }
    fn read_bindings(&self, _dir: &Path, _kind: Binding) -> Result<Bindings, String> {
        Err(format!("Les touches de {} ne se règlent pas encore dans VemuZ.", self.name()))
    }
    /// Écrit les touches puis renvoie les valeurs relues.
    fn write_bindings(&self, _dir: &Path, _kind: Binding, _keys: &Bindings) -> Result<Bindings, String> {
        Err(format!("Les touches de {} ne se règlent pas encore dans VemuZ.", self.name()))
    }

    /// Dossiers et extensions des sauvegardes de l'émulateur installé dans `dir`.
    fn save_layout(&self, dir: &Path) -> SaveLayout;
}

/// Tous les connecteurs. Pour chaque console, le premier listé est l'émulateur conseillé.
pub fn all() -> &'static [&'static dyn Emulator] {
    &[&MelonDs, &DeSmuME]
}

pub fn by_id(id: &str) -> Option<&'static dyn Emulator> {
    all().iter().copied().find(|e| e.id() == id)
}

/// Émulateur conseillé pour une console.
pub fn recommended(console: &str) -> Option<&'static dyn Emulator> {
    all().iter().copied().find(|e| e.console() == console)
}

/// Cherche, dans chaque dossier de base, les sous-dossiers dont le nom commence par `prefix`
/// et qui contiennent l'émulateur. Sans doublon, dans l'ordre des bases.
pub fn scan(emu: &dyn Emulator, bases: &[PathBuf], prefix: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for base in bases {
        let Ok(entries) = std::fs::read_dir(base) else { continue };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.to_lowercase().starts_with(prefix))
            })
            .collect();
        dirs.sort();
        for d in dirs {
            if let Some(r) = emu.resolve_dir(&d) {
                let key = r.to_string_lossy().to_lowercase();
                if !found.iter().any(|f| f.to_string_lossy().to_lowercase() == key) {
                    found.push(r);
                }
            }
        }
    }
    found
}

/// Dossiers où l'utilisateur range habituellement un émulateur téléchargé.
pub fn usual_bases() -> Vec<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut bases = Vec::new();
    if let Some(home) = env("USERPROFILE") {
        for sub in ["Downloads", "Desktop", "Documents"] {
            bases.push(home.join(sub));
        }
        // Bureau et Documents déplacés par OneDrive
        for sub in ["Desktop", "Bureau", "Documents"] {
            bases.push(home.join("OneDrive").join(sub));
        }
        bases.push(home);
    }
    if let Some(local) = env("LOCALAPPDATA") {
        bases.push(local.join("Programs"));
    }
    bases.extend(env("ProgramFiles"));
    bases.extend(env("ProgramFiles(x86)"));
    bases.push(PathBuf::from(format!("{}\\", std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()))));
    bases
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detecte_les_dossiers_melonds() {
        let base = std::env::temp_dir().join(format!("vemuz_detect_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let direct = base.join("melonDS-1.0-windows");
        let nested = base.join("MelonDS zip").join("melonDS-1.0");
        let other = base.join("autre");
        let empty = base.join("melonDS-vide");
        for d in [&direct, &nested, &other, &empty] {
            fs::create_dir_all(d).unwrap();
        }
        for d in [&direct, &nested, &other] {
            fs::write(d.join(MelonDs::EXE), b"").unwrap();
        }

        let found = scan(&MelonDs, &[base.clone(), base.join("absent")], "melonds");
        fs::remove_dir_all(&base).unwrap();

        assert_eq!(found, vec![nested, direct]);
    }

    #[test]
    fn resout_un_sous_dossier() {
        let base = std::env::temp_dir().join(format!("vemuz_resolve_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let inner = base.join("melonDS-1.0");
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join(MelonDs::EXE), b"").unwrap();

        let r = MelonDs.resolve_dir(&base);
        let none = MelonDs.resolve_dir(&inner.join("rien"));
        fs::remove_dir_all(&base).unwrap();

        assert_eq!(r, Some(inner));
        assert_eq!(none, None);
    }

    #[test]
    fn registre_coherent() {
        let mut ids: Vec<&str> = all().iter().map(|e| e.id()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "identifiants uniques");
        assert_eq!(recommended("ds").map(|e| e.id()), Some("melonds"));
        // Chaque réglage déclaré a une valeur lisible et des identifiants uniques
        for e in all() {
            let schema = e.settings_schema();
            let mut sids: Vec<&str> = schema.iter().map(|s| s.id).collect();
            sids.sort();
            sids.dedup();
            assert_eq!(sids.len(), schema.len(), "réglages uniques pour {}", e.id());
            // Un préréglage ne touche que des réglages déclarés
            for p in e.presets() {
                for k in p.values.keys() {
                    assert!(sids.contains(&k.as_str()), "{} : {k} absent du schéma", p.id);
                }
            }
        }
    }
}
