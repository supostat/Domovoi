use domovoi_app::{DeleteStatus, TrayModel};

const NO_BREAK_SPACE: char = '\u{A0}';

fn deleting(files_done: usize, files_total: usize) -> TrayModel {
    TrayModel::default().with_deleting(DeleteStatus {
        files_done,
        files_total,
    })
}

fn spoken_line(model: &TrayModel) -> String {
    model
        .deleting_line()
        .expect("a delete in progress has a line")
        .replace(NO_BREAK_SPACE, " ")
}

#[test]
fn the_sheet_counts_deleted_files_and_the_bar_follows_them() {
    let model = deleting(1_200, 3_400);
    assert_eq!(spoken_line(&model), "Удаляю: 1 200 из 3 400 файлов");
    assert_eq!(model.deleting_share(), Some(1_200.0 / 3_400.0));
}

#[test]
fn the_bar_is_full_once_every_file_is_gone() {
    let model = deleting(3_400, 3_400);
    assert_eq!(spoken_line(&model), "Удаляю: 3 400 из 3 400 файлов");
    assert_eq!(model.deleting_share(), Some(1.0));
}

#[test]
fn the_bar_is_empty_when_the_items_hold_no_files() {
    let model = deleting(0, 0);
    assert_eq!(spoken_line(&model), "Удаляю: 0 из 0 файлов");
    assert_eq!(model.deleting_share(), Some(0.0));
}

#[test]
fn the_total_takes_the_genitive_after_of() {
    assert_eq!(spoken_line(&deleting(0, 1)), "Удаляю: 0 из 1 файла");
    assert_eq!(spoken_line(&deleting(1, 21)), "Удаляю: 1 из 21 файла");
    assert_eq!(spoken_line(&deleting(2, 3)), "Удаляю: 2 из 3 файлов");
}

#[test]
fn a_sheet_that_is_not_deleting_has_no_line_and_no_bar() {
    let model = TrayModel::default();
    assert_eq!(model.deleting_line(), None);
    assert_eq!(model.deleting_share(), None);
}
