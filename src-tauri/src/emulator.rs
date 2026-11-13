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
}
