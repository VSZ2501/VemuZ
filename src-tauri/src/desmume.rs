//! Connecteur DeSmuME (Nintendo DS), version Windows.
//! DeSmuME range desmume.ini, Battery (sauvegardes .dsv) et StateSlots (.ds0 à .ds9) dans son dossier.
//! Une seule commande par bouton dans [Controls] : touche du clavier (code Windows) ou manette
//! (0x8000 | n° de manette << 8 | code DirectInput).

use crate::emulator::{
    scan, usual_bases, Binding, Bindings, Emulator, Kind, LaunchOptions, Preset, SettingDef, Values,
};
use crate::ini::Ini;
use crate::saves::SaveLayout;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub struct DeSmuME;

/// Bit qui distingue une commande de manette d'une touche du clavier
const JOY: i64 = 0x8000;

/// Moteurs 3D de DeSmuME (main.h) : 1 OpenGL 3.2, 2 logiciel, 3 ancien OpenGL
const GL: i64 = 1;
const SOFT: i64 = 2;
const GL_OLD: i64 = 3;

/// Boutons de la console : nom VemuZ (celui de melonDS) et clé de [Controls]
const BUTTONS: [&str; 12] = ["A", "B", "X", "Y", "L", "R", "Start", "Select", "Up", "Down", "Left", "Right"];

/// Raccourcis réglables : nom VemuZ et clé de [Hotkeys] (clavier seulement)
const HOTKEYS: [(&str, &str); 5] = [
    ("HK_Pause", "Pause"),
    ("HK_FastForward", "FastForward"),
    ("HK_SwapScreens", "LCDsSwap"),
    ("HK_SpeedUp", "IncreaseSpeed"),
    ("HK_SpeedDown", "DecreaseSpeed"),
];

/// Vitesses du jeu de DeSmuME (throttle.cpp), dans l'ordre de « FPS Scaler Index » ; 5 = vitesse normale
const SPEEDS: [&str; 13] =
    ["400 %", "200 %", "175 %", "150 %", "125 %", "100 % (normale)", "75 %", "50 %", "37 %", "25 %", "16 %", "12 %", "6 %"];
const NORMAL_SPEED: i64 = 5;

/// Réglages simplifiés exposés dans VemuZ.
#[derive(Serialize, Deserialize)]
struct Config {
    /// 0 verticale, 1 horizontale, 2 un seul écran
    screen_layout: i64,
    screen_swap: bool,
    screen_filter: bool,
    /// Rendu OpenGL (rapide) au lieu du rendu logiciel
    fast_renderer: bool,
    /// Multiple de la définition d'origine (1 à 8)
    resolution: i64,
    /// 0 à 100
    volume: i64,
    /// 0 japonais, 1 anglais, 2 français, 3 allemand, 4 italien, 5 espagnol
    language: i64,
    jit: bool,
    limit_fps: bool,
    pause_on_blur: bool,
    /// Indice dans SPEEDS
    speed: i64,
}

fn config_path(dir: &Path) -> Result<PathBuf, String> {
    let p = dir.join("desmume.ini");
    if p.is_file() {
        Ok(p)
    } else {
        Err("Configuration de DeSmuME introuvable : ouvre DeSmuME une fois, puis ferme-le pour qu'il la crée.".into())
    }
}

fn read(ini: &Ini) -> Config {
    Config {
        screen_layout: ini.int("Video", "LCDsLayout", 0).clamp(0, 2),
        screen_swap: ini.int("Video", "LCDsSwap", 0) == 1,
        screen_filter: ini.bool("Video", "Display Method Filter", false),
        fast_renderer: matches!(ini.int("3D", "Renderer", SOFT), GL | GL_OLD),
        resolution: ini.int("3D", "PrescaleHD", 1).clamp(1, 8),
        volume: ini.int("Sound", "Volume", 100).clamp(0, 100),
        language: ini.int("Firmware", "Language", 1).clamp(0, 5),
        jit: ini.bool("Emulation", "CPUmode", false),
        limit_fps: ini.bool("FrameLimit", "FrameLimit", true),
        pause_on_blur: ini.bool("Focus", "BackgroundPause", false),
        speed: ini.int("Video", "FPS Scaler Index", NORMAL_SPEED).clamp(0, SPEEDS.len() as i64 - 1),
    }
}

fn write(ini: &mut Ini, c: &Config) {
    let b = |v: bool| if v { "1" } else { "0" };
    ini.set("Video", "LCDsLayout", &c.screen_layout.clamp(0, 2).to_string());
    ini.set("Video", "LCDsSwap", b(c.screen_swap));
    ini.set("Video", "Display Method Filter", b(c.screen_filter));
    // Garde l'ancien OpenGL s'il était choisi dans DeSmuME
    let current = ini.int("3D", "Renderer", SOFT);
    let renderer = match (c.fast_renderer, current) {
        (true, GL_OLD) => GL_OLD,
        (true, _) => GL,
        (false, _) => SOFT,
    };
    ini.set("3D", "Renderer", &renderer.to_string());
    ini.set("3D", "PrescaleHD", &c.resolution.clamp(1, 8).to_string());
    ini.set("Sound", "Volume", &c.volume.clamp(0, 100).to_string());
    ini.set("Firmware", "Language", &c.language.clamp(0, 5).to_string());
    ini.set("Emulation", "CPUmode", b(c.jit));
    ini.set("FrameLimit", "FrameLimit", b(c.limit_fps));
    ini.set("Focus", "BackgroundPause", b(c.pause_on_blur));
    ini.set("Video", "FPS Scaler Index", &c.speed.clamp(0, SPEEDS.len() as i64 - 1).to_string());
}

fn to_values(c: &Config) -> Values {
    match serde_json::to_value(c) {
        Ok(Value::Object(m)) => m,
        _ => Values::new(),
    }
}

fn values(v: Value) -> Values {
    match v {
        Value::Object(m) => m,
        _ => Values::new(),
    }
}

fn def(id: &'static str, group: &'static str, label: &'static str, help: Option<&'static str>, kind: Kind) -> SettingDef {
    SettingDef { id, label, help, group, advanced: false, kind }
}

fn adv(id: &'static str, label: &'static str, help: Option<&'static str>, kind: Kind) -> SettingDef {
    SettingDef { id, label, help, group: "Avancé", advanced: true, kind }
}

/// Dossier réglé dans [PathSettings], relatif au dossier de DeSmuME, sinon le dossier par défaut.
fn path_setting(ini: Option<&Ini>, dir: &Path, key: &str, default: &str) -> PathBuf {
    match ini.and_then(|i| i.get("PathSettings", key)).filter(|v| !v.is_empty()) {
        Some(v) => dir.join(v),
        None => dir.join(default),
    }
}

impl Emulator for DeSmuME {
    fn id(&self) -> &'static str {
        "desmume"
    }

    fn name(&self) -> &'static str {
        "DeSmuME"
    }

    fn console(&self) -> &'static str {
        "ds"
    }

    /// Le nom de l'exécutable change avec la version (DeSmuME_0.9.13_x64.exe) : premier .exe qui commence par « desmume »,
    /// la version 64 bits en priorité.
    fn executable(&self, dir: &Path) -> Option<PathBuf> {
        let mut exes: Vec<PathBuf> = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .filter(|p| {
                p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    let n = n.to_lowercase();
                    n.starts_with("desmume") && n.ends_with(".exe")
                })
            })
            .collect();
        exes.sort_by_key(|p| !p.to_string_lossy().to_lowercase().contains("x64"));
        exes.into_iter().next()
    }

    fn launch(&self, dir: &Path, rom: Option<&Path>, options: &LaunchOptions) -> Result<Child, String> {
        let exe = self.executable(dir).ok_or_else(|| format!("DeSmuME introuvable dans {}", dir.display()))?;
        let mut cmd = Command::new(exe);
        // DeSmuME cherche desmume.ini et ses dossiers dans le dossier courant
        cmd.current_dir(dir);
        if options.fullscreen {
            cmd.arg("--windowed-fullscreen");
        }
        if let Some(rom) = rom {
            cmd.arg(rom);
        }
        cmd.spawn().map_err(|e| format!("Lancement impossible : {e}"))
    }

    fn detect(&self) -> Vec<PathBuf> {
        scan(self, &usual_bases(), "desmume")
    }

    fn download_url(&self) -> Option<&'static str> {
        Some("https://desmume.org/download/")
    }

    fn settings_schema(&self) -> Vec<SettingDef> {
        use Kind::*;
        const SCREENS: &str = "Écrans";
        const GRAPHICS: &str = "Graphismes";
        const AUDIO: &str = "Son et langue";
        vec![
            def(
                "screen_layout",
                SCREENS,
                "Disposition",
                None,
                Choice {
                    options: vec![
                        (json!(0), "Verticale (haut et bas)"),
                        (json!(1), "Horizontale (côte à côte)"),
                        (json!(2), "Un seul écran"),
                    ],
                },
            ),
            def("screen_swap", SCREENS, "Échanger les deux écrans", None, Toggle),
            def(
                "screen_filter",
                SCREENS,
                "Adoucir l'image",
                Some("Avec l'affichage OpenGL de DeSmuME (menu Affichage de l'émulateur)."),
                Toggle,
            ),
            def(
                "fast_renderer",
                GRAPHICS,
                "Rendu 3D",
                Some("Rapide : utilise la carte graphique. Compatible : plus lent, mais fonctionne partout."),
                Choice { options: vec![(json!(false), "Compatible (logiciel)"), (json!(true), "Rapide (OpenGL)")] },
            ),
            def(
                "resolution",
                GRAPHICS,
                "Définition de la 3D",
                Some("Rend les jeux en 3D plus nets, mais demande plus de puissance."),
                Choice {
                    options: vec![
                        (json!(1), "D'origine"),
                        (json!(2), "x2"),
                        (json!(3), "x3"),
                        (json!(4), "x4"),
                        (json!(6), "x6"),
                        (json!(8), "x8"),
                    ],
                },
            ),
            def("volume", AUDIO, "Volume", None, Range { min: 0, max: 100, step: 1, unit: "%" }),
            def(
                "language",
                AUDIO,
                "Langue des jeux",
                Some("Les jeux disponibles en plusieurs langues utilisent celle-ci."),
                Choice {
                    options: vec![
                        (json!(1), "Anglais"),
                        (json!(2), "Français"),
                        (json!(3), "Allemand"),
                        (json!(4), "Italien"),
                        (json!(5), "Espagnol"),
                        (json!(0), "Japonais"),
                    ],
                },
            ),
            adv(
                "jit",
                "Accélération JIT",
                Some("Rend l'émulation bien plus rapide. À désactiver si un jeu plante."),
                Toggle,
            ),
            adv(
                "speed",
                "Vitesse du jeu",
                Some(
                    "S'applique en permanence. Les raccourcis Accélérer et Ralentir la changent pendant la partie. L'avance rapide, elle, va aussi vite que ton PC le permet.",
                ),
                Choice { options: SPEEDS.iter().enumerate().map(|(i, l)| (json!(i), *l)).collect() },
            ),
            adv("limit_fps", "Limiter la vitesse", Some("Décoché, le jeu tourne aussi vite que ton PC le permet."), Toggle),
            adv("pause_on_blur", "Mettre en pause quand la fenêtre perd le focus", None, Toggle),
        ]
    }

    fn presets(&self) -> Vec<Preset> {
        vec![
            Preset {
                id: "recommended",
                label: "Conseillé",
                help: "Fonctionne sur tous les PC, image d'origine.",
                values: values(json!({ "fast_renderer": false, "resolution": 1, "jit": false, "limit_fps": true })),
            },
            Preset {
                id: "performance",
                label: "Performance",
                help: "Pour un PC modeste : rendu rapide et accélération JIT.",
                values: values(json!({ "fast_renderer": true, "resolution": 1, "jit": true, "limit_fps": true })),
            },
            Preset {
                id: "quality",
                label: "Qualité",
                help: "3D quatre fois plus nette, demande une carte graphique correcte.",
                values: values(json!({ "fast_renderer": true, "resolution": 4, "jit": true, "limit_fps": true })),
            },
        ]
    }

    fn read_settings(&self, dir: &Path) -> Result<Values, String> {
        Ok(to_values(&read(&Ini::load(&config_path(dir)?)?)))
    }

    fn write_settings(&self, dir: &Path, values: &Values) -> Result<Values, String> {
        let path = config_path(dir)?;
        let mut ini = Ini::load(&path)?;
        let mut merged = to_values(&read(&ini));
        merged.extend(values.clone());
        let c: Config = serde_json::from_value(Value::Object(merged)).map_err(|_| "Réglage invalide.".to_string())?;
        write(&mut ini, &c);
        ini.save(&path)?;
        self.read_settings(dir)
    }

    fn bindings_format(&self) -> Option<&'static str> {
        Some("desmume")
    }

    fn read_bindings(&self, dir: &Path, kind: Binding) -> Result<Bindings, String> {
        Ok(read_bindings(&Ini::load(&config_path(dir)?)?, kind))
    }

    fn write_bindings(&self, dir: &Path, kind: Binding, keys: &Bindings) -> Result<Bindings, String> {
        let path = config_path(dir)?;
        let mut ini = Ini::load(&path)?;
        write_bindings(&mut ini, kind, keys);
        ini.save(&path)?;
        Ok(read_bindings(&ini, kind))
    }

    fn save_layout(&self, dir: &Path) -> SaveLayout {
        let ini = config_path(dir).ok().and_then(|p| Ini::load(&p).ok());
        SaveLayout {
            save_dir: Some(path_setting(ini.as_ref(), dir, "Battery", "Battery")),
            state_dir: Some(path_setting(ini.as_ref(), dir, "States", "StateSlots")),
            save_ext: "dsv",
            state_ext: "ds",
        }
    }
}

/// Clavier : boutons tenus par une touche et raccourcis. Manette : boutons tenus par la manette.
/// -1 : non assigné de ce côté.
fn read_bindings(ini: &Ini, kind: Binding) -> Bindings {
    let mut out = Bindings::new();
    for b in BUTTONS {
        let v = ini.int("Controls", b, 0);
        let mine = match kind {
            Binding::Keyboard => v > 0 && v < JOY,
            Binding::Pad => v >= JOY,
        };
        out.insert(b.to_string(), if mine { v } else { -1 });
    }
    if let Binding::Keyboard = kind {
        for (name, key) in HOTKEYS {
            let v = ini.int("Hotkeys", key, 0);
            out.insert(name.to_string(), if v > 0 { v } else { -1 });
        }
    }
    out
}

/// Une commande assignée remplace celle de l'autre côté (un seul emplacement par bouton).
/// Désassigner n'efface que si la commande actuelle est bien de ce côté.
fn write_bindings(ini: &mut Ini, kind: Binding, keys: &Bindings) {
    for b in BUTTONS {
        let Some(&v) = keys.get(b) else { continue };
        let valid = match kind {
            Binding::Keyboard => v > 0 && v < JOY,
            Binding::Pad => v >= JOY && v <= 0xFFFF,
        };
        if valid {
            ini.set("Controls", b, &v.to_string());
        } else if v < 0 {
            let cur = ini.int("Controls", b, 0);
            let mine = match kind {
                Binding::Keyboard => cur > 0 && cur < JOY,
                Binding::Pad => cur >= JOY,
            };
            if mine {
                ini.set("Controls", b, "0");
            }
        }
    }
    if let Binding::Keyboard = kind {
        for (name, key) in HOTKEYS {
            let Some(&v) = keys.get(name) else { continue };
            let v = if v > 0 && v < JOY { v } else { 0 };
            ini.set("Hotkeys", key, &v.to_string());
            // Touche seule, sans Ctrl, Maj ni Alt
            ini.set("Hotkeys", &format!("{key} MOD"), "0");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vemuz_desmume_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    const SAMPLE: &str = "[3D]\r\nRenderer=2\r\n[Video]\r\nLCDsLayout=0\r\n[Hotkeys]\r\nPause=19\r\nPause MOD=0\r\nFastForward=16\r\nFastForward MOD=0\r\n[Controls]\r\nLeft=37\r\nRight=39\r\nUp=38\r\nDown=40\r\nStart=13\r\nA=69\r\nB=32769\r\n[Sound]\r\nVolume=100\r\n";

    #[test]
    fn trouve_l_executable_versionne() {
        let d = temp_dir("exe");
        fs::write(d.join("DeSmuME_0.9.13_x86.exe"), b"").unwrap();
        fs::write(d.join("DeSmuME_0.9.13_x64.exe"), b"").unwrap();
        fs::write(d.join("README"), b"").unwrap();
        let exe = DeSmuME.executable(&d).map(|p| p.file_name().unwrap().to_string_lossy().into_owned());
        let empty = temp_dir("vide");
        let none = DeSmuME.executable(&empty);
        fs::remove_dir_all(&d).unwrap();
        fs::remove_dir_all(&empty).unwrap();
        assert_eq!(exe.as_deref(), Some("DeSmuME_0.9.13_x64.exe"));
        assert_eq!(none, None);
    }

    #[test]
    fn reglages_lus_et_ecrits() {
        let d = temp_dir("config");
        fs::write(d.join("desmume.ini"), SAMPLE).unwrap();
        let v = DeSmuME.read_settings(&d).unwrap();
        assert_eq!((v["fast_renderer"].clone(), v["volume"].clone(), v["limit_fps"].clone()), (json!(false), json!(100), json!(true)));

        let mut change = Values::new();
        change.insert("fast_renderer".into(), json!(true));
        change.insert("resolution".into(), json!(4));
        change.insert("jit".into(), json!(true));
        assert_eq!(v["speed"], json!(NORMAL_SPEED), "vitesse absente : normale");
        change.insert("speed".into(), json!(1));
        let back = DeSmuME.write_settings(&d, &change).unwrap();
        let ini = Ini::load(&d.join("desmume.ini")).unwrap();
        fs::remove_dir_all(&d).unwrap();

        assert_eq!((back["fast_renderer"].clone(), back["resolution"].clone()), (json!(true), json!(4)));
        assert_eq!((ini.int("3D", "Renderer", 0), ini.int("3D", "PrescaleHD", 0), ini.int("Emulation", "CPUmode", 0)), (GL, 4, 1));
        assert_eq!(ini.int("Controls", "A", 0), 69, "les touches ne bougent pas");
        assert_eq!(ini.int("Video", "FPS Scaler Index", 0), 1, "200 %");
    }

    #[test]
    fn un_seul_emplacement_par_bouton() {
        let d = temp_dir("touches");
        fs::write(d.join("desmume.ini"), SAMPLE).unwrap();
        let kb = DeSmuME.read_bindings(&d, Binding::Keyboard).unwrap();
        let pad = DeSmuME.read_bindings(&d, Binding::Pad).unwrap();
        assert_eq!((kb["A"], kb["B"], kb["HK_Pause"], kb["X"]), (69, -1, 19, -1));
        assert_eq!((pad["A"], pad["B"]), (-1, 32769));
        assert!(!pad.contains_key("HK_Pause"), "raccourcis au clavier seulement");

        // Manette sur A : remplace la touche ; désassigner B côté clavier ne touche pas à la manette
        let mut p = pad.clone();
        p.insert("A".into(), JOY | 8);
        DeSmuME.write_bindings(&d, Binding::Pad, &p).unwrap();
        let mut k = DeSmuME.read_bindings(&d, Binding::Keyboard).unwrap();
        k.insert("B".into(), -1);
        k.insert("HK_SwapScreens".into(), 34);
        k.insert("HK_SpeedUp".into(), 187);
        let k2 = DeSmuME.write_bindings(&d, Binding::Keyboard, &k).unwrap();
        let ini = Ini::load(&d.join("desmume.ini")).unwrap();
        fs::remove_dir_all(&d).unwrap();

        assert_eq!(k2["A"], -1, "A est passé à la manette");
        assert_eq!(ini.int("Controls", "A", 0), JOY | 8);
        assert_eq!(ini.int("Controls", "B", 0), 32769, "la manette de B est gardée");
        assert_eq!((ini.int("Hotkeys", "LCDsSwap", 0), ini.int("Hotkeys", "LCDsSwap MOD", 9)), (34, 0));
        assert_eq!(ini.int("Hotkeys", "IncreaseSpeed", 0), 187);
    }

    #[test]
    fn sauvegardes_dans_battery() {
        let d = temp_dir("saves");
        let l = DeSmuME.save_layout(&d);
        fs::remove_dir_all(&d).unwrap();
        assert_eq!(l.save_dir, Some(d.join("Battery")));
        assert_eq!(l.state_dir, Some(d.join("StateSlots")));
        assert_eq!((l.save_ext, l.state_ext), ("dsv", "ds"));
    }

    #[test]
    fn prereglages_valides() {
        let ini = Ini::load_str(SAMPLE);
        for p in DeSmuME.presets() {
            let mut merged = to_values(&read(&ini));
            merged.extend(p.values.clone());
            assert!(serde_json::from_value::<Config>(Value::Object(merged)).is_ok(), "{} invalide", p.id);
        }
    }
}
