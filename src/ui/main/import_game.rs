use std::path::{Path, PathBuf};

use crate::*;
use super::{App, AppMsg};

fn validate_path(path: &Path) -> Result<(), &'static str> {
    // resolve symlinks so /var/run -> /run etc. are caught
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    for p in [path, canonical.as_path()] {
        let s = p.to_string_lossy();

        // these are fuse-mounted by xdg-document-portal and vanish when the portal
        // closes
        if s.starts_with("/run/user/") && s.contains("/doc/") {
            return Err("import-game-path-runtime");
        }

        // other dangerous prefixes (/run/media is removable drives, allow it)
        for prefix in ["/run/", "/var/run/", "/proc/", "/sys/", "/dev/"] {
            if s.starts_with(prefix) && !s.starts_with("/run/media/") {
                return Err("import-game-path-runtime");
            }
        }
    }

    // reject home directory itself, almost always an accidental pick
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        if canonical == home || path == home {
            return Err("import-game-path-home");
        }
    }

    Ok(())
}

pub fn import_game(sender: relm4::ComponentSender<App>, path: PathBuf) {
    if let Err(key) = validate_path(&path) {
        sender.input(AppMsg::Toast {
            title: tr!(key),
            description: None
        });
        return;
    }

    let config = match Config::get() {
        Ok(c) => c,
        Err(err) => {
            sender.input(AppMsg::Toast {
                title: tr!("import-game-error"),
                description: Some(err.to_string())
            });
            return;
        }
    };

    let edition = config.launcher.edition;
    let game = Game::new(&path, edition);

    if !game.is_installed() {
        sender.input(AppMsg::Toast {
            title: tr!("import-game-invalid-path"),
            description: None
        });
        return;
    }

    // write .version if missing so the launcher can detect the version
    let version_path = path.join(".version");
    if !version_path.exists() {
        match game.get_version() {
            Ok(version) => {
                if let Err(err) = std::fs::write(&version_path, &version.version) {
                    tracing::warn!("Failed to write .version during import: {err}");
                }
            }
            Err(err) => tracing::warn!("Failed to detect version during import: {err}")
        }
    }

    let mut config = config;
    match edition {
        GameEdition::Global => config.game.path.global = path,
        GameEdition::China => config.game.path.china = path
    }
    Config::update(config);

    sender.input(AppMsg::UpdateLauncherState {
        perform_on_download_needed: false,
        show_status_page: true
    });
}
