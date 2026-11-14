//! Lecture et modification d'une sélection de réglages dans melonDS.toml.
//! Éditeur de lignes minimal : seules les clés voulues changent, le reste du fichier reste intact.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Réglages simplifiés exposés à l'utilisateur.
#[derive(Serialize, Deserialize, Clone)]
pub struct EmuConfig {
    /// 0 naturelle, 1 verticale, 2 horizontale, 3 hybride
    pub screen_layout: i64,
    pub screen_swap: bool,
    pub integer_scaling: bool,
    pub screen_filter: bool,
    /// Rendu OpenGL (rapide) au lieu du rendu logiciel (compatible)
    pub fast_renderer: bool,
    /// Définition de la 3D en rendu OpenGL : multiple de 256 x 192 (1 à 8)
    pub resolution: i64,
    pub limit_fps: bool,
    pub jit: bool,
    /// 0 à 100
    pub volume: i64,
    /// 0 japonais, 1 anglais, 2 français, 3 allemand, 4 italien, 5 espagnol
    pub language: i64,
    pub direct_boot: bool,
    pub pause_on_blur: bool,
    /// Vitesse de l'avance rapide en % de la vitesse normale (100 à 1000)
    pub fast_forward: i64,
}

/// melonDS.toml à côté de l'exécutable (mode portable), sinon dans %APPDATA%\melonDS.
pub fn config_path(dir: &Path) -> Option<PathBuf> {
    let local = dir.join("melonDS.toml");
    if local.is_file() {
        return Some(local);
    }
    let p = PathBuf::from(std::env::var_os("APPDATA")?).join("melonDS").join("melonDS.toml");
    p.is_file().then_some(p)
}

fn section_of(line: &str) -> Option<&str> {
    line.trim().strip_prefix('[')?.strip_suffix(']')
}

fn get<'a>(lines: &'a [String], section: &str, key: &str) -> Option<&'a str> {
    let mut cur = "";
    for l in lines {
        if let Some(s) = section_of(l) {
            cur = s;
        } else if cur == section
            && let Some((k, v)) = l.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim());
        }
    }
    None
}

/// Valeur texte TOML ("..."), sans les guillemets ni les échappements simples.
fn unquote(v: &str) -> String {
    let v = v.trim();
    let v = v.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(v);
    v.replace("\\\\", "\\").replace("\\\"", "\"")
}

/// Dossiers des sauvegardes (.sav) et des sauvegardes rapides (.ml0 à .ml9) réglés dans melonDS.
/// None : à côté de la ROM. Un chemin relatif part du dossier de melonDS.
pub fn asset_dirs(config: &Path, melonds_dir: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let Ok(content) = fs::read_to_string(config) else { return (None, None) };
    let l: Vec<String> = content.lines().map(String::from).collect();
    let dir = |key: &str| {
        let v = unquote(get(&l, "Instance0", key)?);
        (!v.is_empty()).then(|| melonds_dir.join(v))
    };
    (dir("SaveFilePath"), dir("SavestatePath"))
}

fn get_bool(lines: &[String], section: &str, key: &str, default: bool) -> bool {
    match get(lines, section, key) {
        Some("true") => true,
        Some("false") => false,
        _ => default,
    }
}

fn get_int(lines: &[String], section: &str, key: &str, default: i64) -> i64 {
    get(lines, section, key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn get_float(lines: &[String], section: &str, key: &str, default: f64) -> f64 {
    get(lines, section, key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Remplace la valeur d'une clé, ou l'ajoute dans sa section (créée si besoin).
fn set(lines: &mut Vec<String>, section: &str, key: &str, value: &str) {
    let new_line = format!("{key} = {value}");
    let mut cur = String::new();
    let mut header = None;
    for i in 0..lines.len() {
        if let Some(s) = section_of(&lines[i]) {
            cur = s.to_string();
            if cur == section {
                header = Some(i);
            }
        } else if cur == section
            && let Some((k, _)) = lines[i].split_once('=')
            && k.trim() == key
        {
            lines[i] = new_line;
            return;
        }
    }
    if section.is_empty() {
        lines.insert(0, new_line);
    } else if let Some(h) = header {
        lines.insert(h + 1, new_line);
    } else {
        if lines.last().is_some_and(|l| !l.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push(format!("[{section}]"));
        lines.push(new_line);
    }
}

const WIN: &str = "Instance0.Window0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touches_lues_ecrites_et_noms_inconnus_ignores() {
        let path = std::env::temp_dir().join("vemuz_test_keys.toml");
        fs::write(&path, "[Instance0.Keyboard]\nA = -1\nB = -1\n\n[Instance0.Joystick]\nA = 7\n").unwrap();
        let mut k = read_keys(&path).unwrap();
        assert_eq!(k["A"], -1);
        assert_eq!(k["HK_Pause"], -1); // clé absente : non assignée
        k.insert("A".into(), 0x58);
        k.insert("Up".into(), 0x01000013); // clé absente : ajoutée dans la bonne section
        k.insert("Inconnu".into(), 5);
        write_keys(&path, &k).unwrap();
        let out = fs::read_to_string(&path).unwrap();
        assert!(!out.contains("Inconnu"));
        assert!(out.contains("[Instance0.Joystick]\nA = 7"), "la manette n'est pas touchée");
        let back = read_keys(&path).unwrap();
        assert_eq!((back["A"], back["Up"], back["B"]), (0x58, 0x01000013, -1));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("toml.bak"));
    }

    #[test]
    fn manette_lue_ecrite_sans_toucher_au_clavier() {
        let path = std::env::temp_dir().join(format!("vemuz_test_joy_{}.toml", std::process::id()));
        fs::write(&path, "[Instance0.Keyboard]\nA = 88\n\n[Instance0.Joystick]\nA = -1\nB = 3\n").unwrap();
        let mut j = read_joy(&path).unwrap();
        assert_eq!((j["A"], j["B"], j["Up"]), (-1, 3, -1));
        // Bouton 1, direction haut de la croix 0, axe 5 en positif
        j.insert("A".into(), 1);
        j.insert("Up".into(), 0x101);
        j.insert("HK_FastForward".into(), 0x1501FFFF);
        // Flèche (bouton 11) et stick gauche vers le haut dans la même touche
        j.insert("Down".into(), 0x0111000B);
        write_joy(&path, &j).unwrap();
        let back = read_joy(&path).unwrap();
        let keys = read_keys(&path).unwrap();
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("toml.bak"));
        assert_eq!((back["A"], back["Up"], back["HK_FastForward"], back["B"]), (1, 0x101, 0x1501FFFF, 3));
        assert_eq!(keys["A"], 88, "le clavier n'est pas touché");
        assert_eq!(back["Down"], 0x0111000B, "bouton et axe combinés conservés");
    }

    const SAMPLE: &str = "RecentROM = [\"a.nds\"]\r\nLimitFPS = true\r\n\r\n[Emu]\r\nDirectBoot = true\r\n\r\n[Instance0.Window0]\r\nScreenLayout = 0\r\nScreenSwap = false\r\n\r\n[Instance0.Audio]\r\nVolume = 256\r\n";

    #[test]
    fn lit_modifie_et_conserve_le_reste() {
        let path = std::env::temp_dir().join("vemuz_test_melonds.toml");
        fs::write(&path, SAMPLE).unwrap();
        let mut c = read(&path).unwrap();
        assert_eq!((c.screen_layout, c.volume, c.language, c.jit), (0, 100, 1, false));

        c.screen_layout = 2;
        c.volume = 50;
        c.jit = true; // section [JIT] absente : doit être créée
        c.pause_on_blur = true; // clé racine absente : ajoutée en tête
        c.fast_renderer = true;
        assert_eq!(c.resolution, 1); // section [3D.GL] absente : définition d'origine
        c.resolution = 4;
        assert_eq!(c.fast_forward, 1000); // valeur absente : plafonnée à 1000 %
        c.fast_forward = 200;
        write(&path, &c).unwrap();

        let out = fs::read_to_string(&path).unwrap();
        assert!(out.contains("RecentROM = [\"a.nds\"]"));
        assert!(out.contains("ScreenLayout = 2") && out.contains("Volume = 128"));
        assert!(out.contains("[JIT]\r\nEnable = true"));
        assert!(out.contains("[Screen]\r\nUseGL = true") || out.contains("UseGL = true"));
        assert!(out.contains("FastForwardFPS = 120.0"));
        assert!(out.contains("[3D.GL]\r\nScaleFactor = 4"));
        assert!(out.starts_with("PauseLostFocus = true\r\n") || out.starts_with("FastForwardFPS"));
        assert!(!out.replace("\r\n", "").contains('\n'), "fins de ligne CRLF conservées");
        let back = read(&path).unwrap();
        assert_eq!((back.screen_layout, back.volume, back.jit, back.fast_renderer), (2, 50, true, true));
        assert_eq!(back.fast_forward, 200);
        assert!(path.with_extension("toml.bak").exists());
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("toml.bak"));
    }
}

pub fn read(path: &Path) -> Result<EmuConfig, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let l: Vec<String> = content.lines().map(String::from).collect();
    Ok(EmuConfig {
        screen_layout: get_int(&l, WIN, "ScreenLayout", 0).clamp(0, 3),
        screen_swap: get_bool(&l, WIN, "ScreenSwap", false),
        integer_scaling: get_bool(&l, WIN, "IntegerScaling", false),
        screen_filter: get_bool(&l, WIN, "ScreenFilter", true),
        fast_renderer: get_int(&l, "3D", "Renderer", 0) != 0,
        resolution: get_int(&l, "3D.GL", "ScaleFactor", 1).clamp(1, 8),
        limit_fps: get_bool(&l, "", "LimitFPS", true),
        jit: get_bool(&l, "JIT", "Enable", false),
        volume: (get_int(&l, "Instance0.Audio", "Volume", 256) * 100 + 128) / 256,
        language: get_int(&l, "Instance0.Firmware", "Language", 1).clamp(0, 5),
        direct_boot: get_bool(&l, "Emu", "DirectBoot", true),
        pause_on_blur: get_bool(&l, "", "PauseLostFocus", false),
        fast_forward: {
            let target = get_float(&l, "", "TargetFPS", 60.0).max(1.0);
            let ff = get_float(&l, "", "FastForwardFPS", 1000.0);
            ((ff / target * 100.0).round() as i64).clamp(100, 1000)
        },
    })
}

pub fn write(path: &Path, c: &EmuConfig) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let eol = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let mut l: Vec<String> = content.lines().map(String::from).collect();

    let b = |v: bool| if v { "true" } else { "false" };
    set(&mut l, WIN, "ScreenLayout", &c.screen_layout.clamp(0, 3).to_string());
    set(&mut l, WIN, "ScreenSwap", b(c.screen_swap));
    set(&mut l, WIN, "IntegerScaling", b(c.integer_scaling));
    set(&mut l, WIN, "ScreenFilter", b(c.screen_filter));
    // Le rendu OpenGL exige aussi l'affichage OpenGL
    set(&mut l, "3D", "Renderer", if c.fast_renderer { "1" } else { "0" });
    set(&mut l, "Screen", "UseGL", b(c.fast_renderer));
    set(&mut l, "3D.GL", "ScaleFactor", &c.resolution.clamp(1, 8).to_string());
    set(&mut l, "", "LimitFPS", b(c.limit_fps));
    set(&mut l, "JIT", "Enable", b(c.jit));
    set(&mut l, "Instance0.Audio", "Volume", &((c.volume.clamp(0, 100) * 256 + 50) / 100).to_string());
    set(&mut l, "Instance0.Firmware", "Language", &c.language.clamp(0, 5).to_string());
    set(&mut l, "Emu", "DirectBoot", b(c.direct_boot));
    set(&mut l, "", "PauseLostFocus", b(c.pause_on_blur));
    // L'avance rapide est exprimée en images par seconde dans melonDS
    let target = get_float(&l, "", "TargetFPS", 60.0).max(1.0);
    set(&mut l, "", "FastForwardFPS", &format!("{:.1}", target * c.fast_forward.clamp(100, 1000) as f64 / 100.0));

    save(path, &l, eol)
}

/// Copie de sécurité à la première modification, puis écriture atomique.
fn save(path: &Path, lines: &[String], eol: &str) -> Result<(), String> {
    let bak = path.with_extension("toml.bak");
    if !bak.exists() {
        fs::copy(path, &bak).map_err(|e| format!("Copie de sécurité impossible : {e}"))?;
    }
    let tmp = path.with_extension("toml.vemuz");
    let mut out = lines.join(eol);
    out.push_str(eol);
    fs::write(&tmp, out).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

const KEYBOARD: &str = "Instance0.Keyboard";

/// Boutons de la console et raccourcis modifiables (seuls noms acceptés).
pub const KEYS: [&str; 17] = [
    "A", "B", "X", "Y", "L", "R", "Start", "Select", "Up", "Down", "Left", "Right",
    "HK_FullscreenToggle", "HK_Pause", "HK_FastForward", "HK_Reset", "HK_SwapScreens",
];

const JOYSTICK: &str = "Instance0.Joystick";

/// Valeurs d'une section de touches, par nom ; -1 si non assignée.
fn read_section(path: &Path, section: &str) -> Result<HashMap<String, i64>, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let l: Vec<String> = content.lines().map(String::from).collect();
    Ok(KEYS.iter().map(|k| (k.to_string(), get_int(&l, section, k, -1))).collect())
}

fn write_section(path: &Path, section: &str, keys: &HashMap<String, i64>) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let eol = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let mut l: Vec<String> = content.lines().map(String::from).collect();
    for name in KEYS {
        if let Some(v) = keys.get(name) {
            set(&mut l, section, name, &(*v).max(-1).to_string());
        }
    }
    save(path, &l, eol)
}

/// Touches du clavier : codes Qt.
pub fn read_keys(path: &Path) -> Result<HashMap<String, i64>, String> {
    read_section(path, KEYBOARD)
}

pub fn write_keys(path: &Path, keys: &HashMap<String, i64>) -> Result<(), String> {
    write_section(path, KEYBOARD, keys)
}

/// Boutons de la manette : codes SDL encodés par melonDS (bouton, direction de croix ou axe).
pub fn read_joy(path: &Path) -> Result<HashMap<String, i64>, String> {
    read_section(path, JOYSTICK)
}

pub fn write_joy(path: &Path, keys: &HashMap<String, i64>) -> Result<(), String> {
    write_section(path, JOYSTICK, keys)
}
