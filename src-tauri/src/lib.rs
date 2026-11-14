mod archive;
mod covers;
mod desmume;
mod emulator;
mod ini;
mod melonds;
mod melonds_config;
mod rom;
mod saves;
mod storage;
mod titles;

use emulator::{Binding, Bindings, Emulator, LaunchOptions, Preset, SettingDef, Values};
use rom::Game;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, RunEvent, State, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

const SETTINGS_FILE: &str = "settings.json";
const GAMES_FILE: &str = "games.json";

/// Ce que VemuZ retient d'un émulateur.
#[derive(Serialize, Deserialize, Default, Clone)]
struct EmuEntry {
    dir: Option<String>,
    #[serde(default)]
    options: LaunchOptions,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct Settings {
    /// Par identifiant d'émulateur
    #[serde(default)]
    emulators: HashMap<String, EmuEntry>,
    /// Émulateur choisi pour chaque console (absent : le conseillé)
    #[serde(default)]
    consoles: HashMap<String, String>,
    /// Ancien format (V1.0), repris dans `emulators` au chargement
    #[serde(default, skip_serializing)]
    melonds_dir: Option<String>,
    #[serde(default, skip_serializing)]
    options: Option<LaunchOptions>,
    /// Taille des jaquettes : "s", "m" ou "l" (vide : moyenne)
    #[serde(default)]
    cover_size: String,
    /// Thème : "dark", "light" ou "system" (vide : sombre)
    #[serde(default)]
    theme: String,
    /// Directions de la manette dans melonDS : "dpad", "stick" ou "both" (vide : les deux)
    #[serde(default)]
    pad_directions: String,
    /// Vrai : la fenêtre est seulement réduite pendant un jeu (sinon fermée pour libérer la mémoire)
    #[serde(default)]
    keep_window: bool,
}

impl Settings {
    /// Reprend les réglages de la V1.0 (un seul émulateur). Vrai si quelque chose a changé.
    fn migrate(&mut self) -> bool {
        let (dir, options) = (self.melonds_dir.take(), self.options.take());
        if dir.is_none() && options.is_none() {
            return false;
        }
        let e = self.emulators.entry("melonds".into()).or_default();
        if e.dir.is_none() {
            e.dir = dir;
        }
        if let Some(o) = options {
            e.options = o;
        }
        true
    }

    /// Émulateur utilisé pour une console : celui choisi, sinon le conseillé.
    fn emulator_for(&self, console: &str) -> Option<&'static dyn Emulator> {
        self.consoles.get(console).and_then(|id| emulator::by_id(id)).or_else(|| emulator::recommended(console))
    }

    fn dir_of(&self, emu: &dyn Emulator) -> Option<PathBuf> {
        self.emulators.get(emu.id()).and_then(|e| e.dir.as_deref()).map(PathBuf::from)
    }
}

/// Vue à rouvrir quand la fenêtre est recréée après un jeu.
#[derive(Serialize, Clone)]
struct Resume {
    view: String,
    game: Option<String>,
}

/// Position et taille de la fenêtre avant sa fermeture.
#[derive(Clone, Copy)]
struct Geometry {
    maximized: bool,
    pos: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

#[derive(Default)]
struct AppState {
    settings: Mutex<Settings>,
    games: Mutex<Vec<Game>>,
    running: AtomicBool,
    // Vrai quand le worker de jaquettes tourne
    fetching: AtomicBool,
    resume: Mutex<Option<Resume>>,
    geometry: Mutex<Option<Geometry>>,
}

// ---------- Réglages ----------

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

// ---------- Émulateurs ----------

/// Un émulateur tel que l'interface le présente.
#[derive(Serialize)]
struct EmuInfo {
    id: &'static str,
    name: &'static str,
    console: &'static str,
    recommended: bool,
    /// Utilisé pour sa console
    selected: bool,
    dir: Option<String>,
    /// L'exécutable est bien dans le dossier mémorisé
    ok: bool,
    options: LaunchOptions,
    download: bool,
    bindings: Option<&'static str>,
}

fn emu_infos(s: &Settings) -> Vec<EmuInfo> {
    emulator::all()
        .iter()
        .map(|e| {
            let entry = s.emulators.get(e.id()).cloned().unwrap_or_default();
            EmuInfo {
                id: e.id(),
                name: e.name(),
                console: e.console(),
                recommended: emulator::recommended(e.console()).is_some_and(|r| r.id() == e.id()),
                selected: s.emulator_for(e.console()).is_some_and(|r| r.id() == e.id()),
                ok: entry.dir.as_deref().is_some_and(|d| e.executable(Path::new(d)).is_some()),
                dir: entry.dir,
                options: entry.options,
                download: e.download_url().is_some(),
                bindings: e.bindings_format(),
            }
        })
        .collect()
}

fn find_emu(id: &str) -> Result<&'static dyn Emulator, String> {
    emulator::by_id(id).ok_or_else(|| "Émulateur inconnu.".to_string())
}

#[tauri::command]
fn list_emulators(state: State<AppState>) -> Vec<EmuInfo> {
    emu_infos(&state.settings.lock().unwrap())
}

/// Vérifie que l'exécutable est dans le dossier (ou un sous-dossier direct), puis mémorise le chemin.
#[tauri::command]
fn set_emulator_dir(app: AppHandle, state: State<AppState>, emu: String, dir: String) -> Result<Vec<EmuInfo>, String> {
    let e = find_emu(&emu)?;
    let dir = e.resolve_dir(Path::new(&dir)).ok_or_else(|| {
        format!("{} introuvable dans ce dossier. Choisis le dossier qui contient son exécutable.", e.name())
    })?;
    let mut settings = state.settings.lock().unwrap();
    settings.emulators.entry(emu).or_default().dir = Some(dir.to_string_lossy().into_owned());
    storage::save(&app, SETTINGS_FILE, &*settings)?;
    Ok(emu_infos(&settings))
}

/// Mémorise les options de lancement d'un émulateur (plein écran).
#[tauri::command]
fn set_launch_options(
    app: AppHandle,
    state: State<AppState>,
    emu: String,
    options: LaunchOptions,
) -> Result<Vec<EmuInfo>, String> {
    find_emu(&emu)?;
    let mut settings = state.settings.lock().unwrap();
    settings.emulators.entry(emu).or_default().options = options;
    storage::save(&app, SETTINGS_FILE, &*settings)?;
    Ok(emu_infos(&settings))
}

/// Choisit l'émulateur utilisé pour une console.
#[tauri::command]
fn set_console_emulator(
    app: AppHandle,
    state: State<AppState>,
    console: String,
    emu: String,
) -> Result<Vec<EmuInfo>, String> {
    if find_emu(&emu)?.console() != console {
        return Err("Cet émulateur ne prend pas en charge cette console.".into());
    }
    let mut settings = state.settings.lock().unwrap();
    settings.consoles.insert(console, emu);
    storage::save(&app, SETTINGS_FILE, &*settings)?;
    Ok(emu_infos(&settings))
}

/// Dossier d'un émulateur configuré, ou une erreur lisible.
fn emu_dir(state: &AppState, e: &dyn Emulator) -> Result<PathBuf, String> {
    state.settings.lock().unwrap().dir_of(e).ok_or_else(|| format!("Dossier de {} non configuré.", e.name()))
}

/// Refuse une écriture pendant un jeu : l'émulateur réécrit souvent sa configuration en se fermant.
fn not_running(state: &AppState, e: &dyn Emulator) -> Result<(), String> {
    if state.running.load(Ordering::SeqCst) {
        return Err(format!("Ferme {} avant de modifier ses réglages.", e.name()));
    }
    Ok(())
}

#[derive(Serialize)]
struct EmuSettings {
    schema: Vec<SettingDef>,
    presets: Vec<Preset>,
    values: Values,
}

/// Réglages déclarés par le connecteur et valeurs lues dans la configuration de l'émulateur.
#[tauri::command]
fn get_emulator_settings(state: State<AppState>, emu: String) -> Result<EmuSettings, String> {
    let e = find_emu(&emu)?;
    let values = e.read_settings(&emu_dir(&state, e)?)?;
    Ok(EmuSettings { schema: e.settings_schema(), presets: e.presets(), values })
}

#[tauri::command]
fn set_emulator_settings(state: State<AppState>, emu: String, values: Values) -> Result<Values, String> {
    let e = find_emu(&emu)?;
    not_running(&state, e)?;
    e.write_settings(&emu_dir(&state, e)?, &values)
}

/// Touches du clavier ou boutons de la manette (-1 si non assigné).
#[tauri::command]
fn get_bindings(state: State<AppState>, emu: String, kind: Binding) -> Result<Bindings, String> {
    let e = find_emu(&emu)?;
    e.read_bindings(&emu_dir(&state, e)?, kind)
}

#[tauri::command]
fn set_bindings(state: State<AppState>, emu: String, kind: Binding, keys: Bindings) -> Result<Bindings, String> {
    let e = find_emu(&emu)?;
    not_running(&state, e)?;
    e.write_bindings(&emu_dir(&state, e)?, kind, &keys)
}

/// Un dossier où un émulateur semble installé.
#[derive(Serialize)]
struct Found {
    emu: &'static str,
    name: &'static str,
    dir: String,
}

/// Émulateurs d'une console trouvés sur la machine, le conseillé en premier.
#[tauri::command]
async fn detect_emulators(console: String) -> Vec<Found> {
    emulator::all()
        .iter()
        .filter(|e| e.console() == console)
        .flat_map(|e| {
            e.detect()
                .into_iter()
                .map(|d| Found { emu: e.id(), name: e.name(), dir: d.to_string_lossy().into_owned() })
        })
        .collect()
}

/// Ouvre la page officielle de téléchargement dans le navigateur. L'adresse est fixée dans le code.
#[tauri::command]
fn open_download_page(emu: String) -> Result<(), String> {
    let url = find_emu(&emu)?.download_url().ok_or("Aucune page de téléchargement connue.")?;
    std::process::Command::new("explorer")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Impossible d'ouvrir le navigateur : {e}"))
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

/// Met VemuZ en veille à la fin de l'animation de lancement : fenêtre fermée (WebView2 libérée)
/// ou seulement réduite selon le réglage. Ignoré si melonDS s'est déjà refermé.
/// Asynchrone : la fenêtre qui appelle est détruite, ce qui bloquerait une commande synchrone.
#[tauri::command]
async fn sleep_window(app: AppHandle, view: String, game: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    if !state.running.load(Ordering::SeqCst) {
        return Ok(());
    }
    *state.resume.lock().unwrap() = Some(Resume { view, game });
    let Some(w) = app.get_webview_window("main") else { return Ok(()) };
    let keep = state.settings.lock().unwrap().keep_window;
    if keep {
        let _ = w.minimize();
    } else {
        remember_geometry(&w);
        let _ = w.destroy();
    }
    Ok(())
}

/// Vue à rouvrir après un jeu (rien au démarrage normal).
#[tauri::command]
fn get_resume(state: State<AppState>) -> Option<Resume> {
    state.resume.lock().unwrap().clone()
}

/// Mémorise le choix de garder la fenêtre (réduite) pendant un jeu.
#[tauri::command]
fn set_keep_window(app: AppHandle, state: State<AppState>, keep: bool) -> Result<Settings, String> {
    let mut settings = state.settings.lock().unwrap();
    settings.keep_window = keep;
    storage::save(&app, SETTINGS_FILE, &*settings)?;
    Ok(settings.clone())
}

fn remember_geometry(w: &WebviewWindow) {
    if w.is_minimized().unwrap_or(false) {
        return;
    }
    let (Ok(pos), Ok(size)) = (w.outer_position(), w.outer_size()) else { return };
    let maximized = w.is_maximized().unwrap_or(false);
    *w.state::<AppState>().geometry.lock().unwrap() = Some(Geometry { maximized, pos, size });
}

/// Affiche la fenêtre, ou la recrée si elle a été fermée pendant un jeu.
fn wake_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    // Fil séparé : sous Windows, créer une fenêtre depuis un gestionnaire d'événement bloque l'app
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(mut cfg) = app.config().app.windows.first().cloned() else { return };
        let geo = *app.state::<AppState>().geometry.lock().unwrap();
        if geo.is_some() {
            // Cachée le temps de reprendre la taille et la position d'avant
            cfg.maximized = false;
            cfg.visible = false;
        }
        let Ok(w) = WebviewWindowBuilder::from_config(&app, &cfg).and_then(|b| b.build()) else { return };
        if let Some(g) = geo {
            let _ = w.set_size(g.size);
            let _ = w.set_position(g.pos);
            if g.maximized {
                let _ = w.maximize();
            }
            let _ = w.show();
        }
        let _ = w.set_focus();
    });
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

/// Où trouver les sauvegardes d'un jeu : fichier de sauvegarde, rangement de l'émulateur, dossier des copies.
struct SaveCtx {
    save: Option<PathBuf>,
    layout: saves::SaveLayout,
    backups: PathBuf,
}

/// Console d'un jeu. Seule la DS est prise en charge pour l'instant.
fn game_console(_game: &str) -> &'static str {
    "ds"
}

/// Émulateur utilisé pour un jeu et son dossier, s'il est configuré.
fn game_emulator(state: &AppState, game: &str) -> Option<(&'static dyn Emulator, Option<PathBuf>)> {
    let s = state.settings.lock().unwrap();
    let e = s.emulator_for(game_console(game))?;
    Some((e, s.dir_of(e)))
}

fn save_ctx(app: &AppHandle, state: &AppState, game: &str) -> Result<SaveCtx, String> {
    let layout = match game_emulator(state, game) {
        Some((e, Some(dir))) => e.save_layout(&dir),
        // Aucun émulateur configuré : convention de melonDS, à côté de la ROM
        _ => saves::SaveLayout { save_dir: None, state_dir: None, save_ext: "sav", state_ext: "ml" },
    };
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
        save: saves::save_file(game, &layout),
        layout,
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
    Ok(saves::get(Path::new(&path), &ctx.layout, &ctx.backups))
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
        return Err("Ferme l'émulateur avant de restaurer une sauvegarde.".into());
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
    let e = game_emulator(&state, &path).ok_or("Aucun émulateur pour ce jeu.")?.0;
    run_emulator(app, &state, e, Some(path))
}

/// Ouvre un émulateur sans jeu, pour régler les manettes, les écrans ou les BIOS directement dans l'émulateur.
#[tauri::command]
fn open_emulator(app: AppHandle, state: State<AppState>, emu: String) -> Result<(), String> {
    run_emulator(app, &state, find_emu(&emu)?, None)
}

/// Lance l'émulateur. Le frontend met ensuite VemuZ en veille (`sleep_window`).
fn run_emulator(app: AppHandle, state: &AppState, emu: &'static dyn Emulator, rom: Option<String>) -> Result<(), String> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("Un jeu est déjà en cours.".into());
    }
    let (dir, options) = {
        let s = state.settings.lock().unwrap();
        let entry = s.emulators.get(emu.id()).cloned().unwrap_or_default();
        (entry.dir, entry.options)
    };
    // Copie de sécurité avant de jouer
    if let Some(r) = &rom {
        auto_backup(&app, r);
    }
    let child = match dir {
        Some(d) => emu.launch(Path::new(&d), rom.as_deref().map(Path::new), &options),
        None => Err(format!("Dossier de {} non configuré (voir Réglages).", emu.name())),
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

    let started = std::time::Instant::now();
    std::thread::spawn(move || {
        let _ = child.wait();
        // Temps de jeu de la partie, compté à la fermeture de l'émulateur
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
        app.state::<AppState>().running.store(false, Ordering::SeqCst);
        wake_window(&app);
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
            let mut settings: Settings = storage::load(handle, SETTINGS_FILE);
            if settings.migrate() {
                let _ = storage::save(handle, SETTINGS_FILE, &settings);
            }
            *state.settings.lock().unwrap() = settings;
            *state.games.lock().unwrap() = storage::load(handle, GAMES_FILE);
            covers::start(handle);
            tray(app)?;
            Ok(())
        })
        .on_window_event(|w, e| {
            // Fermeture par la croix pendant un jeu : on retient la place de la fenêtre pour la recréer
            if let WindowEvent::CloseRequested { .. } = e
                && let Some(w) = w.app_handle().get_webview_window(w.label())
            {
                remember_geometry(&w);
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            list_emulators,
            set_emulator_dir,
            set_launch_options,
            set_console_emulator,
            get_emulator_settings,
            set_emulator_settings,
            get_bindings,
            set_bindings,
            detect_emulators,
            set_cover_size,
            set_theme,
            set_pad_directions,
            sleep_window,
            get_resume,
            set_keep_window,
            open_emulator,
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
        .build(tauri::generate_context!())
        .expect("erreur au démarrage de VemuZ")
        .run(|app, event| {
            // Pendant un jeu, fermer la fenêtre ne quitte pas VemuZ : elle reviendra à la fin de la partie
            if let RunEvent::ExitRequested { code: None, api, .. } = event
                && app.state::<AppState>().running.load(Ordering::SeqCst)
            {
                api.prevent_exit();
            }
        });
}

/// Icône dans la zone de notification : rouvre VemuZ à tout moment, même pendant un jeu.
fn tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Ouvrir VemuZ", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter VemuZ", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::with_id("vemuz")
        .tooltip("VemuZ")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id().as_ref() {
            "open" => wake_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                wake_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reprend_les_reglages_de_la_v1_0() {
        let old = r#"{"melonds_dir":"C:/melonDS","options":{"fullscreen":true},"theme":"light"}"#;
        let mut s: Settings = serde_json::from_str(old).unwrap();
        assert!(s.migrate());
        let e = &s.emulators["melonds"];
        assert_eq!((e.dir.as_deref(), e.options.fullscreen), (Some("C:/melonDS"), true));
        assert_eq!(s.emulator_for("ds").map(|e| e.id()), Some("melonds"));
        // L'ancien format n'est plus écrit
        let out = serde_json::to_string(&s).unwrap();
        assert!(!out.contains("melonds_dir") && out.contains("\"theme\":\"light\""));
        assert!(!s.migrate(), "rien à reprendre la deuxième fois");
    }
}
