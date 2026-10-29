use domovoi_theme::format_size;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::NSTextField;
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::field_ornament::FieldOrnament;
use crate::fill_view::{FillStyle, FillView};
use crate::probes::Probes;
use crate::progress_bar::{PROGRESS_HEIGHT, progress_bar};
use crate::purge_model::PurgeModel;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_layout::NARROW_WINDOW;
use crate::shell_palette::ShellPalette;
use crate::strings::{CANCEL, CLOSE, PURGE_ESTIMATE, PURGE_RELEASE, PURGE_TITLE, STOP};
use crate::tray_button::TrayButton;
use crate::tray_column::{GAP, MARGIN, SECTION_GAP, TrayColumn};
use crate::ui_font::interface_font;

#[derive(Debug)]
pub struct PurgeScreen {
    pub root: Retained<FillView>,
    pub primary: Retained<ShellButton>,
    pub(crate) ornament: Option<Retained<FieldOrnament>>,
    pub lines: Vec<Retained<NSTextField>>,
    pub probes: Probes,
}

impl PurgeScreen {
    pub fn build(marker: MainThreadMarker, model: &PurgeModel, palette: &ShellPalette, actions: &ShellActions) -> Self {
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
        column.paragraph(PURGE_TITLE, &interface_font(17.0, 600.0), &tokens.text.ns_color());
        column.top += GAP;
        let mut lines = Vec::new();
        let mut ornament = None;
        let primary = match model {
            PurgeModel::Confirming { estimate, .. } => {
                let (primary, goes) = confirmation(&mut column, *estimate, model);
                lines.push(goes);
                primary
            }
            PurgeModel::Releasing(_) => {
                let (primary, releasing) = releasing(&mut column, model);
                lines.push(releasing);
                primary
            }
            PurgeModel::Released(outcome) => {
                if outcome.is_ok() {
                    ornament = Some(column.ornament());
                }
                released(&mut column, model, outcome.is_ok(), &mut lines)
            }
        };
        let primary_frame = column.primary.expect("the primary button is placed");
        let height = column.finish();
        Self {
            root,
            primary,
            ornament,
            lines,
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

fn confirmation(
    column: &mut TrayColumn<'_>,
    estimate: u64,
    model: &PurgeModel,
) -> (Retained<ShellButton>, Retained<NSTextField>) {
    let tokens = column.palette.tokens;
    column.figure(PURGE_ESTIMATE, &format_size(estimate));
    column.top += GAP;
    let goes = column.paragraph(
        model.goes_line().expect("the confirmation names what goes"),
        &interface_font(13.0, 400.0),
        &tokens.dim.ns_color(),
    );
    column.top += SECTION_GAP;
    let primary = column.buttons(&[
        (CANCEL, sel!(closePurge:), TrayButton::Cancel),
        (PURGE_RELEASE, sel!(releasePurge:), TrayButton::Primary),
    ]);
    primary.setHasDestructiveAction(true);
    (primary, goes)
}

fn releasing(column: &mut TrayColumn<'_>, model: &PurgeModel) -> (Retained<ShellButton>, Retained<NSTextField>) {
    let tokens = column.palette.tokens;
    let line = column.paragraph(
        &model.releasing_line().expect("a release in progress has a line"),
        &interface_font(13.5, 600.0),
        &tokens.text.ns_color(),
    );
    column.top += GAP;
    let share = model.releasing_share().expect("a release in progress has a share");
    column.add(
        progress_bar(column.marker, column.width, share, column.palette),
        MARGIN,
        NSSize::new(column.width, PROGRESS_HEIGHT),
    );
    column.top += PROGRESS_HEIGHT + SECTION_GAP;
    let primary = column.buttons(&[(STOP, sel!(stopPurge:), TrayButton::Primary)]);
    (primary, line)
}

fn released(
    column: &mut TrayColumn<'_>,
    model: &PurgeModel,
    succeeded: bool,
    lines: &mut Vec<Retained<NSTextField>>,
) -> Retained<ShellButton> {
    let tokens = column.palette.tokens;
    let released = model.released_lines();
    let (headline, reasons) = released.split_first().expect("a finished release has its line");
    let headline_font = if succeeded {
        interface_font(22.0, 600.0)
    } else {
        interface_font(13.5, 600.0)
    };
    lines.push(column.paragraph(headline, &headline_font, &tokens.text.ns_color()));
    for reason in reasons {
        column.top += GAP;
        lines.push(column.paragraph(reason, &interface_font(12.5, 400.0), &tokens.mute.ns_color()));
    }
    column.top += SECTION_GAP;
    column.buttons(&[(CLOSE, sel!(closePurge:), TrayButton::Primary)])
}
