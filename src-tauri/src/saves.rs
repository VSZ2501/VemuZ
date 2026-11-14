//! Sauvegardes des jeux : où melonDS les écrit, copies automatiques, restauration.
//! melonDS nomme le .sav d'après la ROM (pour une archive : la ROM qu'elle contient) et le range
//! dans SaveFilePath, sinon à côté de la ROM. Les sauvegardes rapides suivent la même règle (.ml0 à .ml9).

use crate::archive;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Nombre de copies automatiques gardées par jeu
pub const KEEP: usize = 10;

#[derive(Serialize, Clone)]
pub struct FileInfo {
    pub id: String,
    pub path: String,
    pub size: u64,
    /// Dernière modification, en millisecondes depuis l'epoch Unix
    pub modified: u64,
}

#[derive(Serialize)]
pub struct StateInfo {
    pub slot: u8,
    pub modified: u64,
}

#[derive(Serialize)]
pub struct SaveInfo {
    /// Emplacement attendu du .sav (même s'il n'existe pas encore)
    pub expected: Option<String>,
    pub save: Option<FileInfo>,
    pub states: Vec<StateInfo>,
    pub backups: Vec<FileInfo>,
}

/// Dossier et nom de base (sans extension) utilisés par melonDS pour ce jeu.
fn rom_base(game: &Path) -> Option<(PathBuf, String)> {
    let dir = game.parent()?.to_path_buf();
    let name = if archive::is_archive(game) {
        archive::first_nds_name(game)?
    } else {
        game.file_name()?.to_string_lossy().into_owned()
    };
    let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s).to_string();
    Some((dir, stem))
}

/// Chemin du .sav de ce jeu. `save_dir` : dossier réglé dans melonDS, sinon celui de la ROM.
pub fn save_file(game: &Path, save_dir: Option<&Path>) -> Option<PathBuf> {
    let (dir, stem) = rom_base(game)?;
    Some(save_dir.map_or(dir, Path::to_path_buf).join(format!("{stem}.sav")))
}

fn modified_ms(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

fn info(path: &Path) -> Option<FileInfo> {
    let meta = fs::metadata(path).ok().filter(|m| m.is_file())?;
    Some(FileInfo {
        id: path.file_name()?.to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        size: meta.len(),
        modified: modified_ms(&meta),
    })
}

/// Sauvegardes rapides présentes (emplacements 0 à 9).
fn states(game: &Path, state_dir: Option<&Path>) -> Vec<StateInfo> {
    let Some((dir, stem)) = rom_base(game) else { return Vec::new() };
    let dir = state_dir.map_or(dir, Path::to_path_buf);
    (0..=9u8)
        .filter_map(|slot| {
            let meta = fs::metadata(dir.join(format!("{stem}.ml{slot}"))).ok()?;
            Some(StateInfo { slot, modified: modified_ms(&meta) })
        })
        .collect()
}

/// Copies d'un jeu, de la plus récente à la plus ancienne.
pub fn list_backups(dir: &Path) -> Vec<FileInfo> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut list: Vec<FileInfo> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "sav"))
        .filter_map(|p| info(&p))
        .collect();
    // Les noms sont des horodatages en millisecondes, de même longueur
    list.sort_by(|a, b| b.id.cmp(&a.id));
    list
}

/// Copie la sauvegarde dans `dir` si elle diffère de la dernière copie, puis garde les KEEP plus récentes.
/// Renvoie le chemin de la nouvelle copie, ou None si rien n'a été copié.
pub fn backup(save: &Path, dir: &Path) -> Result<Option<PathBuf>, String> {
    let Ok(data) = fs::read(save) else { return Ok(None) };
    let list = list_backups(dir);
    if let Some(last) = list.first()
        && fs::read(&last.path).is_ok_and(|d| d == data)
    {
        return Ok(None);
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let mut dest = dir.join(format!("{now:015}.sav"));
    // Deux copies dans la même milliseconde : on décale
    let mut n = now;
    while dest.exists() {
        n += 1;
        dest = dir.join(format!("{n:015}.sav"));
    }
    fs::write(&dest, &data).map_err(|e| e.to_string())?;
    for old in list_backups(dir).iter().skip(KEEP) {
        let _ = fs::remove_file(&old.path);
    }
    Ok(Some(dest))
}

/// Remplace la sauvegarde par une copie, après avoir copié la sauvegarde actuelle.
pub fn restore(save: &Path, dir: &Path, id: &str) -> Result<(), String> {
    // L'identifiant vient du frontend : un simple nom de fichier, rien d'autre
    if id.contains(['/', '\\']) || id.contains("..") {
        return Err("Copie invalide.".into());
    }
    // Lue avant la copie de sécurité, dont la rotation peut supprimer la plus ancienne
    let data = fs::read(dir.join(id)).map_err(|_| "Cette copie n'existe plus.".to_string())?;
    backup(save, dir)?;
    if let Some(parent) = save.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(save, data).map_err(|e| format!("Restauration impossible : {e}"))
}

/// Nom du dossier de copies d'un jeu : son code, sinon le nom de la ROM.
pub fn backup_key(code: &str, game: &Path) -> String {
    if !code.is_empty() {
        return code.to_string();
    }
    let stem = game.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    stem.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect()
}

pub fn get(game: &Path, save_dir: Option<&Path>, state_dir: Option<&Path>, backup_dir: &Path) -> SaveInfo {
    let expected = save_file(game, save_dir);
    SaveInfo {
        save: expected.as_deref().and_then(info),
        expected: expected.map(|p| p.to_string_lossy().into_owned()),
        states: states(game, state_dir),
        backups: list_backups(backup_dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vemuz_saves_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn chemins_de_sauvegarde() {
        let rom = Path::new("C:\\Jeux\\Mario Kart (E).nds");
        assert_eq!(save_file(rom, None), Some(PathBuf::from("C:\\Jeux\\Mario Kart (E).sav")));
        assert_eq!(
            save_file(rom, Some(Path::new("D:\\Saves"))),
            Some(PathBuf::from("D:\\Saves\\Mario Kart (E).sav"))
        );
        assert_eq!(backup_key("AMCE", rom), "AMCE");
        assert_eq!(backup_key("", rom), "Mario Kart _E_");
    }

    #[test]
    fn copies_sans_doublon_et_rotation() {
        let d = temp("rotation");
        let save = d.join("jeu.sav");
        let backups = d.join("copies");

        assert_eq!(backup(&save, &backups).unwrap(), None, "pas de sauvegarde : pas de copie");
        fs::write(&save, b"v0").unwrap();
        assert!(backup(&save, &backups).unwrap().is_some());
        assert_eq!(backup(&save, &backups).unwrap(), None, "inchangée : pas de nouvelle copie");

        for i in 1..=KEEP + 2 {
            fs::write(&save, format!("v{i}")).unwrap();
            backup(&save, &backups).unwrap();
        }
        let list = list_backups(&backups);
        assert_eq!(list.len(), KEEP);
        assert_eq!(fs::read(&list[0].path).unwrap(), format!("v{}", KEEP + 2).as_bytes());

        // Restauration : l'actuelle est copiée d'abord, puis remplacée
        let oldest = list.last().unwrap().id.clone();
        let expected = fs::read(&list.last().unwrap().path).unwrap();
        fs::write(&save, b"courante").unwrap();
        restore(&save, &backups, &oldest).unwrap_or_else(|e| panic!("{e}"));
        let after = fs::read(&save).unwrap();
        let newest = fs::read(&list_backups(&backups)[0].path).unwrap();
        assert!(restore(&save, &backups, "..\\jeu.sav").is_err());
        fs::remove_dir_all(&d).unwrap();

        assert_eq!(after, expected);
        assert_eq!(newest, b"courante");
    }
}
