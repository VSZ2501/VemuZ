use crate::{storage, titles, AppState};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const BASE_URL: &str = "https://art.gametdb.com/ds/cover";
// Une jaquette introuvable n'est pas redemandée avant ce délai
const RETRY_AFTER: Duration = Duration::from_secs(7 * 24 * 3600);

#[cfg(windows)]
const CURL: &str = "curl.exe";
#[cfg(not(windows))]
const CURL: &str = "curl";

#[derive(Serialize, Clone)]
struct CoverReady {
    code: String,
    path: String,
}

pub enum Fetch {
    Done,
    NotFound,
    Network,
    NoCurl,
}

pub fn dir(app: &AppHandle) -> Result<PathBuf, String> {
    let d = storage::data_dir(app)?.join("covers");
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

/// Jaquettes déjà en cache : code du jeu vers chemin du fichier.
pub fn list(app: &AppHandle) -> HashMap<String, String> {
    let Ok(d) = dir(app) else { return HashMap::new() };
    let Ok(entries) = fs::read_dir(d) else { return HashMap::new() };
    entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()? != "jpg" {
                return None;
            }
            let code = p.file_stem()?.to_str()?.to_string();
            Some((code, p.to_string_lossy().into_owned()))
        })
        .collect()
}

/// Régions à essayer selon la 4e lettre du code, avec EN en repli.
fn regions(code: &str) -> Vec<&'static str> {
    let mut r = match code.as_bytes().get(3) {
        Some(b'P') | Some(b'F') => vec!["FR", "EN"],
        Some(b'E') => vec!["US"],
        Some(b'J') => vec!["JA"],
        _ => vec![],
    };
    if !r.contains(&"EN") {
        r.push("EN");
    }
    r
}

fn is_jpeg(path: &Path) -> bool {
    let mut magic = [0u8; 3];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_ok()
        && magic == [0xFF, 0xD8, 0xFF]
}

/// Télécharge une URL vers un fichier via curl, de façon atomique.
/// `valid` vérifie le contenu téléchargé avant de remplacer la destination.
pub fn fetch(url: &str, dest: &Path, valid: fn(&Path) -> bool) -> Fetch {
    let tmp = dest.with_extension("part");
    let mut cmd = Command::new(CURL);
    cmd.args(["-f", "-s", "-L", "--max-time", "20", "-o"]).arg(&tmp).arg(url);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // pas de fenêtre console
    }
    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Fetch::NoCurl,
        Err(_) => return Fetch::Network,
    };
    let result = match status.code() {
        Some(0) if valid(&tmp) => fs::rename(&tmp, dest).map(|_| Fetch::Done).unwrap_or(Fetch::Network),
        Some(0) | Some(22) => Fetch::NotFound, // 22 : erreur HTTP (404)
        _ => Fetch::Network,
    };
    if !matches!(result, Fetch::Done) {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn download(dir: &Path, code: &str) -> Fetch {
    let dest = dir.join(format!("{code}.jpg"));
    for region in regions(code) {
        match fetch(&format!("{BASE_URL}/{region}/{code}.jpg"), &dest, is_jpeg) {
            Fetch::NotFound => continue,
            other => return other,
        }
    }
    Fetch::NotFound
}

fn marker_fresh(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < RETRY_AFTER)
}

/// Prochain code de jeu sans jaquette ni marqueur d'absence récent.
fn next_pending(app: &AppHandle, dir: &Path) -> Option<String> {
    let state = app.state::<AppState>();
    let games = state.games.lock().unwrap();
    games
        .iter()
        .map(|g| g.code.as_str())
        .filter(|c| !c.is_empty())
        .find(|c| !dir.join(format!("{c}.jpg")).exists() && !marker_fresh(&dir.join(format!("{c}.none"))))
        .map(str::to_string)
}

/// Lance le téléchargement en arrière-plan (un seul worker à la fois, un jeu à la fois).
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.fetching.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        run(&app);
        app.state::<AppState>().fetching.store(false, Ordering::SeqCst);
    });
}

fn run(app: &AppHandle) {
    let Ok(dir) = dir(app) else { return };
    // Jamais pendant qu'un jeu tourne
    if app.state::<AppState>().running.load(Ordering::SeqCst) {
        return;
    }
    // Titres complets d'abord (un seul petit fichier, rafraîchi tous les 30 jours)
    if let Some(Fetch::NoCurl) = titles::refresh(app) {
        let _ = app.emit("covers-error", "curl introuvable : jaquettes et titres indisponibles.");
        return;
    }
    if titles::apply(app) {
        let _ = app.emit("games-updated", ());
    }
    while !app.state::<AppState>().running.load(Ordering::SeqCst) {
        let Some(code) = next_pending(app, &dir) else { break };
        match download(&dir, &code) {
            Fetch::Done => {
                let path = dir.join(format!("{code}.jpg")).to_string_lossy().into_owned();
                let _ = app.emit("cover-ready", CoverReady { code, path });
            }
            Fetch::NotFound => {
                let _ = fs::write(dir.join(format!("{code}.none")), b"");
            }
            Fetch::Network => break,
            Fetch::NoCurl => {
                let _ = app.emit("covers-error", "curl introuvable : jaquettes indisponibles.");
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
