use domovoi_app::{PurgeFinish, PurgeModel, PurgeProgress, PurgeReport, strings};

const NO_BREAK_SPACE: char = '\u{A0}';
const ESTIMATE: u64 = 8_400_000_000;
const FREED: u64 = 1_200_000_000;
const AVAILABLE: u64 = 10_000_000_000;
const REFUSAL: &str = "CoreServices refused the recovery with code 5";

fn releasing(estimate: u64, freed: u64) -> PurgeModel {
    PurgeModel::Releasing(PurgeProgress { estimate, freed })
}

fn released(finish: PurgeFinish) -> PurgeModel {
    PurgeModel::Released(Ok(PurgeReport {
        estimate: ESTIMATE,
        freed: FREED,
        available_before: AVAILABLE,
        available_after: AVAILABLE + FREED,
        finish,
    }))
}

fn spoken(text: &str) -> String {
    text.replace(NO_BREAK_SPACE, " ")
}

fn spoken_lines(model: &PurgeModel) -> Vec<String> {
    model.released_lines().iter().map(|line| spoken(line)).collect()
}

#[test]
fn the_sheet_names_the_freed_and_the_estimate_while_releasing() {
    let model = releasing(ESTIMATE, FREED);
    assert_eq!(
        model.releasing_line().map(|line| spoken(&line)).as_deref(),
        Some("Освобождаю: 1,2 ГБ из 8,4 ГБ")
    );
    assert_eq!(model.releasing_share(), Some(FREED as f64 / ESTIMATE as f64));
    assert_eq!(model.estimate(), ESTIMATE);
}

#[test]
fn the_bar_is_empty_when_nothing_was_estimated() {
    assert_eq!(releasing(0, 0).releasing_share(), Some(0.0));
    assert_eq!(releasing(0, 500).releasing_share(), Some(0.0));
}

#[test]
fn the_bar_never_overflows_when_more_is_freed_than_estimated() {
    assert_eq!(releasing(1_000, 1_500).releasing_share(), Some(1.0));
    assert_eq!(releasing(1_000, 1_000).releasing_share(), Some(1.0));
}

#[test]
fn a_finished_release_names_the_freed_and_why_it_stopped() {
    let freed = "Освобождено: 1,2 ГБ";
    assert_eq!(spoken_lines(&released(PurgeFinish::Completed)), [freed]);
    assert_eq!(
        spoken_lines(&released(PurgeFinish::Cancelled)),
        [freed, strings::PURGE_STOPPED]
    );
    assert_eq!(
        spoken_lines(&released(PurgeFinish::TimedOut)),
        [freed, strings::PURGE_TIMED_OUT]
    );
    assert_eq!(
        spoken_lines(&released(PurgeFinish::Stalled)),
        [freed, strings::PURGE_STALLED]
    );
    assert_eq!(released(PurgeFinish::Completed).estimate(), ESTIMATE);
}

#[test]
fn a_refusal_of_the_mechanism_is_a_result_with_its_reason() {
    let model = PurgeModel::Released(Err(REFUSAL.to_owned()));
    assert_eq!(model.released_lines(), [strings::purge_failed_line(REFUSAL)]);
    assert_eq!(model.estimate(), 0);
    assert_eq!(model.releasing_line(), None);
}

#[test]
fn the_confirmation_names_what_goes_and_the_other_states_do_not() {
    let with_backups = PurgeModel::Confirming {
        estimate: ESTIMATE,
        time_machine: true,
    };
    assert_eq!(with_backups.goes_line(), Some(strings::PURGE_GOES_TIME_MACHINE));
    assert_eq!(with_backups.estimate(), ESTIMATE);
    let plain = PurgeModel::Confirming {
        estimate: ESTIMATE,
        time_machine: false,
    };
    assert_eq!(plain.goes_line(), Some(strings::PURGE_GOES_PLAIN));
    assert_eq!(plain.releasing_line(), None);
    assert_eq!(plain.releasing_share(), None);
    assert!(plain.released_lines().is_empty());
    assert_eq!(releasing(ESTIMATE, FREED).goes_line(), None);
    assert_eq!(released(PurgeFinish::Completed).goes_line(), None);
}
