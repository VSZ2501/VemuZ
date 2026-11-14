mod archive;
mod covers;
mod emulator;
mod melonds_config;
mod rom;
mod saves;
mod storage;
mod titles;

use emulator::{Emulator, LaunchOptions, MelonDs};
use rom::Game;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

const SETTINGS_FILE: &str = "settings.json";
const GAMES_FILE: &str = "games.json";

#[derive(Serialize, Deserialize, Default, Clone)]
struct Settings {
    melonds_dir: Option<String>,
    #[serde(default)]
    options: LaunchOptions,
    /// Taille des jaquettes : "s", "m" ou "l" (vide : moyenne)
    #[serde(default)]
    cover_size: String,
    /// Thème : "dark", "light" ou "system" (vide : sombre)
    #[serde(default)]
    theme: String,
    /// Directions de la manette dans melonDS : "dpad", "stick" ou "both" (vide : les deux)
    #[serde(default)]
    pad_directions: String,
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

/// Vérifie que l'exécutable melonDS est dans le dossier (ou un sous-dossier direct), puis mémorise le chemin.
#[tauri::command]
fn set_melonds_dir(app: AppHandle, state: State<AppState>, dir: String) -> Result<Settings, String> {
    let dir = MelonDs
        .resolve_dir(Path::new(&dir))
        .ok_or("melonDS.exe introuvable dans ce dossier. Choisis le dossier qui contient melonDS.exe.")?;
    let mut settings = state.settings.lock().unwrap();
    settings.melonds_dir = Some(dir.to_string_lossy().into_owned());
    storage::save(&app, SETTINGS_FILE, &*settings)?;
    Ok(settings.clone())
}

/// Mémorise les options de lancement (plein écran, mode de démarrage).
#[tauri::command]
fn set_launch_options(app: AppHandle, state: State<AppState>, options: LaunchOptions) -> Result<Settings, String> {
    let mut settings = state.settings.lock().unwrap();
    settings.options = options;
    storage::save(&app, SETTINGS_FILE, &*settings)?;
    Ok(settings.clone())
}

/// Mémorise la taille des jaquettes de la bibliothèque.
#[tauri::command]
fn set_cover_size(app: AppHandle, state: State<AppState>, size: String) -> Result<Settings, String> {
    if !matches!(size.as_str(), "s" | "m" | "l") {
        return Err("Taille inconnue.".into());
    }
    let mut settings = state.settings.lock().unwrap();
    settings.cover_size = size;
    storage::save(&app, SETTINGS_FILE, &*settings)?;
    Ok(settings.clone())
}

/// Mémorise le choix des directions de la manette (croix, stick ou les deux).
#[tauri::command]
fn set_pad_directions(app: AppHandle, state: State<AppState>, directions: String) -> Result<Settings, String> {
    if !matches!(directions.as_str(), "dpad" | "stick" | "both") {
        return Err("Choix inconnu.".into());
    }
    let mut settings = state.settings.lock().unwrap();
    settings.pad_directions = directions;
    storage::save(&app, SETTINGS_FILE, &*settings)?;
    Ok(settings.clone())
}

/// Réduit la fenêtre de VemuZ à la fin de l'animation de lancement,
/// sauf si melonDS s'est déjà refermé entre-temps.
#[tauri::command]
fn minimize_window(app: AppHandle, state: State<AppState>) {
    if !state.running.load(Ordering::SeqCst) {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.minimize();
    }
}

/// Mémorise le thème de l'interface.
#[tauri::command]
fn set_theme(app: AppHandle, state: State<AppState>, theme: String) -> Result<Settings, String> {
    if !matches!(theme.as_str(), "dark" | "light" | "system") {
        return Err("Thème inconnu.".into());
    }
    let mut settings = state.settings.lock().unwrap();
    settings.theme = theme;
    storage::save(&app, SETTINGS_FILE, &*settings)?;
    Ok(settings.clone())
}

fn config_file(state: &AppState) -> Result<PathBuf, String> {
    let dir = state.settings.lock().unwrap().melonds_dir.clone().ok_or("Dossier melonDS non configuré.")?;
    melonds_config::config_path(Path::new(&dir)).ok_or_else(|| {
        "Configuration de melonDS introuvable : ouvre melonDS une fois, puis ferme-le pour qu'il la crée.".to_string()
    })
}

/// Lit les réglages simplifiés dans le fichier de configuration de melonDS.
#[tauri::command]
fn get_emulator_config(state: State<AppState>) -> Result<melonds_config::EmuConfig, String> {
    melonds_config::read(&config_file(&state)?)
}

/// Écrit les réglages dans melonDS.toml. Refusé pendant que melonDS tourne : il réécrit son fichier en se fermant.
#[tauri::command]
fn set_emulator_config(
    state: State<AppState>,
    config: melonds_config::EmuConfig,
) -> Result<melonds_config::EmuConfig, String> {
    if state.running.load(Ordering::SeqCst) {
        return Err("Ferme melonDS avant de modifier ses réglages.".into());
    }
    let path = config_file(&state)?;
    melonds_config::write(&path, &config)?;
    melonds_config::read(&path)
}

/// Touches du clavier configurées dans melonDS (codes Qt, -1 si non assignée).
#[tauri::command]
fn get_keybindings(state: State<AppState>) -> Result<std::collections::HashMap<String, i64>, String> {
    melonds_config::read_keys(&config_file(&state)?)
}

#[tauri::command]
fn set_keybindings(
    state: State<AppState>,
    keys: std::collections::HashMap<String, i64>,
) -> Result<std::collections::HashMap<String, i64>, String> {
    if state.running.load(Ordering::SeqCst) {
        return Err("Ferme melonDS avant de modifier ses réglages.".into());
    }
    let path = config_file(&state)?;
    melonds_config::write_keys(&path, &keys)?;
    melonds_config::read_keys(&path)
}

/// Boutons de la manette configurés dans melonDS (codes SDL, -1 si non assigné).
#[tauri::command]
fn get_joybindings(state: State<AppState>) -> Result<std::collections::HashMap<String, i64>, String> {
    melonds_config::read_joy(&config_file(&state)?)
}

#[tauri::command]
fn set_joybindings(
    state: State<AppState>,
    keys: std::collections::HashMap<String, i64>,
) -> Result<std::collections::HashMap<String, i64>, String> {
    if state.running.load(Ordering::SeqCst) {
        return Err("Ferme melonDS avant de modifier ses réglages.".into());
    }
    let path = config_file(&state)?;
    melonds_config::write_joy(&path, &keys)?;
    melonds_config::read_joy(&path)
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

/// Dossiers où melonDS semble installé, pour le premier lancement.
#[tauri::command]
fn detect_emulator() -> Vec<String> {
    MelonDs.detect().iter().map(|p| p.to_string_lossy().into_owned()).collect()
}

/// Ouvre la page officielle de téléchargement dans le navigateur. L'adresse est fixée dans le code.
#[tauri::command]
fn open_download_page() -> Result<(), String> {
    let url = MelonDs.download_url().ok_or("Aucune page de téléchargement connue.")?;
    std::process::Command::new("explorer")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Impossible d'ouvrir le navigateur : {e}"))
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

/// Ajoute des fichiers (.nds, .zip, .7z) et des dossiers, parcourus récursivement.
/// Sert aux boutons d'ajout et au glisser-déposer.
#[tauri::command]
async fn add_paths(app: AppHandle, paths: Vec<String>) -> Result<Vec<Game>, String> {
    // Lecture des archives potentiellement lente : hors du thread principal
    let found = tauri::async_runtime::spawn_blocking(move || {
        let mut found = Vec::new();
        let mut stack: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        while let Some(p) = stack.pop() {
            if p.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&p) {
                    stack.extend(entries.flatten().map(|e| e.path()));
                }
            } else if rom::is_supported(&p) {
                found.extend(rom::read_game(&p));
            }
        }
        found
    })
    .await
    .map_err(|e| e.to_string())?;
    add_to_library(&app, &app.state::<AppState>(), found)
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

/// Franchise choisie à la main. Un texte vide rétablit la détection automatique.
#[tauri::command]
fn set_franchise(app: AppHandle, state: State<AppState>, path: String, franchise: String) -> Result<Vec<Game>, String> {
    let mut games = state.games.lock().unwrap();
    let franchise = franchise.trim();
    if let Some(g) = games.iter_mut().find(|g| same_path(&g.path, &path)) {
        g.franchise = (!franchise.is_empty()).then(|| franchise.to_string());
    }
    storage::save(&app, GAMES_FILE, &*games)?;
    Ok(games.clone())
}

#[tauri::command]
fn set_favorite(app: AppHandle, state: State<AppState>, path: String, favorite: bool) -> Result<Vec<Game>, String> {
    let mut games = state.games.lock().unwrap();
    if let Some(g) = games.iter_mut().find(|g| same_path(&g.path, &path)) {
        g.favorite = favorite;
    }
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

// ---------- Sauvegardes ----------

/// Où trouver les sauvegardes d'un jeu : (fichier .sav, dossier des sauvegardes rapides, dossier des copies).
struct SaveCtx {
    save: Option<PathBuf>,
    states: Option<PathBuf>,
    backups: PathBuf,
}

fn save_ctx(app: &AppHandle, state: &AppState, game: &str) -> Result<SaveCtx, String> {
    let melonds = state.settings.lock().unwrap().melonds_dir.clone().map(PathBuf::from);
    let (save_dir, state_dir) = melonds
        .as_deref()
        .and_then(|d| melonds_config::config_path(d).map(|c| melonds_config::asset_dirs(&c, d)))
        .unwrap_or((None, None));
    let code = state
        .games
        .lock()
        .unwrap()
        .iter()
        .find(|g| same_path(&g.path, game))
        .map(|g| g.code.clone())
        .unwrap_or_default();
    let game = Path::new(game);
    Ok(SaveCtx {
        save: saves::save_file(game, save_dir.as_deref()),
        states: state_dir,
        backups: backups_root(app)?.join(saves::backup_key(&code, game)),
    })
}

fn backups_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(storage::data_dir(app)?.join("saves"))
}

/// Copie automatique de la sauvegarde d'un jeu, si elle a changé.
fn auto_backup(app: &AppHandle, game: &str) {
    let state = app.state::<AppState>();
    if let Ok(ctx) = save_ctx(app, &state, game)
        && let Some(save) = ctx.save
    {
        let _ = saves::backup(&save, &ctx.backups);
    }
}

#[tauri::command]
fn get_saves(app: AppHandle, state: State<AppState>, path: String) -> Result<saves::SaveInfo, String> {
    let ctx = save_ctx(&app, &state, &path)?;
    Ok(saves::get(Path::new(&path), ctx.save.as_deref().and_then(Path::parent), ctx.states.as_deref(), &ctx.backups))
}

#[tauri::command]
fn export_save(app: AppHandle, state: State<AppState>, path: String, dest: String) -> Result<(), String> {
    let save = save_ctx(&app, &state, &path)?.save.filter(|p| p.is_file()).ok_or("Ce jeu n'a pas encore de sauvegarde.")?;
    std::fs::copy(save, dest).map(|_| ()).map_err(|e| format!("Export impossible : {e}"))
}

/// Toutes les sauvegardes existantes dans une archive .zip (créée avec tar.exe).
#[tauri::command]
fn export_all_saves(app: AppHandle, state: State<AppState>, dest: String) -> Result<usize, String> {
    let paths: Vec<String> = state.games.lock().unwrap().iter().map(|g| g.path.clone()).collect();
    let mut cmd = archive::tar();
    cmd.arg("-a").arg("-cf").arg(&dest);
    let mut names: Vec<String> = Vec::new();
    for p in paths {
        let Some(save) = save_ctx(&app, &state, &p)?.save.filter(|s| s.is_file()) else { continue };
        let (Some(dir), Some(name)) = (save.parent(), save.file_name()) else { continue };
        let name = name.to_string_lossy().into_owned();
        // Deux sauvegardes du même nom : seule la première entre dans l'archive
        if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            continue;
        }
        cmd.arg("-C").arg(dir).arg(&name);
        names.push(name);
    }
    if names.is_empty() {
        return Err("Aucune sauvegarde à exporter pour l'instant.".into());
    }
    let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
    if ok { Ok(names.len()) } else { Err("Création de l'archive impossible.".into()) }
}

#[tauri::command]
fn restore_backup(app: AppHandle, state: State<AppState>, path: String, id: String) -> Result<saves::SaveInfo, String> {
    if state.running.load(Ordering::SeqCst) {
        return Err("Ferme melonDS avant de restaurer une sauvegarde.".into());
    }
    let ctx = save_ctx(&app, &state, &path)?;
    let save = ctx.save.as_deref().ok_or("Emplacement de la sauvegarde inconnu pour ce jeu.")?;
    saves::restore(save, &ctx.backups, &id)?;
    get_saves(app, state, path)
}

/// Dossier des copies automatiques, pour l'afficher dans les Réglages.
#[tauri::command]
fn backups_dir(app: AppHandle) -> Result<String, String> {
    let dir = backups_root(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().into_owned())
}

/// Ouvre l'Explorateur sur un fichier (ou un dossier) existant.
#[tauri::command]
fn show_in_explorer(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err("Ce fichier n'existe plus.".into());
    }
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // explorer attend /select,"chemin" en un seul argument, sans guillemets autour du tout
        if p.is_file() {
            cmd.raw_arg(format!("/select,\"{path}\""));
        } else {
            cmd.raw_arg(format!("\"{path}\""));
        }
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ---------- Lancement ----------

/// Lance la ROM, réduit la fenêtre, et la restaure quand l'émulateur se ferme.
#[tauri::command]
fn launch_game(app: AppHandle, state: State<AppState>, path: String) -> Result<(), String> {
    run_emulator(app, &state, Some(path), false)
}

/// Ouvre melonDS sans jeu, pour régler les manettes, les écrans ou les BIOS dans l'émulateur.
#[tauri::command]
fn open_emulator(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    run_emulator(app, &state, None, true)
}

/// Lance melonDS. `minimize` : réduire VemuZ tout de suite (sinon le frontend le fait après son animation).
fn run_emulator(app: AppHandle, state: &AppState, rom: Option<String>, minimize: bool) -> Result<(), String> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("Un jeu est déjà en cours.".into());
    }
    let (dir, options) = {
        let s = state.settings.lock().unwrap();
        (s.melonds_dir.clone(), s.options.clone())
    };
    // Copie de sécurité avant de jouer
    if let Some(r) = &rom {
        auto_backup(&app, r);
    }
    let child = match dir {
        Some(d) => MelonDs.launch(Path::new(&d), rom.as_deref().map(Path::new), &options),
        None => Err("Dossier melonDS non configuré (voir Réglages).".into()),
    };
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            state.running.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };

    // Mémorise la date de lancement pour le tri "dernier lancé"
    if let Some(path) = &rom {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs());
        let mut games = state.games.lock().unwrap();
        if let Some(g) = games.iter_mut().find(|g| same_path(&g.path, path)) {
            g.last_played = now;
        }
        let _ = storage::save(&app, GAMES_FILE, &*games);
    }

    let window = app.get_webview_window("main");
    if minimize && let Some(w) = &window {
        let _ = w.minimize();
    }
    let started = std::time::Instant::now();
    std::thread::spawn(move || {
        let _ = child.wait();
        // Temps de jeu de la partie, compté à la fermeture de melonDS
        if let Some(r) = &rom {
            let state = app.state::<AppState>();
            let mut games = state.games.lock().unwrap();
            if let Some(g) = games.iter_mut().find(|g| same_path(&g.path, r)) {
                g.add_play_time(started.elapsed().as_secs());
            }
            let _ = storage::save(&app, GAMES_FILE, &*games);
            drop(games);
            let _ = app.emit("games-updated", ());
        }
        if let Some(w) = window {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
        app.state::<AppState>().running.store(false, Ordering::SeqCst);
        // Copie de la sauvegarde de la partie qui vient de se terminer
        if let Some(r) = &rom {
            auto_backup(&app, r);
        }
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
            set_launch_options,
            set_cover_size,
            set_theme,
            set_pad_directions,
            minimize_window,
            get_emulator_config,
            set_emulator_config,
            get_keybindings,
            set_keybindings,
            get_joybindings,
            set_joybindings,
            open_emulator,
            melonds_ok,
            detect_emulator,
            open_download_page,
            list_games,
            list_covers,
            add_paths,
            set_favorite,
            set_franchise,
            get_saves,
            export_save,
            export_all_saves,
            restore_backup,
            backups_dir,
            show_in_explorer,
            remove_game,
            rename_game,
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("erreur au démarrage de VemuZ");
}
