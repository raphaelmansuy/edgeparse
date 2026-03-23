//! Structured pipeline logging with per-stage timing.
//!
//! Provides a [`PipelineTimer`] that records how long each pipeline stage takes
//! and produces a structured summary at the end.

use std::time::{Duration, Instant};

/// A single recorded stage execution.
#[derive(Debug, Clone)]
pub struct StageRecord {
    /// Human-readable stage name.
    pub name: String,
    /// Elapsed wall-clock time.
    pub duration: Duration,
    /// Element count after the stage completed.
    pub element_count: usize,
}

/// Accumulates timing records for each pipeline stage.
#[derive(Debug, Default)]
pub struct PipelineTimer {
    records: Vec<StageRecord>,
    current_start: Option<(String, Instant)>,
    pipeline_start: Option<Instant>,
}

impl PipelineTimer {
    /// Create a new timer and record the pipeline start time.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            current_start: None,
            pipeline_start: Some(Instant::now()),
        }
    }

    /// Begin timing a named stage. If a previous stage was still open, it is
    /// automatically ended with element_count = 0.
    pub fn start_stage(&mut self, name: &str) {
        if let Some((prev_name, prev_start)) = self.current_start.take() {
            self.records.push(StageRecord {
                name: prev_name,
                duration: prev_start.elapsed(),
                element_count: 0,
            });
        }
        self.current_start = Some((name.to_string(), Instant::now()));
    }

    /// End the current stage and record its element count.
    pub fn end_stage(&mut self, element_count: usize) {
        if let Some((name, start)) = self.current_start.take() {
            self.records.push(StageRecord {
                name,
                duration: start.elapsed(),
                element_count,
            });
        }
    }

    /// Total pipeline wall-clock time.
    pub fn total_duration(&self) -> Duration {
        self.pipeline_start.map(|s| s.elapsed()).unwrap_or_default()
    }

    /// Recorded stage entries.
    pub fn records(&self) -> &[StageRecord] {
        &self.records
    }

    /// Format a human-readable timing summary.
    pub fn summary(&self) -> String {
        let mut out = String::from("Pipeline Timing Summary\n");
        out.push_str(&format!(
            "{:<40} {:>10} {:>10}\n",
            "Stage", "Time (ms)", "Elements"
        ));
        out.push_str(&"-".repeat(62));
        out.push('\n');
        for r in &self.records {
            out.push_str(&format!(
                "{:<40} {:>10.2} {:>10}\n",
                r.name,
                r.duration.as_secs_f64() * 1000.0,
                r.element_count,
            ));
        }
        out.push_str(&"-".repeat(62));
        out.push('\n');
        out.push_str(&format!(
            "{:<40} {:>10.2}\n",
            "TOTAL",
            self.total_duration().as_secs_f64() * 1000.0
        ));
        out
    }

    /// Log the summary at info level.
    pub fn log_summary(&self) {
        for line in self.summary().lines() {
            log::info!("{}", line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_stage_recording() {
        let mut timer = PipelineTimer::new();
        timer.start_stage("Stage A");
        thread::sleep(Duration::from_millis(5));
        timer.end_stage(100);

        timer.start_stage("Stage B");
        timer.end_stage(200);

        assert_eq!(timer.records().len(), 2);
        assert_eq!(timer.records()[0].name, "Stage A");
        assert_eq!(timer.records()[0].element_count, 100);
        assert_eq!(timer.records()[1].name, "Stage B");
        assert_eq!(timer.records()[1].element_count, 200);
        // Stage A should have non-zero duration
        assert!(timer.records()[0].duration.as_nanos() > 0);
    }

    #[test]
    fn test_auto_close_previous_stage() {
        let mut timer = PipelineTimer::new();
        timer.start_stage("Stage 1");
        // Start another without ending the first
        timer.start_stage("Stage 2");
        timer.end_stage(50);

        assert_eq!(timer.records().len(), 2);
        assert_eq!(timer.records()[0].name, "Stage 1");
        assert_eq!(timer.records()[0].element_count, 0); // auto-closed
        assert_eq!(timer.records()[1].name, "Stage 2");
    }

    #[test]
    fn test_summary_format() {
        let mut timer = PipelineTimer::new();
        timer.start_stage("Test Stage");
        timer.end_stage(42);

        let summary = timer.summary();
        assert!(summary.contains("Test Stage"));
        assert!(summary.contains("42"));
        assert!(summary.contains("TOTAL"));
    }

    #[test]
    fn test_empty_timer() {
        let timer = PipelineTimer::new();
        assert!(timer.records().is_empty());
        let summary = timer.summary();
        assert!(summary.contains("TOTAL"));
    }
}
