use std::path::PathBuf;

/// Resolve one profile at startup and pass it to every document service and child window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    pub config: Option<PathBuf>,
    pub data: PathBuf,
}

impl AppPaths {
    pub fn from_environment(config: Option<PathBuf>) -> Self {
        Self::resolve(
            config,
            std::env::var_os("CRABPDF_DATA_DIR").map(PathBuf::from),
        )
    }

    pub fn resolve(config: Option<PathBuf>, data_override: Option<PathBuf>) -> Self {
        let data = data_override
            .or_else(|| {
                config
                    .as_ref()
                    .and_then(|path| path.parent().map(|p| p.to_path_buf()))
            })
            .unwrap_or_else(|| std::env::temp_dir().join("crabpdf"));
        Self { config, data }
    }
}

pub fn config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("RUST_READER_CONFIG") {
        return Some(PathBuf::from(path));
    }
    let root = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|p| std::path::Path::new(&p).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| std::path::Path::new(&p).join(".config")))
    };
    root.map(|root| root.join("rust-reader").join("preferences.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_profile_keeps_all_derived_data_together() {
        let first = AppPaths::resolve(Some(PathBuf::from("profiles/a/settings.toml")), None);
        let second = AppPaths::resolve(Some(PathBuf::from("profiles/b/settings.toml")), None);
        assert_eq!(first.data, PathBuf::from("profiles/a"));
        assert_eq!(second.data, PathBuf::from("profiles/b"));
        assert_ne!(first.data, second.data);
        assert_eq!(
            AppPaths::resolve(first.config.clone(), Some(PathBuf::from("separate-data"))).data,
            PathBuf::from("separate-data")
        );
    }
}
