use std::{fs, path::PathBuf};

pub(crate) struct Temp(pub(crate) PathBuf);
impl Temp {
    pub(crate) fn new() -> Self {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "logisim-rs-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
pub(crate) mod plugins;

mod simulation;

pub(crate) fn workspace_shows(workspace: &mut logisim_ui::Workspace, text: &str) -> bool {
    use eframe::egui::{Context, Pos2, RawInput, Rect, Shape, vec2};
    fn contains(shape: &Shape, text: &str) -> bool {
        match shape {
            Shape::Text(shape) => shape.galley.job.text.trim() == text,
            Shape::Vec(shapes) => shapes.iter().any(|shape| contains(shape, text)),
            _ => false,
        }
    }
    let context = Context::default();
    logisim_ui::configure(&context);
    let mut found = false;
    for _ in 0..3 {
        let mut output = context.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 820.0))),
                ..Default::default()
            },
            |ui| workspace.show(ui),
        );
        output.textures_delta.clear();
        found = output
            .shapes
            .iter()
            .any(|shape| contains(&shape.shape, text));
    }
    found
}
