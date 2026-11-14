use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

/// Options communes de lancement, mémorisées par VemuZ.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct LaunchOptions {
    #[serde(default)]
    pub fullscreen: bool,
}

/// Interface commune à tous les émulateurs. Un connecteur par émulateur.
pub trait Emulator {
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
}

/// Cherche, dans chaque dossier de base, les sous-dossiers dont le nom commence par `prefix`
/// et qui contiennent l'émulateur. Sans doublon, dans l'ordre des bases.
fn scan(emu: &dyn Emulator, bases: &[PathBuf], prefix: &str) -> Vec<PathBuf> {
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
fn usual_bases() -> Vec<PathBuf> {
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

pub struct MelonDs;

impl MelonDs {
    #[cfg(windows)]
    const EXE: &'static str = "melonDS.exe";
    #[cfg(not(windows))]
    const EXE: &'static str = "melonDS";
}

impl Emulator for MelonDs {
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
}
