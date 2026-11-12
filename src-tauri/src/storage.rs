use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Dossier de données de l'application (créé s'il manque).
pub fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn file_path(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join(name))
}

/// Lit un fichier JSON de l'application. Valeur par défaut s'il est absent ou invalide.
pub fn load<T: DeserializeOwned + Default>(app: &AppHandle, name: &str) -> T {
    file_path(app, name)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save<T: Serialize>(app: &AppHandle, name: &str, value: &T) -> Result<(), String> {
    let path = file_path(app, name)?;
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    // Écriture via un fichier temporaire pour ne jamais laisser un JSON tronqué
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}
