use crate::{covers, storage, AppState, GAMES_FILE};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager};

const URL: &str = "https://www.gametdb.com/dstdb.txt";
const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 3600);

fn file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(storage::data_dir(app)?.join("dstdb.txt"))
}

// La première ligne du fichier commence par "TITLES"
fn looks_valid(path: &Path) -> bool {
    let mut magic = [0u8; 6];
    File::open(path).and_then(|mut f| f.read_exact(&mut magic)).is_ok() && &magic == b"TITLES"
}

fn is_stale(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_none_or(|age| age > MAX_AGE)
}

/// Télécharge la base de titres si elle est absente ou ancienne.
/// None si rien n'était à faire.
pub fn refresh(app: &AppHandle) -> Option<covers::Fetch> {
    let path = file(app).ok()?;
    if path.exists() && !is_stale(&path) {
        return None;
    }
    Some(covers::fetch(URL, &path, looks_valid))
}

/// Renseigne le titre complet des jeux qui n'en ont pas encore. Vrai si la liste a changé.
pub fn apply(app: &AppHandle) -> bool {
    let Ok(path) = file(app) else { return false };
    let Ok(f) = File::open(path) else { return false };
    let state = app.state::<AppState>();

    let wanted: HashSet<String> = state
        .games
        .lock()
        .unwrap()
        .iter()
        .filter(|g| !g.code.is_empty() && g.db_title.is_none())
        .map(|g| g.code.clone())
        .collect();
    if wanted.is_empty() {
        return false;
    }

    // Lecture ligne à ligne : on ne garde en mémoire que les codes utiles
    let mut found: HashMap<String, String> = HashMap::new();
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if let Some((code, title)) = line.split_once(" = ")
            && wanted.contains(code)
        {
            found.insert(code.to_string(), title.trim().to_string());
        }
    }
    if found.is_empty() {
        return false;
    }

    let mut games = state.games.lock().unwrap();
    for g in games.iter_mut() {
        if g.db_title.is_none() && let Some(t) = found.get(&g.code) {
            g.db_title = Some(t.clone());
        }
    }
    games.sort_by_key(|g| g.display_title().to_lowercase());
    let _ = storage::save(app, GAMES_FILE, &*games);
    true
}
