//! Desktop integration on Linux.
//!
//! A Linux desktop does not take a window's icon from the window. It looks up
//! the window's application id among the installed launcher entries and uses
//! that entry's name and icon; without one, the dock shows a generic icon.
//! So `--install-desktop` writes an entry and the icon, drawn from the logo,
//! into the user's own data directory. Nothing needs root.

use std::path::{Path, PathBuf};

use crate::ui::logo::{self, Logo};

/// The application id the window announces, and the name of the launcher
/// entry and icon that go with it.
pub const APP_ID: &str = "buttercandy";

/// The sizes the icon is installed at, in pixels.
const ICON_SIZES: [u32; 6] = [32, 48, 64, 128, 256, 512];

fn data_dir() -> Result<PathBuf, String> {
    dirs::data_dir().ok_or_else(|| "could not find your data directory".to_string())
}

fn entry_path(data: &Path) -> PathBuf {
    data.join("applications").join(format!("{APP_ID}.desktop"))
}

fn icon_path(data: &Path, size: u32) -> PathBuf {
    data.join("icons")
        .join("hicolor")
        .join(format!("{size}x{size}"))
        .join("apps")
        .join(format!("{APP_ID}.png"))
}

/// `path` as one argument of an `Exec=` line: quoted, with the characters
/// the desktop entry specification reserves inside quotes escaped.
fn exec_arg(path: &Path) -> String {
    let mut out = String::from("\"");
    for c in path.to_string_lossy().chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    // A literal percent sign has to be doubled; %F below is the file list.
    out.replace('%', "%%")
}

/// The launcher entry for the program at `exe`.
fn entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=buttercandy\n\
         GenericName=Music Visualizer\n\
         Comment=Ear candy for your eyes: a music visualizer\n\
         Exec={} %F\n\
         Icon={APP_ID}\n\
         Terminal=false\n\
         Categories=AudioVideo;Audio;Player;\n\
         MimeType=audio/mpeg;audio/flac;audio/x-wav;audio/ogg;audio/mp4;audio/aac;audio/x-aiff;\n\
         StartupWMClass={APP_ID}\n\
         StartupNotify=true\n",
        exec_arg(exe)
    )
}

/// Add buttercandy to the applications menu, with its icon.
pub fn install() -> Result<(), String> {
    let data = data_dir()?;
    let exe = std::env::current_exe().map_err(|e| format!("could not find this program: {e}"))?;
    let write = |path: &Path, bytes: &[u8]| {
        let dir = path.parent().expect("an installed file has a directory");
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    };

    for size in ICON_SIZES {
        let rgba = logo::rasterize(Logo::Mark, size, size, -0.5);
        let mut png = Vec::new();
        image::write_buffer_with_format(
            &mut std::io::Cursor::new(&mut png),
            &rgba,
            size,
            size,
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|e| e.to_string())?;
        write(&icon_path(&data, size), &png)?;
    }
    let entry_at = entry_path(&data);
    write(&entry_at, entry(&exe).as_bytes())?;

    // Nudge the desktop to notice. Both are optional: without them it picks
    // the files up by itself, only later.
    let _ = std::process::Command::new("update-desktop-database")
        .arg(data.join("applications"))
        .status();
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .args(["--force", "--ignore-theme-index", "--quiet"])
        .arg(data.join("icons").join("hicolor"))
        .status();

    println!("installed {}", entry_at.display());
    println!("  it starts {}", exe.display());
    println!("  run this again if you move the program; --uninstall-desktop removes it");
    Ok(())
}

/// Remove what [`install`] wrote.
pub fn uninstall() -> Result<(), String> {
    let data = data_dir()?;
    let mut paths = vec![entry_path(&data)];
    paths.extend(ICON_SIZES.map(|size| icon_path(&data, size)));
    for path in paths {
        match std::fs::remove_file(&path) {
            Ok(()) => println!("removed {}", path.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_names_the_program_and_its_icon() {
        let text = entry(Path::new("/opt/butter candy/bin/butter$candy"));
        // A path with a space and a dollar sign is quoted and escaped.
        assert!(
            text.contains("Exec=\"/opt/butter candy/bin/butter\\$candy\" %F\n"),
            "{text}"
        );
        assert!(text.contains("Icon=buttercandy\n"));
        // The window's application id is how the desktop matches the two.
        assert!(text.contains(&format!("StartupWMClass={APP_ID}\n")));
        assert!(text.starts_with("[Desktop Entry]\n") && text.ends_with('\n'));
    }

    #[test]
    fn files_go_where_the_desktop_looks() {
        let data = Path::new("/home/someone/.local/share");
        assert_eq!(
            entry_path(data),
            Path::new("/home/someone/.local/share/applications/buttercandy.desktop")
        );
        assert_eq!(
            icon_path(data, 64),
            Path::new("/home/someone/.local/share/icons/hicolor/64x64/apps/buttercandy.png")
        );
    }
}
