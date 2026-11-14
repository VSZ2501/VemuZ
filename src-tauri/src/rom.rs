use crate::archive;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
pub struct Game {
    pub path: String,
    /// Titre lu dans l'en-tête de la ROM (tronqué à 12 caractères) ou nom du fichier
    pub title: String,
    pub code: String,
    /// Titre complet issu de la base GameTDB
    #[serde(default)]
    pub db_title: Option<String>,
    /// Titre choisi par l'utilisateur, prioritaire
    #[serde(default)]
    pub custom_title: Option<String>,
    /// Dernier lancement, en secondes depuis l'epoch Unix
    #[serde(default)]
    pub last_played: Option<u64>,
    /// Temps de jeu cumulé, en secondes (durée pendant laquelle melonDS est ouvert)
    #[serde(default)]
    pub play_time: u64,
    #[serde(default)]
    pub favorite: bool,
    /// Franchise choisie par l'utilisateur ; sinon déduite du titre dans l'interface
    #[serde(default)]
    pub franchise: Option<String>,
}

impl Game {
    /// Ajoute la durée d'une partie au temps de jeu.
    pub fn add_play_time(&mut self, secs: u64) {
        self.play_time = self.play_time.saturating_add(secs);
    }

    pub fn display_title(&self) -> &str {
        self.custom_title.as_deref().or(self.db_title.as_deref()).unwrap_or(&self.title)
    }
}

/// Lit le titre et le code du jeu dans l'en-tête NDS (octets 0x00-0x0B et 0x0C-0x0F),
/// depuis un fichier .nds ou la première ROM d'une archive .zip ou .7z.
/// Retourne None si le fichier est illisible ou si l'archive ne contient aucune ROM.
/// Le nom du fichier sert de secours pour le titre.
pub fn read_game(path: &Path) -> Option<Game> {
    let header = if archive::is_archive(path) {
        if archive::nds_count(path) == 0 {
            return None;
        }
        archive::first_header(path)?
    } else {
        let mut header = [0u8; 16];
        File::open(path).ok()?.read_exact(&mut header).ok()?;
        header
    };

    let title = ascii_clean(&header[0..12]);
    let code = ascii_clean(&header[12..16]);
    let code = if code.len() == 4 && code.bytes().all(|b| b.is_ascii_alphanumeric()) {
        code
    } else {
        String::new()
    };

    let title = if title.is_empty() {
        path.file_stem()?.to_string_lossy().into_owned()
    } else {
        title
    };

    Some(Game {
        path: path.to_string_lossy().into_owned(),
        title,
        code,
        db_title: None,
        custom_title: None,
        last_played: None,
        play_time: 0,
        favorite: false,
        franchise: None,
    })
}

/// Garde les caractères ASCII imprimables jusqu'au premier octet nul.
fn ascii_clean(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let s: String = bytes[..end]
        .iter()
        .map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '\u{FFFD}' })
        .collect();
    if s.contains('\u{FFFD}') { String::new() } else { s.trim().to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancien_json_et_temps_de_jeu() {
        let old = r#"{"path":"C:\\a.nds","title":"MARIO","code":"AMCE","last_played":12}"#;
        let mut g: Game = serde_json::from_str(old).unwrap();
        assert_eq!((g.play_time, g.favorite, g.last_played), (0, false, Some(12)));
        assert_eq!(g.franchise, None);
        g.add_play_time(90);
        g.add_play_time(30);
        assert_eq!(g.play_time, 120);
        g.play_time = u64::MAX - 1;
        g.add_play_time(10);
        assert_eq!(g.play_time, u64::MAX);
    }
}

pub fn is_nds(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("nds"))
}

/// Fichier que VemuZ sait ajouter : ROM .nds ou archive .zip / .7z.
pub fn is_supported(path: &Path) -> bool {
    is_nds(path) || archive::is_archive(path)
}
