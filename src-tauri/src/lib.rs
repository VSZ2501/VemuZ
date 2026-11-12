mod covers;
mod emulator;
mod rom;
mod storage;
mod titles;

use emulator::{Emulator, MelonDs};
use rom::Game;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

const SETTINGS_FILE: &str = "settings.json";
const GAMES_FILE: &str = "games.json";

#[derive(Serialize, Deserialize, Default, Clone)]
struct Settings {
    melonds_dir: Option<String>,
}

#[derive(Default)]
struct AppState {
    settings: Mutex<Settings>,
    games: Mutex<Vec<Game>>,
    running: AtomicBool,
    // Vrai quand le worker de jaquettes tourne
    fetching: AtomicBool,
}

// ---------- Réglages ----------

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

/// Vérifie que l'exécutable melonDS est présent dans le dossier, puis mémorise le chemin.
#[tauri::command]
fn set_melonds_dir(app: AppHandle, state: State<AppState>, dir: String) -> Result<Settings, String> {
    if MelonDs.executable(Path::new(&dir)).is_none() {
        return Err("Exécutable melonDS introuvable dans ce dossier.".into());
    }
    let mut settings = state.settings.lock().unwrap();
    settings.melonds_dir = Some(dir);
    storage::save(&app, SETTINGS_FILE, &*settings)?;
    Ok(settings.clone())
}

/// Indique si le dossier mémorisé contient toujours l'exécutable.
#[tauri::command]
fn melonds_ok(state: State<AppState>) -> bool {
    state
        .settings
        .lock()
        .unwrap()
        .melonds_dir
        .as_deref()
        .is_some_and(|d| MelonDs.executable(Path::new(d)).is_some())
}

// ---------- Bibliothèque ----------

#[tauri::command]
fn list_games(state: State<AppState>) -> Vec<Game> {
    state.games.lock().unwrap().clone()
}

fn same_path(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Ajoute les jeux sans doublon, trie par titre et sauvegarde.
fn add_to_library(app: &AppHandle, state: &AppState, new: Vec<Game>) -> Result<Vec<Game>, String> {
    let mut games = state.games.lock().unwrap();
    for g in new {
        if !games.iter().any(|x| same_path(&x.path, &g.path)) {
            games.push(g);
        }
    }
    games.sort_by_key(|g| g.display_title().to_lowercase());
    storage::save(app, GAMES_FILE, &*games)?;
    let result = games.clone();
    drop(games);
    covers::start(app);
    Ok(result)
}

#[tauri::command]
fn list_covers(app: AppHandle) -> std::collections::HashMap<String, String> {
    covers::list(&app)
}

#[tauri::command]
fn add_games(app: AppHandle, state: State<AppState>, paths: Vec<String>) -> Result<Vec<Game>, String> {
    let new = paths
        .iter()
        .map(PathBuf::from)
        .filter(|p| rom::is_nds(p))
        .filter_map(|p| rom::read_game(&p))
        .collect();
    add_to_library(&app, &state, new)
}

#[tauri::command]
fn add_folder(app: AppHandle, state: State<AppState>, dir: String) -> Result<Vec<Game>, String> {
    let mut found = Vec::new();
    let mut stack = vec![PathBuf::from(dir)];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if rom::is_nds(&p) {
                found.extend(rom::read_game(&p));
            }
        }
    }
    add_to_library(&app, &state, found)
}

/// Renomme un jeu. Un titre vide rétablit le titre d'origine.
#[tauri::command]
fn rename_game(app: AppHandle, state: State<AppState>, path: String, title: String) -> Result<Vec<Game>, String> {
    let mut games = state.games.lock().unwrap();
    let title = title.trim();
    if let Some(g) = games.iter_mut().find(|g| same_path(&g.path, &path)) {
        g.custom_title = (!title.is_empty()).then(|| title.to_string());
    }
    games.sort_by_key(|g| g.display_title().to_lowercase());
    storage::save(&app, GAMES_FILE, &*games)?;
    Ok(games.clone())
}

#[tauri::command]
fn remove_game(app: AppHandle, state: State<AppState>, path: String) -> Result<Vec<Game>, String> {
    let mut games = state.games.lock().unwrap();
    games.retain(|g| !same_path(&g.path, &path));
    storage::save(&app, GAMES_FILE, &*games)?;
    Ok(games.clone())
}

// ---------- Lancement ----------

/// Lance la ROM, réduit la fenêtre, et la restaure quand l'émulateur se ferme.
#[tauri::command]
fn launch_game(app: AppHandle, state: State<AppState>, path: String) -> Result<(), String> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("Un jeu est déjà en cours.".into());
    }
    let dir = state.settings.lock().unwrap().melonds_dir.clone();
    let child = match dir {
        Some(d) => MelonDs.launch(Path::new(&d), Path::new(&path)),
        None => Err("Dossier melonDS non configuré (voir Réglages).".into()),
    };
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            state.running.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };

    let window = app.get_webview_window("main");
    if let Some(w) = &window {
        let _ = w.minimize();
    }
    std::thread::spawn(move || {
        let _ = child.wait();
        if let Some(w) = window {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
        app.state::<AppState>().running.store(false, Ordering::SeqCst);
        // Reprend les jaquettes manquantes une fois le jeu fermé
        covers::start(&app);
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            let handle = app.handle();
            let state = app.state::<AppState>();
            *state.settings.lock().unwrap() = storage::load(handle, SETTINGS_FILE);
            *state.games.lock().unwrap() = storage::load(handle, GAMES_FILE);
            covers::start(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_melonds_dir,
            melonds_ok,
            list_games,
            list_covers,
            add_games,
            add_folder,
            remove_game,
            rename_game,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("erreur au démarrage de VemuZ");
}
