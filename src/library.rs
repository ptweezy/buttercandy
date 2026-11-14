//! Finding audio files and naming them.

use std::path::{Path, PathBuf};

/// Extensions we can decode (see the symphonia features in Cargo.toml).
pub const EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "oga", "m4a", "m4b", "mp4", "aac", "aiff", "aif", "caf", "mka", "mp2", "mp1",
];

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

fn walk(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    for path in entries {
        if path.is_dir() {
            if depth > 0 {
                walk(&path, depth - 1, out);
            }
        } else if is_audio(&path) {
            out.push(path);
        }
    }
}

/// Turn dropped or picked paths into a flat list of audio files; folders are
/// searched a few levels deep, in name order.
pub fn expand(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            walk(&path, 4, &mut out);
        } else if is_audio(&path) {
            out.push(path);
        }
    }
    out
}

/// A (title, artist) guess from a file name, used until tags are read.
/// Understands `Artist - Title` and leading track numbers.
pub fn display_name(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut name = stem.replace('_', " ");
    name = name.trim().to_string();

    // Strip "07 ", "07. ", "07 - ", "1-07 " and friends.
    let digits = name.chars().take_while(|c| c.is_ascii_digit() || *c == '-').count();
    if (1..=5).contains(&digits) && name[..digits].chars().any(|c| c.is_ascii_digit()) {
        let rest = name[digits..].trim_start_matches(['.', ' ', '-']);
        let separated = name[digits..].starts_with(['.', ' ', '-']) || name[..digits].ends_with('-');
        if separated && !rest.is_empty() && rest.chars().any(|c| c.is_alphabetic()) {
            name = rest.to_string();
        }
    }

    match name.split_once(" - ") {
        Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => {
            (title.trim().to_string(), artist.trim().to_string())
        }
        _ if name.is_empty() => ("Unknown track".to_string(), String::new()),
        _ => (name, String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> (String, String) {
        display_name(Path::new(s))
    }

    #[test]
    fn names_from_files() {
        assert_eq!(name("Daft Punk - Voyager.mp3"), ("Voyager".into(), "Daft Punk".into()));
        assert_eq!(name("07 - Voyager.flac"), ("Voyager".into(), "".into()));
        assert_eq!(
            name("07. Daft Punk - Voyager.flac"),
            ("Voyager".into(), "Daft Punk".into())
        );
        assert_eq!(name("1-07 Voyager.m4a"), ("Voyager".into(), "".into()));
        assert_eq!(name("some_song_name.wav"), ("some song name".into(), "".into()));
        // A name that is only a number stays as it is.
        assert_eq!(name("1979.mp3"), ("1979".into(), "".into()));
    }

    #[test]
    fn audio_extensions() {
        assert!(is_audio(Path::new("a/b/Song.MP3")));
        assert!(is_audio(Path::new("x.flac")));
        assert!(!is_audio(Path::new("cover.jpg")));
        assert!(!is_audio(Path::new("noext")));
    }
}
