use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

pub struct CoreLogger;

impl CoreLogger {
    pub fn log(layer: &str, event: &str) {
        let line = format!("[{}] {}\n", layer, event);

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(Self::log_path())
        {
            let _ = file.write_all(line.as_bytes());
        }
    }

    pub fn log_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("core_mock_pipeline.log")
    }
}
