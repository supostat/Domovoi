use domovoi_theme::{Palette, RADII, Rgba, format_size};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSLineBreakMode, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::assembled_view::{Parts, assemble};
use crate::button_kind::ButtonKind;
use crate::fill_view::{FillStyle, FillView};
use crate::label::label;
use crate::model_card_model::ModelCardModel;
use crate::model_card_state::ModelCardState;
use crate::pill_badge::pill_badge;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_palette::ShellPalette;
use crate::strings::{
    ACTIVE, CANCEL_DOWNLOAD, CHECKING, DOWNLOAD, DOWNLOAD_AGAIN, FAILED, INSTALLED, MAKE_ACTIVE, PAUSE, PAUSED, REMOVE,
    RESUMABLE, RESUME, SHA_MATCHES,
};
use crate::ui_font::{interface_font, mono_font};

const PADDING: NSSize = NSSize::new(18.0, 16.0);
const ROW_GAP: f64 = 10.0;
const BUTTON_GAP: f64 = 8.0;
const BUTTON_HEIGHT: f64 = 26.0;
const PROGRESS_HEIGHT: f64 = 8.0;
const META_GAP: f64 = 12.0;

type ButtonSpec = (String, ButtonKind, Sel);

pub(crate) fn model_card_view(
    marker: MainThreadMarker,
    card: &ModelCardModel,
    position: usize,
    fits: bool,
    width: f64,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> Retained<NSView> {
    let tokens = palette.tokens;
    let inner = width - 2.0 * PADDING.width;
    let tag = isize::try_from(position).expect("a catalog fits in isize");
    let button = |(title, kind, selector): ButtonSpec| {
        let button = ShellButton::new(marker, &title, NSPoint::ZERO, BUTTON_HEIGHT, kind.small_style(palette));
        button.setTag(tag);
        button.on_click(actions, selector);
        button
    };
    let corner: Retained<NSView> = if let Some((title, ink, soft)) = badge(card, tokens) {
        pill_badge(marker, &title, ink, soft).into_super()
    } else {
        let download = button((
            format!("{DOWNLOAD} · {}", format_size(card.size)),
            ButtonKind::Plain,
            sel!(downloadModel:),
        ));
        download.setEnabled(fits);
        download.into_super().into_super().into_super()
    };
    let title = label(
        marker,
        &card.title(),
        &interface_font(14.0, 600.0),
        &tokens.text.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    let corner_size = corner.frame().size;
    let title_natural = title.fittingSize();
    let head_height = title_natural.height.max(corner_size.height);
    let title_size = NSSize::new(
        title_natural.width.min(inner - corner_size.width - META_GAP).ceil(),
        title_natural.height,
    );
    let mut parts: Parts = vec![
        (
            title.into_super().into_super(),
            NSPoint::new(0.0, (head_height - title_size.height) / 2.0),
            title_size,
        ),
        (
            corner,
            NSPoint::new(inner - corner_size.width, (head_height - corner_size.height) / 2.0),
            corner_size,
        ),
    ];
    let mut top = head_height + ROW_GAP;
    if let Some(share) = card.share() {
        parts.push((
            progress(marker, inner, share, palette),
            NSPoint::new(0.0, top),
            NSSize::new(inner, PROGRESS_HEIGHT),
        ));
        top += PROGRESS_HEIGHT + ROW_GAP;
    }
    let (meta, hint, specs) = meta_of(card);
    let buttons = specs.into_iter().map(button).collect();
    meta_row(marker, &mut parts, (top, inner), (&meta, hint, buttons), palette);
    assemble(
        marker,
        parts,
        PADDING,
        FillStyle {
            fill: Some(tokens.surface.ns_color()),
            border: Some(if card.state == ModelCardState::Active {
                tokens.wax.ns_color()
            } else {
                palette.line_soft()
            }),
            radius: f64::from(RADII.m),
            dashed: false,
        },
    )
}

fn badge(card: &ModelCardModel, tokens: &Palette) -> Option<(String, Rgba, Rgba)> {
    let warn = (tokens.warn, tokens.warn_soft);
    let ok = (tokens.ok, tokens.ok_soft);
    let (title, (ink, soft)) = match &card.state {
        ModelCardState::Available => return None,
        ModelCardState::Downloading { .. } => (card.percent().expect("a download has progress"), warn),
        ModelCardState::Paused { .. } => (PAUSED.to_owned(), warn),
        ModelCardState::Checking => (CHECKING.to_owned(), warn),
        ModelCardState::Failed { .. } => (FAILED.to_owned(), (tokens.risk, tokens.risk_soft)),
        ModelCardState::Installed => (INSTALLED.to_owned(), ok),
        ModelCardState::Active => (ACTIVE.to_owned(), ok),
    };
    Some((title, ink, soft))
}

fn meta_of(card: &ModelCardModel) -> (String, Option<&'static str>, Vec<ButtonSpec>) {
    let spec = |title: &str, kind: ButtonKind, selector: Sel| (title.to_owned(), kind, selector);
    let installed = format!(
        "{} · {SHA_MATCHES} · {}",
        card.memory_requirement(),
        format_size(card.size)
    );
    match &card.state {
        ModelCardState::Available => (format!("{} · {}", card.memory_requirement(), card.note), None, vec![]),
        ModelCardState::Downloading { .. } => (
            card.transfer_line().expect("a download has a transfer line"),
            Some(RESUMABLE),
            vec![
                spec(PAUSE, ButtonKind::Plain, sel!(pauseDownload:)),
                spec(CANCEL_DOWNLOAD, ButtonKind::Ghost, sel!(cancelDownload:)),
            ],
        ),
        ModelCardState::Paused { .. } => (
            card.transfer_line().expect("a paused download has a transfer line"),
            Some(RESUMABLE),
            vec![
                spec(RESUME, ButtonKind::Plain, sel!(downloadModel:)),
                spec(CANCEL_DOWNLOAD, ButtonKind::Ghost, sel!(cancelDownload:)),
            ],
        ),
        ModelCardState::Failed { reason } => (
            reason.clone(),
            None,
            vec![spec(DOWNLOAD_AGAIN, ButtonKind::Plain, sel!(downloadModel:))],
        ),
        ModelCardState::Checking => (
            format!("{} · {}", card.memory_requirement(), format_size(card.size)),
            None,
            vec![],
        ),
        ModelCardState::Installed => (
            installed,
            None,
            vec![
                spec(MAKE_ACTIVE, ButtonKind::Plain, sel!(activateModel:)),
                spec(REMOVE, ButtonKind::Ghost, sel!(removeModel:)),
            ],
        ),
        ModelCardState::Active => (
            installed,
            None,
            vec![spec(REMOVE, ButtonKind::Ghost, sel!(removeModel:))],
        ),
    }
}

fn meta_row(
    marker: MainThreadMarker,
    parts: &mut Parts,
    (top, inner): (f64, f64),
    (meta, hint, buttons): (&str, Option<&str>, Vec<Retained<ShellButton>>),
    palette: &ShellPalette,
) {
    let tokens = palette.tokens;
    let font = mono_font(12.0, 400.0);
    let row_height = if buttons.is_empty() { 0.0 } else { BUTTON_HEIGHT };
    let mut right = inner;
    let mut placed = Parts::new();
    for button in buttons.into_iter().rev() {
        let size = button.frame().size;
        right -= size.width;
        placed.push((
            button.into_super().into_super().into_super(),
            NSPoint::new(right, 0.0),
            size,
        ));
        right -= BUTTON_GAP;
    }
    if let Some(hint) = hint {
        let field = label(marker, hint, &font, &tokens.dim.ns_color(), NSLineBreakMode::ByClipping);
        let size = field.fittingSize();
        right -= size.width + META_GAP - BUTTON_GAP;
        placed.push((field.into_super().into_super(), NSPoint::new(right, 0.0), size));
    }
    let field = label(
        marker,
        meta,
        &font,
        &tokens.dim.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    let natural = field.fittingSize();
    let meta_size = NSSize::new(natural.width.min((right - META_GAP).max(0.0)).ceil(), natural.height);
    placed.push((field.into_super().into_super(), NSPoint::ZERO, meta_size));
    let height = row_height.max(natural.height);
    for (view, origin, size) in placed {
        parts.push((view, NSPoint::new(origin.x, top + (height - size.height) / 2.0), size));
    }
}

fn progress(marker: MainThreadMarker, width: f64, share: f64, palette: &ShellPalette) -> Retained<NSView> {
    let track = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(width, PROGRESS_HEIGHT)),
        FillStyle {
            fill: Some(palette.line_soft()),
            border: None,
            radius: PROGRESS_HEIGHT / 2.0,
            dashed: false,
        },
    );
    let fill = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new((width * share).round(), PROGRESS_HEIGHT)),
        FillStyle {
            fill: Some(palette.tokens.wax.ns_color()),
            border: None,
            radius: PROGRESS_HEIGHT / 2.0,
            dashed: false,
        },
    );
    track.addSubview(&fill);
    track.into_super()
}
