use std::path::{Path, PathBuf};

use domovoi_app::{CardModel, Provenance, RecommendationsModel, Screen, ShellFixture};

const EMPTY_PROJECT: &str = "facet";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json")
}

fn projects(model: &mut RecommendationsModel) -> &mut CardModel {
    model
        .cards
        .iter_mut()
        .find(|card| card.provenance == Provenance::Projects)
        .expect("the fixture has a projects card")
}

fn with_an_empty_project() -> RecommendationsModel {
    let mut model = ShellFixture::read(&fixture())
        .and_then(|fixture| fixture.shell_data(Screen::Recommendations))
        .expect("the fixture builds the recommendations screen data")
        .care
        .recommendations;
    let card = projects(&mut model);
    for row in &mut card.rows {
        row.selected = false;
    }
    card.selected = false;
    card.rows
        .iter_mut()
        .find(|row| row.name == EMPTY_PROJECT)
        .expect("the fixture has the project that frees nothing")
        .amount = 0;
    model
}

fn selection(card: &CardModel) -> Vec<(&str, bool)> {
    card.rows.iter().map(|row| (row.name.as_str(), row.selected)).collect()
}

fn empty_position(card: &CardModel) -> usize {
    card.rows
        .iter()
        .position(|row| row.name == EMPTY_PROJECT)
        .expect("the fixture has the project that frees nothing")
}

#[test]
fn the_group_check_leaves_a_project_that_frees_nothing_unchecked() {
    let mut model = with_an_empty_project();
    let card = projects(&mut model);

    card.toggle_group(true);

    assert_eq!(
        selection(card),
        [
            ("quartz", true),
            ("facet", false),
            ("clavis", true),
            ("engram", false),
            ("boss", false)
        ]
    );
}

#[test]
fn the_row_check_of_a_project_that_frees_nothing_does_nothing() {
    let mut model = with_an_empty_project();
    let card = projects(&mut model);
    let position = empty_position(card);

    card.toggle_row(position);

    assert!(!card.rows[position].selected);
    assert!(!card.some_row_selected());
}

#[test]
fn the_card_check_selects_every_project_that_frees_space_and_checks_the_card() {
    let mut model = with_an_empty_project();
    let card = projects(&mut model);

    card.toggle_rows();

    assert_eq!(
        selection(card),
        [
            ("quartz", true),
            ("facet", false),
            ("clavis", true),
            ("engram", true),
            ("boss", true)
        ]
    );
    assert!(card.selected, "the card is checked, not mixed");
    card.toggle_rows();
    assert!(!card.some_row_selected() && !card.selected);
}

#[test]
fn a_rebuild_keeps_the_selection_made_by_the_card_check() {
    let mut previous = with_an_empty_project();
    projects(&mut previous).toggle_rows();
    let mut rebuilt = with_an_empty_project();

    rebuilt.keep_selection(&previous);

    let before = projects(&mut previous).clone();
    let after = projects(&mut rebuilt);
    assert_eq!(selection(after), selection(&before));
    assert_eq!(
        after.selected, before.selected,
        "the card does not turn mixed by itself"
    );
}
