use std::path::{Path, PathBuf};

use domovoi_app::{CardModel, Provenance, RecommendationsModel, Screen, ShellFixture};

const UNCHECKED_RULE: &str = "xcode.derived-data";
const CHECKED_RULE: &str = "docker.vm";
const NEW_FOLDER: &str = "~/Library/Developer/Xcode/DerivedData/Fresh-abc";
const OTHER_HYPOTHESIS: &str = "~/Library/Application Support/com.otherapp.helper";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell_state.json")
}

fn recommendations() -> RecommendationsModel {
    ShellFixture::read(&fixture())
        .and_then(|fixture| fixture.shell_data(Screen::Recommendations))
        .expect("the fixture builds the recommendations screen data")
        .care
        .recommendations
}

fn rule_card<'model>(model: &'model mut RecommendationsModel, rule: &str) -> &'model mut CardModel {
    model
        .cards
        .iter_mut()
        .find(|card| card.provenance == Provenance::Rule(rule.to_owned()))
        .unwrap_or_else(|| panic!("the fixture has a card of {rule}"))
}

fn with_two_hypotheses(mut model: RecommendationsModel) -> RecommendationsModel {
    let mut other = model
        .cards
        .iter()
        .find(|card| card.provenance == Provenance::Hypothesis)
        .expect("the fixture has a hypothesis card")
        .clone();
    other.paths = vec![OTHER_HYPOTHESIS.to_owned()];
    model.cards.push(other);
    model
}

fn hypothesis_selection(model: &RecommendationsModel) -> Vec<(String, bool)> {
    model
        .cards
        .iter()
        .filter(|card| card.provenance == Provenance::Hypothesis)
        .map(|card| (card.paths.join(", "), card.selected))
        .collect()
}

#[test]
fn a_rule_card_keeps_its_selection_when_its_folders_change() {
    let mut previous = recommendations();
    rule_card(&mut previous, UNCHECKED_RULE).selected = false;
    rule_card(&mut previous, CHECKED_RULE).selected = true;
    let mut rebuilt = recommendations();
    for rule in [UNCHECKED_RULE, CHECKED_RULE] {
        rule_card(&mut rebuilt, rule).paths.push(NEW_FOLDER.to_owned());
    }

    rebuilt.keep_selection(&previous);

    assert!(
        !rule_card(&mut rebuilt, UNCHECKED_RULE).selected,
        "an unchecked card stays unchecked when a folder appears in it"
    );
    assert!(
        rule_card(&mut rebuilt, CHECKED_RULE).selected,
        "a checked card stays checked when a folder appears in it"
    );
}

#[test]
fn each_hypothesis_keeps_its_own_selection() {
    let mut previous = with_two_hypotheses(recommendations());
    let first = previous
        .cards
        .iter_mut()
        .find(|card| card.provenance == Provenance::Hypothesis)
        .expect("the fixture has a hypothesis card");
    first.selected = true;
    let first_path = first.paths.join(", ");
    let mut rebuilt = with_two_hypotheses(recommendations());

    rebuilt.keep_selection(&previous);

    assert_eq!(
        hypothesis_selection(&rebuilt),
        [(first_path, true), (OTHER_HYPOTHESIS.to_owned(), false)],
        "a hypothesis is the same card only on the same path"
    );
}
