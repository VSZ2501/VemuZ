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
}

impl Game {
    pub fn display_title(&self) -> &str {
        self.custom_title.as_deref().or(self.db_title.as_deref()).unwrap_or(&self.title)
    }
}

/// Lit le titre et le code du jeu dans l'en-tête NDS (octets 0x00-0x0B et 0x0C-0x0F).
/// Retourne None si le fichier est illisible. Le nom du fichier sert de secours pour le titre.
pub fn read_game(path: &Path) -> Option<Game> {
    let mut header = [0u8; 16];
    File::open(path).ok()?.read_exact(&mut header).ok()?;

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

pub fn is_nds(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("nds"))
}
