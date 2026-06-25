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
