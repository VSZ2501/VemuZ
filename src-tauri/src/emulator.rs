use std::path::{Path, PathBuf};
use std::process::{Child, Command};

/// Interface commune à tous les émulateurs. Un connecteur par émulateur.
pub trait Emulator {
    /// Chemin de l'exécutable dans le dossier d'installation, s'il existe.
    fn executable(&self, dir: &Path) -> Option<PathBuf>;
    /// Lance l'émulateur avec la ROM donnée.
    fn launch(&self, dir: &Path, rom: &Path) -> Result<Child, String>;
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

    fn launch(&self, dir: &Path, rom: &Path) -> Result<Child, String> {
        let exe = self
            .executable(dir)
            .ok_or_else(|| format!("{} introuvable dans {}", Self::EXE, dir.display()))?;
        Command::new(exe)
            .arg(rom)
            .current_dir(dir)
            .spawn()
            .map_err(|e| format!("Lancement impossible : {e}"))
    }
}
