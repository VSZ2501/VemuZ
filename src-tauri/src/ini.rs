//! Éditeur de lignes pour les fichiers .ini de Windows (DeSmuME).
//! Seules les clés voulues changent. Le fichier est en page de code ANSI : chaque octet est lu
//! comme un caractère et réécrit tel quel, pour ne pas abîmer les chemins accentués.

use std::fs;
use std::path::Path;

pub struct Ini {
    lines: Vec<String>,
    eol: &'static str,
}

fn section_of(line: &str) -> Option<&str> {
    line.trim().strip_prefix('[')?.strip_suffix(']')
}

impl Ini {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let text: String = bytes.iter().map(|&b| b as char).collect();
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        Ok(Ini { lines: text.lines().map(String::from).collect(), eol })
    }

    #[cfg(test)]
    pub fn load_str(text: &str) -> Self {
        Ini { lines: text.lines().map(String::from).collect(), eol: "\r\n" }
    }

    /// Valeur d'une clé. Sections et clés sans distinction de casse, comme Windows.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let mut cur = "";
        for l in &self.lines {
            if let Some(s) = section_of(l) {
                cur = s;
            } else if cur.eq_ignore_ascii_case(section)
                && let Some((k, v)) = l.split_once('=')
                && k.trim().eq_ignore_ascii_case(key)
            {
                return Some(v.trim());
            }
        }
        None
    }

    /// Entier au début de la valeur, comme GetPrivateProfileInt.
    pub fn int(&self, section: &str, key: &str, default: i64) -> i64 {
        let Some(v) = self.get(section, key) else { return default };
        let end = v.char_indices().find(|&(i, c)| !(c.is_ascii_digit() || (i == 0 && c == '-'))).map_or(v.len(), |(i, _)| i);
        v[..end].parse().unwrap_or(default)
    }

    pub fn bool(&self, section: &str, key: &str, default: bool) -> bool {
        self.int(section, key, default as i64) != 0
    }

    /// Remplace la valeur d'une clé, ou l'ajoute à la fin de sa section (créée si besoin).
    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        let new_line = format!("{key}={value}");
        let mut cur = String::new();
        let mut last_of_section = None;
        for i in 0..self.lines.len() {
            if let Some(s) = section_of(&self.lines[i]) {
                cur = s.to_string();
                if cur.eq_ignore_ascii_case(section) {
                    last_of_section = Some(i);
                }
            } else if cur.eq_ignore_ascii_case(section) {
                if let Some((k, _)) = self.lines[i].split_once('=')
                    && k.trim().eq_ignore_ascii_case(key)
                {
                    self.lines[i] = new_line;
                    return;
                }
                if !self.lines[i].trim().is_empty() {
                    last_of_section = Some(i);
                }
            }
        }
        match last_of_section {
            Some(i) => self.lines.insert(i + 1, new_line),
            None => {
                self.lines.push(format!("[{section}]"));
                self.lines.push(new_line);
            }
        }
    }

    /// Copie de sécurité à la première modification, puis écriture atomique.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let bak = path.with_extension("ini.bak");
        if !bak.exists() {
            fs::copy(path, &bak).map_err(|e| format!("Copie de sécurité impossible : {e}"))?;
        }
        let mut out = self.lines.join(self.eol);
        out.push_str(self.eol);
        // Tous les caractères viennent d'octets ou de valeurs ASCII : ils tiennent sur un octet
        let bytes: Vec<u8> = out.chars().map(|c| c as u32 as u8).collect();
        let tmp = path.with_extension("ini.vemuz");
        fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_modifie_et_garde_les_accents() {
        let path = std::env::temp_dir().join(format!("vemuz_test_{}.ini", std::process::id()));
        // « é » en Windows-1252 : un seul octet 0xE9
        let mut src = b"[3D]\r\nRenderer=2\r\n[PathSettings]\r\nRoms=C:\\Jeux\\Pok\xE9mon\r\n[Sound]\r\nVolume=100\r\n".to_vec();
        src.extend_from_slice(b"[Controls]\r\nA=88 ; commentaire\r\n");
        fs::write(&path, &src).unwrap();

        let mut ini = Ini::load(&path).unwrap();
        assert_eq!(ini.int("3d", "renderer", 0), 2, "sans distinction de casse");
        assert_eq!(ini.int("Controls", "A", 0), 88, "entier en tête de valeur");
        assert_eq!(ini.int("Sound", "Absent", 7), 7);
        ini.set("3D", "Renderer", "1");
        ini.set("3D", "PrescaleHD", "4"); // clé absente : ajoutée dans la section
        ini.set("Emulation", "CPUmode", "1"); // section absente : créée
        ini.save(&path).unwrap();

        let out = fs::read(&path).unwrap();
        let back = Ini::load(&path).unwrap();
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("ini.bak"));

        assert!(out.windows(9).any(|w| w == b"Pok\xE9mon\r\n"), "octet accentué conservé");
        assert!(out.starts_with(b"[3D]\r\nRenderer=1\r\nPrescaleHD=4\r\n[PathSettings]"));
        assert_eq!(back.int("Emulation", "CPUmode", 0), 1);
        assert_eq!(back.int("Sound", "Volume", 0), 100);
    }
}
