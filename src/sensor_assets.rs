use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::config::LocalConfig;

const ASSETS: &[(&str, &[u8])] = &[
    (
        "assets/icon-16.png",
        include_bytes!("../sensor/dist/extension/assets/icon-16.png"),
    ),
    (
        "assets/icon-16@2x.png",
        include_bytes!("../sensor/dist/extension/assets/icon-16@2x.png"),
    ),
    (
        "assets/icon-32.png",
        include_bytes!("../sensor/dist/extension/assets/icon-32.png"),
    ),
    (
        "assets/icon-32@2x.png",
        include_bytes!("../sensor/dist/extension/assets/icon-32@2x.png"),
    ),
    (
        "assets/icon-48.png",
        include_bytes!("../sensor/dist/extension/assets/icon-48.png"),
    ),
    (
        "assets/icon-48@2x.png",
        include_bytes!("../sensor/dist/extension/assets/icon-48@2x.png"),
    ),
    (
        "assets/icon-128.png",
        include_bytes!("../sensor/dist/extension/assets/icon-128.png"),
    ),
    (
        "assets/icon-128@2x.png",
        include_bytes!("../sensor/dist/extension/assets/icon-128@2x.png"),
    ),
    (
        "content.js",
        include_bytes!("../sensor/dist/extension/content.js"),
    ),
    (
        "background.js",
        include_bytes!("../sensor/dist/extension/background.js"),
    ),
    (
        "options.js",
        include_bytes!("../sensor/dist/extension/options.js"),
    ),
    (
        "manifest.json",
        include_bytes!("../sensor/dist/extension/manifest.json"),
    ),
    (
        "options.html",
        include_bytes!("../sensor/dist/extension/options.html"),
    ),
    (
        "build-info.json",
        include_bytes!("../sensor/dist/extension/build-info.json"),
    ),
];

pub fn install(home: &Path, config: &LocalConfig) -> Result<PathBuf> {
    let destination = home.join("sensor-extension");
    fs::create_dir_all(&destination)?;
    for (name, bytes) in ASSETS {
        let path = destination.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = destination.join(format!("{name}.tmp"));
        if let Some(parent) = temporary.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&temporary, bytes)?;
        fs::rename(temporary, path)?;
    }
    let local_config = serde_json::json!({
        "capturePort": config.capture_port,
        "captureToken": config.capture_token,
    });
    let path = destination.join("starlee-config.json");
    let temporary = destination.join("starlee-config.json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&local_config)?)?;
    restrict_permissions(&temporary)?;
    fs::rename(temporary, path)?;
    restrict_permissions(&destination.join("starlee-config.json"))?;
    Ok(destination)
}

/// Names of the installed code/asset files that, if they differ from the bytes
/// embedded in this binary, mean the on-disk extension is running stale code
/// relative to the build that was compiled in. Two files are intentionally
/// excluded:
/// - `starlee-config.json` is generated per machine by [`install`] and is never
///   embedded.
/// - `build-info.json` carries a `built_at` timestamp (and git fields) that change
///   on every rebuild even when the extension code is identical, so comparing it
///   would report drift on a meaningless timestamp difference.
pub fn installed_drift(home: &Path) -> Vec<String> {
    let destination = home.join("sensor-extension");
    let mut drift = Vec::new();
    for (name, bytes) in ASSETS {
        if *name == "build-info.json" {
            continue;
        }
        let path = destination.join(name);
        match fs::read(&path) {
            Ok(actual) if actual.as_slice() == *bytes => {}
            Ok(_) => drift.push((*name).to_string()),
            Err(_) => drift.push(format!("{name} (missing)")),
        }
    }
    drift
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_embedded(home: &Path) {
        let destination = home.join("sensor-extension");
        for (name, bytes) in ASSETS {
            let path = destination.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, bytes).unwrap();
        }
    }

    #[test]
    fn installed_drift_is_empty_when_files_match_embedded_build() {
        let temp = tempfile::tempdir().unwrap();
        write_embedded(temp.path());
        assert!(installed_drift(temp.path()).is_empty());
    }

    #[test]
    fn installed_drift_ignores_build_info_timestamp_changes() {
        let temp = tempfile::tempdir().unwrap();
        write_embedded(temp.path());
        // build-info.json legitimately differs every rebuild; it must not count.
        fs::write(
            temp.path().join("sensor-extension/build-info.json"),
            br#"{"git_commit":"deadbeef","built_at":"2000-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        assert!(installed_drift(temp.path()).is_empty());
    }

    #[test]
    fn installed_drift_reports_changed_and_missing_code_files() {
        let temp = tempfile::tempdir().unwrap();
        write_embedded(temp.path());
        fs::write(
            temp.path().join("sensor-extension/content.js"),
            b"// stale build",
        )
        .unwrap();
        fs::remove_file(temp.path().join("sensor-extension/background.js")).unwrap();
        let drift = installed_drift(temp.path());
        assert!(drift.iter().any(|entry| entry == "content.js"));
        assert!(drift.iter().any(|entry| entry == "background.js (missing)"));
    }
}
