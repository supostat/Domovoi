use domovoi_render::category_color;
use domovoi_theme::format_size;
use objc2::rc::Retained;

use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSAccessibility, NSLineBreakMode, NSView};
use objc2_foundation::{NSInteger, NSPoint, NSRect, NSSize, NSString};

use crate::button_style::ButtonStyle;
use crate::field_ornament::{FieldOrnament, OrnamentInk};
use crate::fill_view::{FillStyle, FillView};
use crate::label::{label, measure_label};
use crate::probes::Probes;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_layout::NARROW_WINDOW;
use crate::shell_palette::ShellPalette;
use crate::strings::{
    CANCEL, CLOSE, FREED_AFTER_EMPTYING, MOVE_TO_TRASH, NOT_MOVED, OPEN_TRASH, RECENTLY_MOVED, RESTORE, TRAY,
    TRAY_EMPTY, TRAY_NOTE, WILL_MOVE,
};
use crate::tray_column::{MARGIN, TrayColumn};
use crate::tray_model::TrayModel;
use crate::tray_row::TrayRow;
use crate::ui_font::{interface_font, mono_font};

const ROW_HEIGHT: f64 = 40.0;
const DOT: f64 = 10.0;
const DOT_RADIUS: f64 = 3.0;
const GAP: f64 = 12.0;
const SECTION_GAP: f64 = 22.0;
const SMALL_BUTTON_HEIGHT: f64 = 26.0;

#[derive(Debug)]
pub struct TrayScreen {
    pub root: Retained<FillView>,
    pub primary: Retained<ShellButton>,
    pub(crate) ornament: Option<Retained<FieldOrnament>>,
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
        let primary = if let Some(outcome) = &model.outcome {
            let stitched = FieldOrnament::new(
                marker,
                NSRect::new(NSPoint::ZERO, NSSize::new(column.width, FieldOrnament::height())),
                OrnamentInk {
                    outline: tokens.wax,
                    fill: tokens.berry,
                    center: tokens.text,
                },
            );
            column.add(
                stitched.clone().into_super(),
                MARGIN,
                NSSize::new(column.width, FieldOrnament::height()),
            );
            column.top += FieldOrnament::height() + SECTION_GAP;
            ornament = Some(stitched);
            column.paragraph(
                &TrayModel::moved_line(outcome.moved),
                &interface_font(22.0, 600.0),
                &tokens.text.ns_color(),
            );
            column.top += GAP / 2.0;
            column.paragraph(
                FREED_AFTER_EMPTYING,
                &interface_font(13.0, 400.0),
                &tokens.dim.ns_color(),
            );
            if !outcome.failures.is_empty() {
                column.top += GAP;
                column.paragraph(NOT_MOVED, &interface_font(13.0, 600.0), &tokens.text.ns_color());
                for failure in &outcome.failures {
                    column.paragraph(failure, &interface_font(12.5, 400.0), &tokens.mute.ns_color());
                }
            }
            column.top += SECTION_GAP;
            column.buttons(&[(OPEN_TRASH, sel!(openTrash:), false), (CLOSE, sel!(closeTray:), true)])
        } else if model.rows.is_empty() {
            column.paragraph(TRAY_EMPTY, &interface_font(14.0, 400.0), &tokens.mute.ns_color());
            column.top += SECTION_GAP;
            column.buttons(&[(CLOSE, sel!(closeTray:), true)])
        } else {
            confirmation(&mut column, model);
            column.buttons(&[
                (CANCEL, sel!(closeTray:), false),
                (MOVE_TO_TRASH, sel!(confirmTray:), true),
            ])
        };
        if !model.journal.is_empty() {
            column.top += SECTION_GAP;
            column.caps(RECENTLY_MOVED);
            column.top += GAP / 2.0;
            for row in &model.journal {
                journal_row(&mut column, row, palette, actions);
            }
        }
        let primary_frame = column.primary.expect("the primary button is placed");
        let height = column.finish();
        Self {
            root,
            primary,
            ornament,
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

fn confirmation(column: &mut TrayColumn<'_>, model: &TrayModel) {
    let tokens = column.palette.tokens;
    for row in &model.rows {
        tray_row(column, row);
    }
    column.top += GAP;
    let caption = measure_label(
        column.marker,
        WILL_MOVE,
        &interface_font(13.5, 400.0),
        &tokens.mute.ns_color(),
    );
    let total = measure_label(
        column.marker,
        &format_size(model.total()),
        &mono_font(26.0, 600.0),
        &tokens.wax.ns_color(),
    );
    let caption_size = caption.fittingSize();
    let total_size = total.fittingSize();
    let line = total_size.height.max(caption_size.height);
    column.placed.push((
        caption.into_super().into_super(),
        MARGIN,
        column.top + line - caption_size.height,
        caption_size,
    ));
    column.add(
        total.into_super().into_super(),
        MARGIN + column.width - total_size.width,
        total_size,
    );
    column.top += line + GAP;
    if let Some(held) = model.held_line() {
        column.paragraph(&held, &interface_font(12.5, 400.0), &tokens.dim.ns_color());
        column.top += GAP / 2.0;
    }
    column.paragraph(TRAY_NOTE, &interface_font(13.0, 400.0), &tokens.dim.ns_color());
    column.top += SECTION_GAP;
}

fn tray_row(column: &mut TrayColumn<'_>, row: &TrayRow) {
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
    let size_size = size.fittingSize();
    size.setFrameOrigin(NSPoint::new(
        column.width - size_size.width,
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
        NSSize::new(
            (column.width - DOT - 3.0 * GAP - size_size.width).max(0.0),
            title_height,
        ),
    ));
    for child in [&*rule as &NSView, &dot, &title, &size] {
        band.addSubview(child);
    }
    column.add(band.into_super(), MARGIN, NSSize::new(column.width, ROW_HEIGHT));
    column.top += ROW_HEIGHT;
}

fn journal_row(
    column: &mut TrayColumn<'_>,
    row: &crate::journal_row::JournalRow,
    palette: &ShellPalette,
    actions: &ShellActions,
) {
    let tokens = palette.tokens;
    let restore = ShellButton::new(
        column.marker,
        RESTORE,
        NSPoint::ZERO,
        SMALL_BUTTON_HEIGHT,
        ButtonStyle {
            fill: None,
            border: Some(palette.line()),
            text: tokens.text.ns_color(),
            font: interface_font(12.0, 600.0),
        },
    );
    restore.on_click(actions, sel!(restore:));
    restore.setTag(NSInteger::try_from(row.position).expect("journal positions fit"));
    let restore_size = restore.frame().size;
    let size = measure_label(
        column.marker,
        &format_size(row.size),
        &mono_font(12.0, 500.0),
        &tokens.mute.ns_color(),
    );
    let size_size = size.fittingSize();
    let text_width = (column.width - restore_size.width - size_size.width - 2.0 * GAP).max(0.0);
    let title = label(
        column.marker,
        &row.title,
        &interface_font(13.0, 500.0),
        &tokens.text.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    let path = label(
        column.marker,
        &row.path,
        &mono_font(11.5, 400.0),
        &tokens.dim.ns_color(),
        NSLineBreakMode::ByTruncatingMiddle,
    );
    for (field, text) in [(&title, &row.title), (&path, &row.path)] {
        let full = NSString::from_str(text);
        field.setToolTip(Some(&full));
        field.setAccessibilityLabel(Some(&full));
    }
    let title_height = title.fittingSize().height;
    let path_height = path.fittingSize().height;
    let height = (title_height + path_height).max(SMALL_BUTTON_HEIGHT) + GAP;
    let top = column.top;
    column.placed.push((
        title.into_super().into_super(),
        MARGIN,
        top + GAP / 2.0,
        NSSize::new(text_width, title_height),
    ));
    column.placed.push((
        path.into_super().into_super(),
        MARGIN,
        top + GAP / 2.0 + title_height,
        NSSize::new(text_width, path_height),
    ));
    column.placed.push((
        size.into_super().into_super(),
        MARGIN + text_width + GAP,
        top + (height - size_size.height) / 2.0,
        size_size,
    ));
    column.placed.push((
        restore.into_super().into_super().into_super(),
        MARGIN + column.width - restore_size.width,
        top + (height - SMALL_BUTTON_HEIGHT) / 2.0,
        restore_size,
    ));
    column.top += height;
}
