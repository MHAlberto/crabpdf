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
