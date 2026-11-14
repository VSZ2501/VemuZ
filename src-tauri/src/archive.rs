//! Lecture des ROMs dans les archives .zip et .7z avec tar.exe (libarchive, livré avec Windows).
//! Aucune dépendance : on liste le contenu et on lit seulement l'en-tête de la ROM.
//! tar.exe écrit les noms dans la page de code ANSI : on ne s'appuie donc que sur l'extension
//! (ASCII) et on laisse melonDS choisir la ROM dans l'archive.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Motifs des ROMs dans l'archive (la casse de l'extension varie)
const PATTERNS: [&str; 3] = ["*.nds", "*.NDS", "*.Nds"];

pub fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip") || e.eq_ignore_ascii_case("7z"))
}

pub(crate) fn tar() -> Command {
    // Chemin complet : on n'exécute pas un tar.exe trouvé ailleurs dans le PATH
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    let mut cmd = Command::new(root.join("System32").join("tar.exe"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // pas de fenêtre console
    }
    cmd.stdin(Stdio::null()).stderr(Stdio::null());
    cmd
}

/// Noms bruts (page de code ANSI) des ROMs .nds de l'archive, dans l'ordre de l'archive.
fn nds_entries(archive: &Path) -> Vec<Vec<u8>> {
    let Ok(out) = tar().arg("-tf").arg(archive).output() else { return Vec::new() };
    out.stdout
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
        .filter(|l| l.len() > 4 && l[l.len() - 4..].eq_ignore_ascii_case(b".nds"))
        .map(<[u8]>::to_vec)
        .collect()
}

/// Nombre de ROMs .nds dans l'archive (0 si illisible).
pub fn nds_count(archive: &Path) -> usize {
    nds_entries(archive).len()
}

/// Nom de fichier (sans dossier) de la première ROM, décodé depuis Windows-1252.
/// Sert à retrouver la sauvegarde, que melonDS nomme d'après la ROM et non l'archive.
pub fn first_nds_name(archive: &Path) -> Option<String> {
    let raw = nds_entries(archive).into_iter().next()?;
    let name = cp1252(&raw);
    Some(name.rsplit(['/', '\\']).next().unwrap_or(&name).to_string())
}

/// Décodage Windows-1252 (page de code ANSI des Windows occidentaux).
fn cp1252(bytes: &[u8]) -> String {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}', '\u{90}', '‘', '’', '“',
        '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
    ];
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => HIGH[(b - 0x80) as usize],
            _ => b as char,
        })
        .collect()
}

/// En-tête (16 premiers octets) de la première ROM de l'archive, sans tout décompresser.
pub fn first_header(archive: &Path) -> Option<[u8; 16]> {
    let mut child = tar().arg("-xOf").arg(archive).args(PATTERNS).stdout(Stdio::piped()).spawn().ok()?;
    let mut header = [0u8; 16];
    let ok = child.stdout.take()?.read_exact(&mut header).is_ok();
    // Inutile d'extraire la suite
    let _ = child.kill();
    let _ = child.wait();
    ok.then_some(header)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Crée une archive de test avec tar.exe, à partir des fichiers donnés.
    fn make(dir: &Path, name: &str, format: &[&str], files: &[(&str, &[u8])]) -> PathBuf {
        let src = dir.join(format!("src_{name}"));
        fs::create_dir_all(&src).unwrap();
        for (f, data) in files {
            fs::write(src.join(f), data).unwrap();
        }
        let out = dir.join(name);
        let status = tar()
            .current_dir(&src)
            .args(format)
            .arg("-cf")
            .arg(&out)
            .args(files.iter().map(|(f, _)| *f))
            .status()
            .unwrap();
        assert!(status.success());
        out
    }

    #[test]
    fn lit_zip_et_7z() {
        let dir = std::env::temp_dir().join(format!("vemuz_archive_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut rom = b"MARIOKART DSAMCE".to_vec();
        rom.extend(std::iter::repeat_n(7u8, 100_000));
        let files: &[(&str, &[u8])] = &[("lisez-moi.txt", b"texte"), ("Mario Kart [b] (E).NDS", &rom)];

        let zip = make(&dir, "t.zip", &["-a"], files);
        let sz = make(&dir, "t.7z", &["--format", "7zip"], files);
        let vide = make(&dir, "vide.zip", &["-a"], &[("lisez-moi.txt", b"texte")]);

        let results = (
            nds_count(&zip),
            first_header(&zip),
            nds_count(&sz),
            first_header(&sz),
            nds_count(&vide),
            first_header(&vide),
        );
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(results.0, 1);
        assert_eq!(&results.1.unwrap(), b"MARIOKART DSAMCE");
        assert_eq!(results.2, 1);
        assert_eq!(&results.3.unwrap(), b"MARIOKART DSAMCE");
        assert_eq!(results.4, 0);
        assert_eq!(cp1252(b"Pok\xe9mon \x9c.nds"), "Pokémon œ.nds");
        assert_eq!(results.5, None);
    }
}
