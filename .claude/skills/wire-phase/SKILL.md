---
name: wire-phase
description: Carry a DOMOVOI wire phase, the last phase of a spec that proves a feature on the bundled main path - a --e2e scenario step that goes through the application's own functions, its place and checks in wire_main_path, the bundle rebuilt before the test, a red run on disk images sorted by its panic line, the docs/DOMOVOI.md paragraph and the design book sample checked by a headless Chrome screenshot, and the phase note that says what the main path proves and what it does not. Use when a mneme phase with id wire runs in DOMOVOI, when a feature needs a step on the bundled main path, or when wire_main_path fails.
---

# /wire-phase — prove a feature on the bundled main path

The wire phase closes a spec: the earlier phases proved their modules in their own tests, this one
runs the bundled application through a scenario on a disk image and checks the feature end to end.

Live examples: the steps `mark_from_list`, `unmark_tray_row` and `purge_space` in
`crates/app/src/end_to_end.rs`, their checks `assert_tray_drag` and `assert_space_release` in
`crates/app/tests/wire_main_path.rs`. When this file and that code disagree, the code wins and this
file is fixed in the same commit.

The phase's recall bundle carries the notes of earlier wire phases; read them before the code.

## The scenario step

- First decide: a new step, or an existing step extended. A feature that continues an action the
  main path already performs extends that step's report and its `assert_*` — the tray marks added
  `marked_rows` and `collapsed_segments` to `mark_from_list` and `unmark_tray_row` and their
  checks to `assert_tray_drag`. A feature with an action of its own gets a new step.
- A new step is a variant of `ScenarioStep` in `crates/app/src/scenario_step.rs`
  (`#[serde(tag = "step", rename_all = "snake_case", deny_unknown_fields)]`), its name in
  `step_name` and its arm in `EndToEnd::perform`, both in `crates/app/src/end_to_end.rs`.
- The step method goes through the application's own functions, never a copy of their logic: a
  drop goes through `Session::judge_drop` and `Session::drop_into_tray`, a level of the disk screen
  through `ShellData::level_rows` and `MainScreen::diagram_state`. What the step reports is what
  the window would show.
- `live_data` in `end_to_end.rs` mirrors `ShellState::rebuild_data` in
  `crates/app/src/shell_state.rs`. When the application's rebuild gains a call (as
  `ShellData::mark_tray` did), `live_data` gains it too — otherwise the scenario checks data the
  application never builds.
- The step returns its own JSON object, `"step": "<name>"` plus snake_case fields;
  `run_end_to_end` collects them under `steps` in the report. An `Err` stops the scenario and lands
  in the report as `{ "step", "error" }`. A failure of the mechanism under test that the scenario
  must survive goes into the step's own `"error"` field instead, and the check expects it null —
  `purge_space` does so.
- `Result<Value, String>` only for a step that can fail. A step that cannot returns `Value` and its
  arm in `perform` wraps it in `Ok` — otherwise clippy refuses it with `unnecessary_wraps`. An
  extension that makes a step fallible moves it to `Result`, as the tray marks did with
  `mark_from_list`.
- No comment in the step names the spec or the phase: `cargo xtask check-comments` refuses them.

## The scenario and the checks

- Fixtures are the constants at the top of `wire_main_path.rs` and the folders `populate` writes on
  the image (it calls `populate_applications` and `populate_cleanup`; a feature with many fixtures
  gets its own `populate_<feature>` called from there).
- The step's place in `scenario()` is where the volume is in the state the step expects and where
  it does not shift what later steps check: `mark_from_list` and `unmark_tray_row` stand after
  `delete_recommendations` `check_yourself`, so the tray list is empty before and after them;
  `purge_space` stands after `tmutil_commands` and is followed by a second `hidden_space`.
- The checks are one function `assert_<feature>(report, …)` that reads the step through `step`
  (the first occurrence) or `steps` (every occurrence), called in
  `the_bundled_application_walks_the_main_path_on_real_modules` in the order of the steps in the
  scenario: `assert_space_release` and `assert_tray_drag` stand before `assert_model_and_answer`
  and `assert_snapshots`, as their steps do.
- Where the feature can be undone, the scenario does and undoes it, and the checks cover both
  directions (a mark, then its removal).

## Running it

- The test runs the bundled binary,
  `target/bundle/DOMOVOI.app/Contents/MacOS/domovoi --e2e <scenario> --report <report>`
  (`crates/app/src/command_line.rs`). A stale bundle runs old code, so every run is two commands:

  ```
  cargo xtask bundle --profile adhoc
  cargo test -p domovoi-app --test wire_main_path
  ```

- In the spec, the wire phase's done-when lists `cargo xtask bundle --profile adhoc` before
  `cargo test -p domovoi-app --test wire_main_path`, both with `timeout=900s`, as
  `docs/SPEC-TRAY-MARKS.md` does.

## A red run on disk images

Read the panic line first — its file, line and message decide what the red means.

- The panic is in the phase's own `assert_<feature>`: a defect of the phase, fix it.
- The panic is on `.expect("the image attaches")` right after `ImageVolume::create` in a test
  file (hdiutil attaching while other image tests run in parallel — `rescan`,
  `smoke_volume_delta`, `wire_main_path`) or in another
  feature's check (`assert_space_release` once saw `freed 32768` on `purge_space`): rerun the same
  bundle, unchanged, up to four times, and keep every output.
  - The reruns are green: a flake. It is not fixed inside the phase; the harvest carries a
    decision artifact that quotes the red output and the reruns and asks for a separate
    `/mneme:fix`.
  - The red holds: treat it as a defect, whatever line it is on.
- A red image test in a full `cargo test -p domovoi-app`: rerun with `--no-fail-fast` and compare
  the panic lines before touching any code.

## The document and the book

- The feature's text in `docs/DOMOVOI.md`: an addition to the paragraph that already describes its
  screen, or a new paragraph next to it for a new behaviour — what the user sees and its rules.
- The sample in `design/domovoi-design-book.html` changes with the feature. Before its caption is
  written, take a screenshot with headless Chrome and read the image:

  ```
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars --user-data-dir=<temporary folder> --window-size=<width>,<height> --screenshot=<png> file://<absolute path of the book>
  ```

  The caption names what the screenshot shows. In run cdec2202 a caption named Music while the
  sample's second row was Containers.
- Then `cargo xtask check-sources`: the document and the book must not drift from the decisions.

## Before the gate

```
cargo test -p domovoi-app
cargo clippy -p domovoi-app --all-targets -- -D warnings
cargo xtask check-comments
cargo xtask check-tokens
cargo xtask check-sources
```

## The phase note

The harvest carries one decision artifact, in Russian, shaped like the notes of earlier wire
phases:

- Decision: the step and its report fields, its place in the scenario, the checks, the document
  paragraph, the book sample.
- Rationale, three parts:
  - «Главный путь доказывает цепочку на настоящих модулях бандла: A → B → C» — the chain of real
    functions the step went through;
  - «Не доказывает (приёмка человеком на бандле): …» — what only a person sees on
    `target/bundle/DOMOVOI.app`: gestures, animation, the look on screen, live sheets and timers;
  - «Тест читает target/bundle — бандл пересобирается перед ним».

The phase boundary is the human acceptance point: the closing message names what to look at on the
bundle.
