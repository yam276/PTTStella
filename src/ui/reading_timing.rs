//! Explicit opt-in timings. No screen text, URLs, input bytes or credentials.
use super::article_cache::LoadState;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    time::Instant,
};
type Progress = (u64, usize, usize, Option<LoadState>);
pub(super) struct ReadingTiming {
    file: Option<File>,
    start: Instant,
    events: usize,
    cached: Option<Progress>,
}
impl ReadingTiming {
    pub(super) fn new() -> Result<Self, String> {
        let file = std::env::var_os("PTTSTELLA_READING_TIMING")
            .map(|path| OpenOptions::new().write(true).create_new(true).open(path))
            .transpose()
            .map_err(|e| format!("reading timing: {e}"))?;
        Ok(Self {
            file,
            start: Instant::now(),
            events: 0,
            cached: None,
        })
    }
    pub(super) fn record(
        &mut self,
        event: &str,
        generation: u64,
        revision: u64,
        progress: Option<Progress>,
    ) -> Result<(), String> {
        let Some(file) = self.file.as_mut() else {
            return Ok(());
        };
        if self.events >= 8192 {
            return Ok(());
        }
        if event == "cached" {
            if self.cached == progress {
                return Ok(());
            }
            self.cached = progress;
        }
        let Some((article, first, last, load)) = progress else {
            return Ok(());
        };
        let value = serde_json::json!({"event":event,"ms":self.start.elapsed().as_secs_f64()*1000.0,"generation":generation,"revision":revision,"article":article,"first":first,"last":last,"load":load});
        writeln!(file, "{value}").map_err(|e| e.to_string())?;
        self.events += 1;
        Ok(())
    }
}
