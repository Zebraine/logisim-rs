//! Lifetime of a single confirmed print submission and its private spool files.
use super::{encode_png, printer_command};
use crate::background::Cancellation;
use eframe::egui::{ColorImage, Context};
use logisim_ui::{
    Language,
    i18n::{tr, with_language},
    tf,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(50);

#[derive(Debug, PartialEq, Eq)]
pub(in crate::files) enum Delivery {
    Sent,
    Cancelled,
}

pub(in crate::files) struct Report {
    pub status: Result<Delivery, String>,
    pub cleanup_warning: Option<String>,
}

impl From<Result<Delivery, String>> for Report {
    fn from(status: Result<Delivery, String>) -> Self {
        Self {
            status,
            cleanup_warning: None,
        }
    }
}

pub(in crate::files) struct Job {
    cancellation: Cancellation,
    worker: Option<JoinHandle<Report>>,
}

impl Job {
    pub(super) fn start(
        image: Arc<ColorImage>,
        ctx: Context,
        language: Language,
    ) -> io::Result<Self> {
        let cancellation = Cancellation::default();
        let flag = cancellation.clone();
        let worker = thread::Builder::new()
            .name("print-canvas".into())
            .spawn(move || {
                let report = with_language(language, || {
                    submit(&std::env::temp_dir(), &image, |path| {
                        run(printer_command(path, cfg!(windows)), path, &flag, TIMEOUT)
                    })
                });
                ctx.request_repaint();
                report
            })?;
        Ok(Self {
            cancellation,
            worker: Some(worker),
        })
    }

    pub(in crate::files) fn is_finished(&self) -> bool {
        self.worker.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub(super) fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub(super) fn finish(mut self) -> Report {
        self.worker
            .take()
            .expect("active print job")
            .join()
            .unwrap_or_else(|_| Err(tr("The print worker stopped.").into()).into())
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            self.cancel();
            let _ = worker.join();
        }
    }
}

struct Spool {
    path: PathBuf,
    cleaned: bool,
}

impl Spool {
    fn create(directory: &Path) -> io::Result<Self> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        loop {
            let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = directory.join(format!("logisim-print-{}-{id}", std::process::id()));
            match builder.create(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        cleaned: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
    fn clean(&mut self) -> io::Result<()> {
        fs::remove_dir_all(&self.path)?;
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for Spool {
    fn drop(&mut self) {
        // Also runs if the encoder or worker unwinds.
        if !self.cleaned {
            let _ = self.clean();
        }
    }
}

pub(super) fn submit(
    directory: &Path,
    image: &ColorImage,
    send: impl FnOnce(&Path) -> Result<Delivery, String>,
) -> Report {
    let mut spool = match Spool::create(directory) {
        Ok(spool) => spool,
        Err(error) => return Err(format!("{}: {error}", directory.display())).into(),
    };
    let path = spool.path.join("canvas.png");
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let status = options
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))
        .and_then(|file| encode_png(file, image))
        .and_then(|()| send(&path));
    let cleanup_warning = spool
        .clean()
        .err()
        .map(|e| format!("{}: {e}", spool.path.display()));
    Report {
        status,
        cleanup_warning,
    }
}

pub(super) fn run(
    mut command: Command,
    png: &Path,
    cancellation: &Cancellation,
    timeout: Duration,
) -> Result<Delivery, String> {
    if cancellation.is_cancelled() {
        return Ok(Delivery::Cancelled);
    }
    let program = command.get_program().to_string_lossy().into_owned();
    let diagnostic = png.with_extension("stderr");
    // A regular file cannot fill a pipe or leave us waiting for EOF from a descendant.
    let stderr =
        File::create_new(&diagnostic).map_err(|e| format!("{}: {e}", diagnostic.display()))?;
    let with_stderr = |message: String| {
        let mut bytes = Vec::new();
        match File::open(&diagnostic).and_then(|file| file.take(8192).read_to_end(&mut bytes)) {
            Ok(0) => message,
            Ok(_) => format!("{message}\n{}", String::from_utf8_lossy(&bytes).trim()),
            Err(error) => format!("{message}\n{}: {error}", diagnostic.display()),
        }
    };
    let os_error = |e: io::Error| format!("{program}: {e}");
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                tf!("The system print command {} was not found.", program)
            } else {
                os_error(error)
            }
        })?;
    let deadline = Instant::now() + timeout;
    loop {
        let stop = match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(Delivery::Sent),
            Ok(Some(status)) => return Err(with_stderr(format!("{program} ({status})"))),
            Err(error) => Err(os_error(error)),
            Ok(None) if cancellation.is_cancelled() => Ok(Delivery::Cancelled),
            Ok(None) if Instant::now() >= deadline => Err(tf!(
                "The print command {} timed out after {} seconds.",
                program,
                timeout.as_secs_f64()
            )),
            Ok(None) => {
                thread::sleep(POLL.min(deadline.saturating_duration_since(Instant::now())));
                continue;
            }
        };
        if let Err(error) = child.kill()
            && child.try_wait().map_err(os_error)?.is_none()
        {
            return Err(os_error(error));
        }
        child.wait().map_err(os_error)?;
        return stop.map_err(with_stderr);
    }
}
