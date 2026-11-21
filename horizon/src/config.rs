use mod_api::{InstanceDirectory, RegistrationContext};
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub struct Config {
    pub jars: Vec<PathBuf>,
    pub dedicated: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    jars: Vec<String>,
}

impl Config {
    pub fn read(context: &RegistrationContext) -> Result<Self, String> {
        if context.game_version() != "26.3" {
            return Err(format!(
                "MMGA native bridge targets Horizon game 26.3, got {}",
                context.game_version()
            ));
        }
        let directory = directory_path(
            context
                .instance_dir()
                .map_err(|e| format!("instance directory: {e:?}"))?,
        )?;
        // The Horizon package is installed in the client game. Java guests use
        // its separate integrated-server hooks as a dedicated NeoForge server.
        Self::from_directory(&directory, true)
    }

    pub fn from_directory(directory: &Path, dedicated: bool) -> Result<Self, String> {
        let file = directory.join("config/mmga.toml");
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    jars: Vec::new(),
                    dedicated,
                })
            }
            Err(error) => return Err(format!("{}: {error}", file.display())),
        };
        let config: FileConfig =
            toml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        let root = directory.canonicalize().map_err(|e| e.to_string())?;
        let mut jars = Vec::new();
        for entry in config.jars {
            let path = Path::new(&entry);
            if path.is_absolute()
                || path
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
                || path.extension().and_then(|e| e.to_str()) != Some("jar")
            {
                return Err(format!("invalid instance-relative jar path: {entry:?}"));
            }
            let path = root
                .join(path)
                .canonicalize()
                .map_err(|e| format!("{entry}: {e}"))?;
            if !path.starts_with(&root) || !path.is_file() || jars.contains(&path) {
                return Err(format!("duplicate, escaped or non-file jar: {entry:?}"));
            }
            jars.push(path);
        }
        Ok(Self { jars, dedicated })
    }
}

fn directory_path(value: &InstanceDirectory) -> Result<PathBuf, String> {
    match value {
        #[cfg(windows)]
        InstanceDirectory::Windows(units) => {
            use std::os::windows::ffi::OsStringExt;
            Ok(std::ffi::OsString::from_wide(units).into())
        }
        #[cfg(unix)]
        InstanceDirectory::Unix(bytes) => {
            use std::os::unix::ffi::OsStringExt;
            Ok(std::ffi::OsString::from_vec(bytes.clone()).into())
        }
        _ => Err("Horizon instance directory uses another operating system's path encoding".into()),
    }
}
