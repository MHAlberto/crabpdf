//! The desktop composition root: resolve a profile once and wire runtime services.
use crate::{application::preferences::Preferences, document::ReaderError};
use crabpdf_runtime::{paths::AppPaths, storage::profile::ProfileSession, worker::PdfWorker};
use std::{path::PathBuf, sync::Arc};

pub struct DesktopServices {
    pub paths: AppPaths,
    pub preferences: Preferences,
    pub profile: ProfileSession,
    pub config_warning: Option<String>,
    pub worker: Option<PdfWorker>,
    pub worker_error: Option<ReaderError>,
}
impl DesktopServices {
    pub fn start(config: Option<PathBuf>, repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let paths = AppPaths::from_environment(config.clone());
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
        let (profile, preferences, config_warning) =
            ProfileSession::open(config, Arc::clone(&repaint));
        let (worker, worker_error) =
            match PdfWorker::spawn_with_paths(move || repaint(), paths.clone()) {
                Ok(worker) => (Some(worker), None),
                Err(error) => (None, Some(error)),
            };
        Self {
            paths,
            preferences,
            profile,
            config_warning,
            worker,
            worker_error,
        }
    }
}
