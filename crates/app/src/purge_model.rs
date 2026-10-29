use domovoi_theme::format_size;

use crate::purge_finish::PurgeFinish;
use crate::purge_progress::PurgeProgress;
use crate::purge_report::PurgeReport;
use crate::strings::{
    OF, PURGE_GOES_PLAIN, PURGE_GOES_TIME_MACHINE, PURGE_STALLED, PURGE_STOPPED, PURGE_TIMED_OUT, PURGED, PURGING,
    purge_failed_line,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PurgeModel {
    Confirming { estimate: u64, time_machine: bool },
    Releasing(PurgeProgress),
    Released(Result<PurgeReport, String>),
}

impl PurgeModel {
    pub fn estimate(&self) -> u64 {
        match self {
            Self::Confirming { estimate, .. } => *estimate,
            Self::Releasing(progress) => progress.estimate,
            Self::Released(Ok(report)) => report.estimate,
            Self::Released(Err(_)) => 0,
        }
    }

    pub fn goes_line(&self) -> Option<&'static str> {
        match self {
            Self::Confirming { time_machine: true, .. } => Some(PURGE_GOES_TIME_MACHINE),
            Self::Confirming {
                time_machine: false, ..
            } => Some(PURGE_GOES_PLAIN),
            Self::Releasing(_) | Self::Released(_) => None,
        }
    }

    pub fn releasing_line(&self) -> Option<String> {
        let Self::Releasing(progress) = self else {
            return None;
        };
        Some(format!(
            "{PURGING} {} {OF} {}",
            format_size(progress.freed),
            format_size(progress.estimate)
        ))
    }

    pub fn releasing_share(&self) -> Option<f64> {
        let Self::Releasing(progress) = self else {
            return None;
        };
        if progress.estimate == 0 {
            return Some(0.0);
        }
        Some((progress.freed as f64 / progress.estimate as f64).min(1.0))
    }

    pub fn released_lines(&self) -> Vec<String> {
        match self {
            Self::Released(Ok(report)) => {
                let mut lines = vec![format!("{PURGED} {}", format_size(report.freed))];
                lines.extend(finish_reason(report.finish).map(str::to_owned));
                lines
            }
            Self::Released(Err(error)) => vec![purge_failed_line(error)],
            Self::Confirming { .. } | Self::Releasing(_) => Vec::new(),
        }
    }
}

fn finish_reason(finish: PurgeFinish) -> Option<&'static str> {
    match finish {
        PurgeFinish::Completed => None,
        PurgeFinish::Cancelled => Some(PURGE_STOPPED),
        PurgeFinish::TimedOut => Some(PURGE_TIMED_OUT),
        PurgeFinish::Stalled => Some(PURGE_STALLED),
    }
}
