use super::{CACHE_LOCK_POLL, CACHE_LOCK_WAIT, io_error, limit_error, prepare_in};
use crate::document::{ErrorKind, ReaderError};
use std::{
    cell::RefCell,
    fs::File,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
    },
    time::{Duration, Instant},
};
#[derive(Clone)]
struct ImportLimits {
    cancelled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    deadline: Instant,
}

thread_local! {
    static IMPORT_LIMITS: RefCell<Option<ImportLimits>> = const { RefCell::new(None) };
}

struct RestoreLimits(Option<ImportLimits>);
impl Drop for RestoreLimits {
    fn drop(&mut self) {
        IMPORT_LIMITS.with(|current| *current.borrow_mut() = self.0.take());
    }
}

pub(super) fn check_work() -> Result<(), ReaderError> {
    IMPORT_LIMITS.with(|current| {
        let current = current.borrow();
        if let Some(limits) = current.as_ref() {
            if limits.cancelled.load(AtomicOrdering::Relaxed)
                || limits.shutdown.load(AtomicOrdering::Relaxed)
            {
                return Err(ReaderError::new(
                    ErrorKind::Worker,
                    "Importación cancelada.",
                ));
            }
            if Instant::now() >= limits.deadline {
                return Err(limit_error());
            }
        }
        Ok(())
    })
}

pub(super) fn lock_cache(file: &File) -> Result<(), ReaderError> {
    lock_cache_for(file, CACHE_LOCK_WAIT)
}

pub(super) fn lock_cache_for(file: &File, wait: Duration) -> Result<(), ReaderError> {
    let deadline = Instant::now() + wait;
    loop {
        check_work()?;
        match file.try_lock() {
            Ok(()) => {
                check_work()?;
                return Ok(());
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(ReaderError::new(
                        ErrorKind::File,
                        "La caché de este documento sigue ocupada.",
                    )
                    .with_details("Se agotó el tiempo esperando otra importación."));
                }
                std::thread::sleep(remaining.min(CACHE_LOCK_POLL));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(io_error(error)),
        }
    }
}

pub fn prepare_cancellable_in(
    source: &Path,
    data_directory: &Path,
    cancelled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> Result<PathBuf, ReaderError> {
    with_limits(cancelled, shutdown, || prepare_in(source, data_directory))
}

pub(super) fn with_limits<T>(
    cancelled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    operation: impl FnOnce() -> Result<T, ReaderError>,
) -> Result<T, ReaderError> {
    let previous = IMPORT_LIMITS.with(|current| {
        current.borrow_mut().replace(ImportLimits {
            cancelled,
            shutdown,
            deadline: Instant::now() + Duration::from_secs(180),
        })
    });
    let _restore = RestoreLimits(previous);
    operation()
}
