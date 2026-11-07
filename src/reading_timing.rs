//! Explicit opt-in timings. No screen text, URLs, input bytes or credentials.
use crate::article_cache::LoadState;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    time::Instant,
};
type Progress = (u64, usize, usize, Option<LoadState>);
pub struct ReadingTiming {
    file: Option<File>,
    start: Instant,
    events: usize,
    cached: Option<Progress>,
}
impl ReadingTiming {
    pub fn new() -> Result<Self, String> {
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
    /// An ACK describes the installed packet, which may be older than the
    /// newest cache. Never label an older paint as a completed article.
    pub fn painted(&mut self, paint: Option<&crate::presentation::Paint>) -> Result<(), String> {
        let Some(paint) = paint else {
            return Ok(());
        };
        self.record(
            "painted",
            paint.generation,
            paint.frame.revision,
            paint
                .article
                .as_ref()
                .map(|a| (a.id, a.first_line, a.last_line, a.load)),
        )
    }
    pub fn record(
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_events_merge_but_ack_reports_the_installed_packet_not_newer_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clock.jsonl");
        let mut clock = ReadingTiming {
            file: Some(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&path)
                    .unwrap(),
            ),
            start: Instant::now(),
            events: 0,
            cached: None,
        };
        let newest = Some((9, 1, 837, Some(LoadState::Complete)));
        clock.record("cached", 3, 99, newest).unwrap();
        clock.record("cached", 3, 99, newest).unwrap();
        let frame = crate::screen::Screen::new(80, 24).unwrap().frame();
        let mut installed = crate::presentation::sample_packet(&frame, &Default::default());
        installed.generation = 3;
        installed.frame.revision = 42;
        installed.article = Some(crate::article_cache::Reading {
            id: 9,
            first_line: 1,
            last_line: 23,
            end: false,
            load: Some(LoadState::Loading),
            next_segment: false,
            reply_count: 0,
            percent_cells: None,
            page_cells: None,
            range_cells: None,
            pages: Vec::new(),
        });
        clock.painted(Some(&installed)).unwrap();
        clock.painted(None).unwrap();
        let rows: Vec<serde_json::Value> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0]["last"].as_u64(), rows[0]["load"].as_str()),
            (Some(837), Some("complete"))
        );
        assert_eq!(
            (
                rows[1]["last"].as_u64(),
                rows[1]["revision"].as_u64(),
                rows[1]["load"].as_str()
            ),
            (Some(23), Some(42), Some("loading"))
        );
        assert!(rows.iter().all(|r| r.as_object().unwrap().keys().all(|k| {
            [
                "event",
                "ms",
                "generation",
                "revision",
                "article",
                "first",
                "last",
                "load",
            ]
            .contains(&k.as_str())
        })));
        for _ in 0..9000 {
            clock.record("request", 3, 99, newest).unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(&path).unwrap().lines().count(),
            8192
        );
    }
}
