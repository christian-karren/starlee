use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;

const ASSETS: &[(&str, &[u8])] = &[
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
];

pub fn install(home: &Path) -> Result<PathBuf> {
    let destination = home.join("sensor-extension");
    fs::create_dir_all(&destination)?;
    for (name, bytes) in ASSETS {
        let path = destination.join(name);
        let temporary = destination.join(format!("{name}.tmp"));
        fs::write(&temporary, bytes)?;
        fs::rename(temporary, path)?;
    }
    Ok(destination)
}
