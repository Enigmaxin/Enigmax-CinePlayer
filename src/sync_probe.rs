//! Opt-in diagnostics. Normal playback does not instantiate or poll this logger.
use crate::{
    mpv::Mpv,
    player::{self, State},
};
use std::{
    fs::File,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

pub struct SyncProbe {
    file: File,
    started: Instant,
    last: Instant,
    stage: usize,
    exercise: bool,
}
impl SyncProbe {
    pub fn new(path: &Path) -> Result<Self, String> {
        let mut file = File::create(path.with_extension("sync.csv")).map_err(|e| e.to_string())?;
        writeln!(file, "elapsed,stage,time-pos,audio-pts,avsync,pause,seeking,paused-for-cache,frame-drop-count,decoder-frame-drop-count,sub-start,sub-end,sub-text,audio-delay,sub-delay,output,mvc,title,current-ao").map_err(|e| e.to_string())?;
        let now = Instant::now();
        Ok(Self {
            file,
            started: now,
            last: now - Duration::from_secs(1),
            stage: 0,
            exercise: std::env::var_os("ENIGMAX_PROBE_EXERCISE").is_some(),
        })
    }
    pub fn tick(&mut self, m: &Mpv, s: &mut State) -> Result<(), String> {
        if self.last.elapsed() < Duration::from_millis(100) {
            return Ok(());
        }
        self.last = Instant::now();
        let elapsed = self.started.elapsed().as_secs_f64();
        if self.exercise && !s.pending_load {
            let deadlines = [3., 4., 7., 11., 15., 20., 25.];
            if self.stage < deadlines.len() && elapsed >= deadlines[self.stage] {
                match self.stage {
                    0 => m.set("pause", "yes")?,
                    1 => m.set("pause", "no")?,
                    2 => m.command(&["seek", "20", "absolute+exact"])?,
                    3 => m.command(&["seek", "5", "absolute+exact"])?,
                    4 | 6 if s.input_layout(m).packed() => player::action(m, s, 1200)?,
                    5 if s.input_layout(m).packed() => player::action(m, s, 1210)?,
                    _ => {}
                }
                self.stage += 1;
            }
        }
        let mut cells = vec![format!("{elapsed:.6}"), self.stage.to_string()];
        for prop in [
            "time-pos",
            "audio-pts",
            "avsync",
            "pause",
            "seeking",
            "paused-for-cache",
            "frame-drop-count",
            "decoder-frame-drop-count",
            "sub-start",
            "sub-end",
            "sub-text",
            "audio-delay",
            "sub-delay",
        ] {
            cells.push(m.get(prop).unwrap_or_default());
        }
        cells.push(s.prefs.output.label().into());
        cells.push(s.raw.is_some().to_string());
        cells.push(m.get("title").unwrap_or_default());
        cells.push(m.get("current-ao").unwrap_or_default());
        let row = cells
            .iter()
            .map(|v| format!("\"{}\"", v.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(self.file, "{row}").map_err(|e| e.to_string())
    }
}
