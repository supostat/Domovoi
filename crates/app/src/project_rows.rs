use domovoi_theme::{format_count, format_size};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSAccessibility, NSColor, NSFont, NSLineBreakMode, NSTextField};
use objc2_foundation::{NSInteger, NSPoint, NSRect, NSSize, NSString};

use crate::card_model::CardModel;
use crate::check_box::{CheckBox, SIDE};
use crate::fill_view::{FillStyle, FillView};
use crate::label::{label, measure_label};
use crate::project_row::ProjectRow;
use crate::project_row_fonts::ProjectRowFonts;
use crate::project_row_view::ProjectRowView;
use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;
use crate::stacked_column::StackedColumn;
use crate::strings::{FRESH_PROJECTS, PROJECTS, STALE_PROJECTS};

const ROWS_GAP: f64 = 14.0;
const CHECK_GAP: f64 = 10.0;
const COLUMN_GAP: f64 = 12.0;
const ROW_PADDING: f64 = 8.0;
const HEADER_TOP: f64 = 12.0;
const HEADER_BOTTOM: f64 = 6.0;
const LINE_GAP: f64 = 2.0;
const ARTIFACTS_SHARE: f64 = 0.4;
const SEPARATOR: f64 = 1.0;
const SUMMARY_SEPARATOR: &str = " · ";
const ARTIFACTS_SEPARATOR: &str = " · ";

pub(crate) fn project_rows(
    marker: MainThreadMarker,
    card: &CardModel,
    column: &mut StackedColumn,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> Vec<ProjectRowView> {
    let mut views = Vec::new();
    let fonts = ProjectRowFonts::new();
    column.skip(ROWS_GAP);
    push_separator(marker, column, palette);
    for (group, title) in [
        (ProjectRow::STALE_GROUP, STALE_PROJECTS),
        (ProjectRow::FRESH_GROUP, FRESH_PROJECTS),
    ] {
        let rows: Vec<(usize, &ProjectRow)> = card
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.group() == group)
            .collect();
        if rows.is_empty() {
            continue;
        }
        push_header(marker, column, (palette, &fonts), actions, (group, title), &rows);
        for (position, row) in rows {
            views.push(push_row(marker, column, (palette, &fonts), actions, (position, row)));
            push_separator(marker, column, palette);
        }
    }
    views
}

fn push_header(
    marker: MainThreadMarker,
    column: &mut StackedColumn,
    (palette, fonts): (&ShellPalette, &ProjectRowFonts),
    actions: &ShellActions,
    (group, title): (usize, &str),
    rows: &[(usize, &ProjectRow)],
) {
    let tokens = palette.tokens;
    let every = rows.iter().all(|(_, row)| row.selected);
    let some = rows.iter().any(|(_, row)| row.selected);
    let check = CheckBox::new(
        marker,
        title,
        every,
        rows.iter().any(|(_, row)| row.amount > 0),
        palette,
        (
            actions,
            sel!(toggleProjectGroup:),
            NSInteger::try_from(group).expect("groups are two"),
        ),
    );
    if some && !every {
        check.show_mixed();
    }
    let heading = measure_label(marker, title, &fonts.heading, &tokens.dim.ns_color());
    let total: u64 = rows.iter().map(|(_, row)| row.amount).sum();
    let summary = measure_label(
        marker,
        &format!(
            "{}{SUMMARY_SEPARATOR}{}",
            format_count(rows.len() as u64, &PROJECTS),
            format_size(total)
        ),
        &fonts.summary,
        &tokens.dim.ns_color(),
    );
    let heading_size = heading.fittingSize();
    let summary_size = summary.fittingSize();
    let line = SIDE.max(heading_size.height).max(summary_size.height);
    let height = HEADER_TOP + line + HEADER_BOTTOM;
    let container = container(marker, column.width, height);
    let middle = HEADER_BOTTOM + line / 2.0;
    check.setFrameOrigin(NSPoint::new(0.0, middle - SIDE / 2.0));
    heading.setFrame(NSRect::new(
        NSPoint::new(SIDE + CHECK_GAP, middle - heading_size.height / 2.0),
        heading_size,
    ));
    summary.setFrame(NSRect::new(
        NSPoint::new(column.width - summary_size.width, middle - summary_size.height / 2.0),
        summary_size,
    ));
    container.addSubview(&check);
    container.addSubview(&heading);
    container.addSubview(&summary);
    column.push(container.into_super(), height);
}

fn push_row(
    marker: MainThreadMarker,
    column: &mut StackedColumn,
    (palette, fonts): (&ShellPalette, &ProjectRowFonts),
    actions: &ShellActions,
    (position, row): (usize, &ProjectRow),
) -> ProjectRowView {
    let tokens = palette.tokens;
    let check = CheckBox::new(
        marker,
        &row.name,
        row.selected,
        row.amount > 0,
        palette,
        (
            actions,
            sel!(toggleProjectRow:),
            NSInteger::try_from(position).expect("projects fit in an integer"),
        ),
    );
    let name_color = if row.selected { tokens.text } else { tokens.mute };
    let name = truncated(marker, &row.name, &fonts.name, &name_color.ns_color());
    let path = truncated(marker, &row.path, &fonts.path, &tokens.dim.ns_color());
    let joined = row.artifacts.join(ARTIFACTS_SEPARATOR);
    let artifacts = label(
        marker,
        &joined,
        &fonts.artifacts,
        &tokens.mute.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    describe(&artifacts, &joined);
    let age_color = if row.stale { tokens.dim } else { tokens.mute };
    let age = measure_label(marker, &row.age, &fonts.age, &age_color.ns_color());
    let size = measure_label(marker, &format_size(row.amount), &fonts.size, &tokens.text.ns_color());
    let name_height = name.fittingSize().height;
    let path_height = path.fittingSize().height;
    let height = ROW_PADDING + name_height + LINE_GAP + path_height + ROW_PADDING;
    let width = column.width;
    let text_x = SIDE + CHECK_GAP;
    let name_middle = height - ROW_PADDING - name_height / 2.0;
    let path_middle = ROW_PADDING + path_height / 2.0;
    let size_size = size.fittingSize();
    let size_x = width - size_size.width;
    size.setFrame(NSRect::new(
        NSPoint::new(size_x, name_middle - size_size.height / 2.0),
        size_size,
    ));
    let age_size = age.fittingSize();
    let age_x = size_x - COLUMN_GAP - age_size.width;
    age.setFrame(NSRect::new(
        NSPoint::new(age_x, name_middle - age_size.height / 2.0),
        age_size,
    ));
    name.setFrame(NSRect::new(
        NSPoint::new(text_x, height - ROW_PADDING - name_height),
        NSSize::new((age_x - COLUMN_GAP - text_x).max(0.0), name_height),
    ));
    let artifacts_fitting = artifacts.fittingSize();
    let artifacts_width = artifacts_fitting.width.min(width * ARTIFACTS_SHARE);
    let artifacts_x = width - artifacts_width;
    artifacts.setFrame(NSRect::new(
        NSPoint::new(artifacts_x, path_middle - artifacts_fitting.height / 2.0),
        NSSize::new(artifacts_width, artifacts_fitting.height),
    ));
    path.setFrame(NSRect::new(
        NSPoint::new(text_x, ROW_PADDING),
        NSSize::new((artifacts_x - COLUMN_GAP - text_x).max(0.0), path_height),
    ));
    check.setFrameOrigin(NSPoint::new(0.0, height / 2.0 - SIDE / 2.0));
    let container = container(marker, width, height);
    for view in [
        check.clone().into_super().into_super().into_super(),
        name.clone().into_super().into_super(),
        path.clone().into_super().into_super(),
        artifacts.clone().into_super().into_super(),
        age.clone().into_super().into_super(),
        size.clone().into_super().into_super(),
    ] {
        container.addSubview(&view);
    }
    column.push(container.into_super(), height);
    ProjectRowView {
        check,
        name,
        path,
        artifacts,
        age,
        size,
    }
}

fn truncated(marker: MainThreadMarker, text: &str, font: &NSFont, color: &NSColor) -> Retained<NSTextField> {
    let field = label(marker, text, font, color, NSLineBreakMode::ByTruncatingMiddle);
    describe(&field, text);
    field
}

fn describe(field: &NSTextField, text: &str) {
    let full = NSString::from_str(text);
    field.setToolTip(Some(&full));
    field.setAccessibilityLabel(Some(&full));
}

fn container(marker: MainThreadMarker, width: f64, height: f64) -> Retained<FillView> {
    FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(width, height)),
        FillStyle {
            fill: None,
            border: None,
            radius: 0.0,
            dashed: false,
        },
    )
}

fn push_separator(marker: MainThreadMarker, column: &mut StackedColumn, palette: &ShellPalette) {
    let line = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(column.width, SEPARATOR)),
        FillStyle {
            fill: Some(palette.line_soft()),
            border: None,
            radius: 0.0,
            dashed: false,
        },
    );
    column.push(line.into_super(), SEPARATOR);
}
