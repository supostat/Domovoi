use domovoi_render::category_color;
use domovoi_theme::format_size;
use objc2::rc::Retained;

use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSAccessibility, NSLineBreakMode, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::button_style::ButtonStyle;
use crate::delete_outcome::DeleteOutcome;
use crate::field_ornament::FieldOrnament;
use crate::fill_view::{FillStyle, FillView};
use crate::label::{label, measure_label};
use crate::probes::Probes;
use crate::progress_bar::{PROGRESS_HEIGHT, progress_bar};
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_layout::NARROW_WINDOW;
use crate::shell_palette::ShellPalette;
use crate::strings::{
    CANCEL, CLOSE, DELETE, ESTIMATE_MARK, NOT_DELETED, NOT_MARKED_NAME_ONLY, STOP, TRAY, TRAY_EMPTY, TRAY_NOTE, UNMARK,
    UNMARK_MARK, WILL_DELETE,
};
use crate::tray_button::TrayButton;
use crate::tray_column::{GAP, MARGIN, SECTION_GAP, TrayColumn};
use crate::tray_model::TrayModel;
use crate::tray_row::TrayRow;
use crate::tray_row_views::TrayRowViews;
use crate::ui_font::{interface_font, mono_font};

const ROW_HEIGHT: f64 = 40.0;
const DOT: f64 = 10.0;
const DOT_RADIUS: f64 = 3.0;
const DETAIL_GAP: f64 = 6.0;
const UNMARK_SIZE: f64 = 24.0;

#[derive(Debug)]
pub struct TrayScreen {
    pub root: Retained<FillView>,
    pub primary: Retained<ShellButton>,
    pub(crate) ornament: Option<Retained<FieldOrnament>>,
    pub details: Vec<Retained<NSTextField>>,
    pub rows: Vec<TrayRowViews>,
    pub probes: Probes,
}

impl TrayScreen {
    pub fn build(marker: MainThreadMarker, model: &TrayModel, palette: &ShellPalette, actions: &ShellActions) -> Self {
        let tokens = palette.tokens;
        let root = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, NARROW_WINDOW),
            FillStyle {
                fill: Some(tokens.bg.ns_color()),
                border: None,
                radius: 0.0,
                dashed: false,
            },
        );
        let mut column = TrayColumn {
            marker,
            root: &root,
            palette,
            actions,
            width: NARROW_WINDOW.width - 2.0 * MARGIN,
            top: MARGIN,
            placed: Vec::new(),
            primary: None,
        };
        column.paragraph(TRAY, &interface_font(17.0, 600.0), &tokens.text.ns_color());
        column.top += GAP;
        let mut ornament = None;
        let mut details = Vec::new();
        let mut rows = Vec::new();
        let primary = if let Some(outcome) = &model.deleted {
            let (primary, stitched) = deleted(&mut column, outcome);
            ornament = Some(stitched);
            primary
        } else if model.rows.is_empty() {
            column.paragraph(TRAY_EMPTY, &interface_font(14.0, 400.0), &tokens.mute.ns_color());
            column.top += SECTION_GAP;
            column.buttons(&[(CLOSE, sel!(closeTray:), TrayButton::Primary)])
        } else {
            (details, rows) = confirmation(&mut column, model);
            if let Some(deleting) = model.deleting_line() {
                column.paragraph(&deleting, &interface_font(13.5, 600.0), &tokens.text.ns_color());
                column.top += GAP;
                let share = model.deleting_share().expect("a delete in progress has a share");
                column.add(
                    progress_bar(marker, column.width, share, palette),
                    MARGIN,
                    NSSize::new(column.width, PROGRESS_HEIGHT),
                );
                column.top += PROGRESS_HEIGHT + SECTION_GAP;
                column.buttons(&[(STOP, sel!(stopDelete:), TrayButton::Primary)])
            } else {
                column.buttons(&[
                    (CANCEL, sel!(closeTray:), TrayButton::Secondary),
                    (DELETE, sel!(deleteTray:), TrayButton::Primary),
                ])
            }
        };
        let primary_frame = column.primary.expect("the primary button is placed");
        let height = column.finish();
        Self {
            root,
            primary,
            ornament,
            details,
            rows,
            probes: Probes {
                window_background: [NARROW_WINDOW.width - 6.0, height - 6.0],
                sidebar: None,
                accent_button: Some(primary_frame),
            },
        }
    }

    pub fn size(&self) -> NSSize {
        self.root.frame().size
    }
}

fn deleted(column: &mut TrayColumn<'_>, outcome: &DeleteOutcome) -> (Retained<ShellButton>, Retained<FieldOrnament>) {
    let tokens = column.palette.tokens;
    let stitched = column.ornament();
    column.paragraph(&outcome.line(), &interface_font(22.0, 600.0), &tokens.text.ns_color());
    if !outcome.failures.is_empty() {
        column.top += GAP;
        column.paragraph(NOT_DELETED, &interface_font(13.0, 600.0), &tokens.text.ns_color());
        for failure in &outcome.failures {
            column.paragraph(failure, &interface_font(12.5, 400.0), &tokens.mute.ns_color());
        }
    }
    column.top += SECTION_GAP;
    let primary = column.buttons(&[(CLOSE, sel!(closeTray:), TrayButton::Primary)]);
    (primary, stitched)
}

fn confirmation(column: &mut TrayColumn<'_>, model: &TrayModel) -> (Vec<Retained<NSTextField>>, Vec<TrayRowViews>) {
    let tokens = column.palette.tokens;
    let unmarkable = model.deleting.is_none();
    let mut details = Vec::new();
    let mut rows = Vec::new();
    for (position, row) in model.rows.iter().enumerate() {
        rows.push(tray_row(column, row, unmarkable.then_some(position)));
        details.extend(row_details(column, row));
    }
    column.top += GAP;
    let total_text = if model.measuring.is_some() {
        format!("{ESTIMATE_MARK} {}", format_size(model.total()))
    } else {
        format_size(model.total())
    };
    column.figure(WILL_DELETE, &total_text);
    column.top += GAP;
    if let Some(measuring) = model.measuring_line() {
        column.paragraph(&measuring, &interface_font(12.5, 400.0), &tokens.mute.ns_color());
        column.top += GAP / 2.0;
    }
    if let Some(held) = model.held_line() {
        column.paragraph(&held, &interface_font(12.5, 400.0), &tokens.dim.ns_color());
        column.top += GAP / 2.0;
    }
    column.paragraph(TRAY_NOTE, &interface_font(13.0, 400.0), &tokens.dim.ns_color());
    column.top += SECTION_GAP;
    (details, rows)
}

fn row_details(column: &mut TrayColumn<'_>, row: &TrayRow) -> Vec<Retained<NSTextField>> {
    if row.details.is_empty() && row.unmarked.is_empty() {
        return Vec::new();
    }
    let tokens = column.palette.tokens;
    let indent = DOT + GAP;
    let path_font = mono_font(11.5, 400.0);
    let dim = tokens.dim.ns_color();
    let mut fields = Vec::new();
    column.top += DETAIL_GAP;
    for detail in &row.details {
        fields.push(column.indented_line(detail, (&path_font, &dim), indent));
    }
    if !row.unmarked.is_empty() {
        column.top += DETAIL_GAP;
        let heading = interface_font(12.0, 600.0);
        fields.push(column.indented_line(NOT_MARKED_NAME_ONLY, (&heading, &tokens.mute.ns_color()), indent));
        for path in &row.unmarked {
            fields.push(column.indented_line(path, (&path_font, &dim), indent));
        }
    }
    column.top += DETAIL_GAP;
    fields
}

fn tray_row(column: &mut TrayColumn<'_>, row: &TrayRow, unmark_position: Option<usize>) -> TrayRowViews {
    let tokens = column.palette.tokens;
    let band = FillView::new(
        column.marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(column.width, ROW_HEIGHT)),
        FillStyle {
            fill: None,
            border: None,
            radius: 0.0,
            dashed: false,
        },
    );
    let rule = FillView::new(
        column.marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(column.width, 1.0)),
        FillStyle {
            fill: Some(column.palette.line_soft()),
            border: None,
            radius: 0.0,
            dashed: false,
        },
    );
    let dot = FillView::new(
        column.marker,
        NSRect::new(NSPoint::new(0.0, (ROW_HEIGHT - DOT) / 2.0), NSSize::new(DOT, DOT)),
        FillStyle {
            fill: Some(category_color(tokens, row.category).ns_color()),
            border: None,
            radius: DOT_RADIUS,
            dashed: false,
        },
    );
    let size = measure_label(
        column.marker,
        &format_size(row.size),
        &mono_font(12.5, 500.0),
        &tokens.mute.ns_color(),
    );
    let unmark = unmark_position.map(|position| unmark_button(column, row, position));
    let size_right = unmark
        .as_ref()
        .map_or(column.width, |button| button.frame().origin.x - GAP);
    let size_size = size.fittingSize();
    size.setFrameOrigin(NSPoint::new(
        size_right - size_size.width,
        (ROW_HEIGHT - size_size.height) / 2.0,
    ));
    let title = label(
        column.marker,
        &row.title,
        &interface_font(13.5, 400.0),
        &tokens.text.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    let full = NSString::from_str(&row.title);
    title.setToolTip(Some(&full));
    title.setAccessibilityLabel(Some(&full));
    let title_height = title.fittingSize().height;
    title.setFrame(NSRect::new(
        NSPoint::new(DOT + GAP, (ROW_HEIGHT - title_height) / 2.0),
        NSSize::new((size_right - DOT - 3.0 * GAP - size_size.width).max(0.0), title_height),
    ));
    for child in [&*rule as &NSView, &dot, &title, &size] {
        band.addSubview(child);
    }
    if let Some(button) = &unmark {
        band.addSubview(button);
    }
    column.add(band.into_super(), MARGIN, NSSize::new(column.width, ROW_HEIGHT));
    column.top += ROW_HEIGHT;
    TrayRowViews { title, size, unmark }
}

fn unmark_button(column: &TrayColumn<'_>, row: &TrayRow, position: usize) -> Retained<ShellButton> {
    let tokens = column.palette.tokens;
    let button = ShellButton::new(
        column.marker,
        UNMARK_MARK,
        NSPoint::ZERO,
        UNMARK_SIZE,
        ButtonStyle {
            fill: None,
            border: None,
            text: tokens.dim.ns_color(),
            font: interface_font(17.0, 400.0),
        },
    );
    button.setFrame(NSRect::new(
        NSPoint::new(column.width - UNMARK_SIZE, (ROW_HEIGHT - UNMARK_SIZE) / 2.0),
        NSSize::new(UNMARK_SIZE, UNMARK_SIZE),
    ));
    button.setTag(isize::try_from(position).expect("a tray has few rows"));
    button.on_click(column.actions, sel!(unmarkTrayRow:));
    let label = NSString::from_str(&format!("{UNMARK}: {}", row.title));
    button.setToolTip(Some(&label));
    button.setAccessibilityLabel(Some(&label));
    button
}
