//! One explicit invocation in a background thread, with bounded file exchange.
use super::{InstalledPackage, Repository};
use crabpdf_extension_api::{
    API_VERSION, ExtensionCommand, ExtensionOutput, InputKind, MAX_MESSAGE_BYTES, Request,
    Response, SelectionImage,
};
use std::{
    fs::{self, File},
    io,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

pub struct Invocation {
    pub command: ExtensionCommand,
    pub page: usize,
    pub region: crabpdf_extension_api::Region,
    pub text: String,
    pub image: Option<(Vec<u8>, u32, u32)>,
}
pub struct RunningExtension {
    cancelled: Arc<AtomicBool>,
    events: mpsc::Receiver<Result<ExtensionOutput, String>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl RunningExtension {
    pub fn spawn(
        repository: Repository,
        package: InstalledPackage,
        input: Invocation,
        repaint: impl FnOnce() + Send + 'static,
    ) -> io::Result<Self> {
        let timeout = Duration::from_secs(if input.command.input == InputKind::None {
            600
        } else {
            120
        });
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancelled);
        let (sender, events) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("crabpdf-extension".into())
            .spawn(move || {
                let result =
                    invoke(&repository, &package, input, &flag, timeout).map_err(|e| e.to_string());
                let _ = sender.send(result);
                repaint();
            })?;
        Ok(Self {
            cancelled,
            events,
            thread: Some(thread),
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn poll(&self) -> Option<Result<ExtensionOutput, String>> {
        match self.events.try_recv() {
            Ok(value) => Some(value),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("La extensión dejó de responder.".into()))
            }
        }
    }
}
impl Drop for RunningExtension {
    fn drop(&mut self) {
        self.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
struct Work(PathBuf);
impl Work {
    fn create() -> io::Result<Self> {
        let path = fs::canonicalize(std::env::temp_dir())?.join(format!(
            "crabpdf-extension-{:016x}",
            getrandom::u64().map_err(io::Error::other)?
        ));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        for name in ["request.json", "response.json", "selection.png"] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}
/// Invocation scratch space lives beside extension data so verified downloads can be renamed.
struct Scratch(PathBuf);
impl Scratch {
    fn create(data: &std::path::Path) -> io::Result<Self> {
        let path = data.join(".crabpdf-scratch");
        Self::clean(&path)?;
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn clean(path: &std::path::Path) -> io::Result<()> {
        match fs::remove_dir_all(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = Self::clean(&self.0);
    }
}
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn invoke(
    repository: &Repository,
    package: &InstalledPackage,
    input: Invocation,
    cancelled: &AtomicBool,
    timeout: Duration,
) -> io::Result<ExtensionOutput> {
    let _package_lock = repository.acquire(package)?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(io::Error::other("Operación cancelada."));
    }
    let command = package
        .manifest
        .commands
        .iter()
        .find(|command| command.id == input.command.id)
        .ok_or_else(|| io::Error::other("Comando de extensión desconocido."))?;
    if !input.region.valid()
        || input.page == 0
        || command.input != input.command.input
        || command.output != input.command.output
    {
        return Err(io::Error::other("La selección o el comando cambió."));
    }
    let work = Work::create()?;
    let request_id = getrandom::u64().map_err(io::Error::other)?;
    let image = if command.input == InputKind::Region {
        let (png, width, height) = input
            .image
            .ok_or_else(|| io::Error::other("Falta la imagen seleccionada."))?;
        if png.len() > 20 * 1024 * 1024
            || width == 0
            || height == 0
            || width > 2048
            || height > 2048
        {
            return Err(io::Error::other("Imagen fuera de los límites de la API."));
        }
        let path = work.0.join("selection.png");
        use std::io::Write;
        File::create_new(&path)?.write_all(&png)?;
        Some(SelectionImage {
            png_path: path,
            width,
            height,
        })
    } else {
        None
    };
    let request = Request {
        api_version: API_VERSION,
        request_id,
        command: command.id.clone(),
        page: input.page,
        region: input.region,
        text: (command.input != InputKind::None).then_some(input.text),
        image,
    };
    let request_path = work.0.join("request.json");
    let response_path = work.0.join("response.json");
    crabpdf_extension_api::write_message(&request_path, &request)?;
    let executable = fs::canonicalize(package.directory.join(&package.manifest.executable))?;
    let data = repository.model_data(&package.manifest.id)?;
    let scratch = Scratch::create(&data)?;
    let mut process = Command::new(executable);
    process
        .current_dir(&package.directory)
        .env("CRABPDF_EXTENSION_DATA", &data)
        .env("CRABPDF_EXTENSION_SCRATCH", &scratch.0)
        .arg("--crabpdf-request")
        .arg(&request_path)
        .arg("--crabpdf-response")
        .arg(&response_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        process.creation_flags(0x08000000);
    }
    let mut child = ChildGuard(process.spawn()?);
    let started = Instant::now();
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(io::Error::other("Operación cancelada."));
        }
        if started.elapsed() >= timeout {
            return Err(io::Error::other(format!(
                "La extensión superó el límite de {} segundos.",
                timeout.as_secs()
            )));
        }
        if response_path.exists() {
            let meta = fs::symlink_metadata(&response_path)?;
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_MESSAGE_BYTES {
                return Err(io::Error::other(
                    "La respuesta de la extensión excede los límites.",
                ));
            }
        }
        if let Some(status) = child.0.try_wait()? {
            if !status.success() {
                return Err(io::Error::other(format!(
                    "La extensión terminó con {status}."
                )));
            }
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let response: Response = crabpdf_extension_api::read_message(&response_path)?;
    if response.api_version != API_VERSION || response.request_id != request_id {
        return Err(io::Error::other("Respuesta de otra solicitud o API."));
    }
    let output = response.result.map_err(io::Error::other)?;
    if output.kind != command.output || output.text.is_empty() {
        return Err(io::Error::other(
            "La extensión devolvió un resultado inválido.",
        ));
    }
    Ok(output)
}
