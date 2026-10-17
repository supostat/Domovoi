use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSAccessibility, NSColor, NSFont, NSLineBreakMode, NSTextField};
use objc2_foundation::{NSInteger, NSPoint, NSRect, NSSize, NSString};

use crate::card_model::CardModel;
use crate::check_box::{CheckBox, SIDE};
use crate::fill_view::{FillStyle, FillView};
use crate::label::{label, measure_label};
use crate::pill_badge::pill_badge;
use crate::provenance::Provenance;
use crate::provenance_tag::provenance_tag;
use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;
use crate::stacked_column::StackedColumn;
use crate::ui_font::{interface_font, mono_font};

const PADDING_X: f64 = 18.0;
const PADDING_Y: f64 = 16.0;
const COLUMN_GAP: f64 = 16.0;
const RADIUS: f64 = 10.0;
const TITLE_LINES: isize = 2;
const PATH_GAP: f64 = 3.0;
const WHY_GAP: f64 = 8.0;
const LINE_GAP: f64 = 4.0;
const TAGS_GAP: f64 = 10.0;
const TAG_SPACING: f64 = 6.0;
const CHECK_OFFSET: f64 = 3.0;

#[derive(Debug)]
pub struct RecommendationCard {
    pub view: Retained<FillView>,
    pub check: Retained<CheckBox>,
    pub title: Retained<NSTextField>,
    pub path: Retained<NSTextField>,
    pub amount: Retained<NSTextField>,
    pub height: f64,
}

impl RecommendationCard {
    pub fn build(
        marker: MainThreadMarker,
        card: &CardModel,
        position: usize,
        width: f64,
        palette: &ShellPalette,
        actions: &ShellActions,
    ) -> Self {
        let tokens = palette.tokens;
        let hypothesis = matches!(card.provenance, Provenance::Hypothesis);
        let view = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, NSSize::new(width, 1.0)),
            FillStyle {
                fill: Some(tokens.surface.ns_color()),
                border: Some(if hypothesis {
                    tokens.wax.ns_color()
                } else {
                    palette.line_soft()
                }),
                radius: RADIUS,
                dashed: hypothesis,
            },
        );
        let check = CheckBox::new(
            marker,
            &card.title,
            card.selected,
            card.selectable(),
            palette,
            (
                actions,
                sel!(toggleCard:),
                NSInteger::try_from(position).expect("cards are few"),
            ),
        );
        let amount = measure_label(
            marker,
            &domovoi_theme::format_size(card.amount),
            &mono_font(17.0, 600.0),
            &tokens.text.ns_color(),
        );
        let amount_size = amount.fittingSize();
        let content_x = PADDING_X + SIDE + COLUMN_GAP;
        let content_width = (width - content_x - COLUMN_GAP - amount_size.width - PADDING_X).max(0.0);
        let mut column = StackedColumn::new(content_x, content_width, PADDING_Y);
        let (title, path) = content(marker, card, &mut column, palette);
        let height = column.top + PADDING_Y;
        view.setFrameSize(NSSize::new(width, height));
        column.place_into(&view, height);
        check.setFrameOrigin(NSPoint::new(PADDING_X, height - PADDING_Y - CHECK_OFFSET - SIDE));
        view.addSubview(&check);
        amount.setFrameOrigin(NSPoint::new(
            width - PADDING_X - amount_size.width,
            height - PADDING_Y - amount_size.height,
        ));
        view.addSubview(&amount);
        Self {
            view,
            check,
            title,
            path,
            amount,
            height,
        }
    }
}

fn content(
    marker: MainThreadMarker,
    card: &CardModel,
    column: &mut StackedColumn,
    palette: &ShellPalette,
) -> (Retained<NSTextField>, Retained<NSTextField>) {
    let tokens = palette.tokens;
    let title = wrapped(
        marker,
        &card.title,
        &interface_font(15.0, 600.0),
        &tokens.text.ns_color(),
        column.width,
    );
    title.setMaximumNumberOfLines(TITLE_LINES);
    if let Some(cell) = title.cell() {
        cell.setTruncatesLastVisibleLine(true);
    }
    let joined = card.paths.join(", ");
    let path = label(
        marker,
        &joined,
        &mono_font(11.5, 400.0),
        &tokens.dim.ns_color(),
        NSLineBreakMode::ByTruncatingMiddle,
    );
    for (field, text) in [(&title, card.title.as_str()), (&path, joined.as_str())] {
        let full = NSString::from_str(text);
        field.setToolTip(Some(&full));
        field.setAccessibilityLabel(Some(&full));
    }
    let title_height = title.fittingSize().height;
    column.push(title.clone().into_super().into_super(), title_height);
    column.skip(PATH_GAP);
    let path_height = path.fittingSize().height;
    column.push(path.clone().into_super().into_super(), path_height);
    column.skip(WHY_GAP);
    let why = wrapped(
        marker,
        &card.explanation,
        &interface_font(13.5, 400.0),
        &tokens.mute.ns_color(),
        column.width,
    );
    let why_height = why.fittingSize().height;
    column.push(why.into_super().into_super(), why_height);
    let notes = [
        (card.held_line(), tokens.dim),
        (card.owner_line(), tokens.warn),
        (card.blocked.clone(), tokens.dim),
        (card.vendor_command.clone(), tokens.dim),
        (card.age.clone(), tokens.dim),
    ];
    for (text, color) in notes
        .into_iter()
        .filter_map(|(text, color)| text.map(|text| (text, color)))
    {
        let note = wrapped(
            marker,
            &text,
            &interface_font(12.5, 400.0),
            &color.ns_color(),
            column.width,
        );
        let height = note.fittingSize().height;
        column.skip(LINE_GAP);
        column.push(note.into_super().into_super(), height);
    }
    column.skip(TAGS_GAP);
    let (ink, soft) = card.level.colors(tokens);
    let badge = pill_badge(marker, card.level.title(), ink, soft);
    let tag = provenance_tag(marker, &card.provenance, palette);
    let badge_size = badge.frame().size;
    let tag_size = tag.frame().size;
    let x = column.x;
    column.push_beside(badge.into_super(), x, badge_size);
    column.push_beside(tag.into_super(), x + badge_size.width + TAG_SPACING, tag_size);
    column.skip(badge_size.height.max(tag_size.height));
    (title, path)
}

fn wrapped(marker: MainThreadMarker, text: &str, font: &NSFont, color: &NSColor, width: f64) -> Retained<NSTextField> {
    let field = label(marker, text, font, color, NSLineBreakMode::ByWordWrapping);
    field.setPreferredMaxLayoutWidth(width);
    field
}
