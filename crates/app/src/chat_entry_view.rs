use domovoi_theme::{format_count, format_size};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSColor, NSFont, NSLineBreakMode, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSSize};

use crate::answer_item::AnswerItem;
use crate::assembled_view::{Parts, assemble};
use crate::button_style::ButtonStyle;
use crate::chat_entry::ChatEntry;
use crate::fill_view::FillStyle;
use crate::label::label;
use crate::provenance::Provenance;
use crate::provenance_tag::provenance_tag;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_palette::ShellPalette;
use crate::strings::{ADD_TO_TRAY, NODES};
use crate::ui_font::{interface_font, mono_font};

const BUBBLE_PADDING: NSSize = NSSize::new(14.0, 11.0);
const BUBBLE_RADIUS: f64 = 12.0;
const TOOL_PADDING: NSSize = NSSize::new(12.0, 8.0);
const TOOL_RADIUS: f64 = 8.0;
const TOOL_GAP: f64 = 8.0;
const LIST_TOP: f64 = 8.0;
const LIST_INDENT: f64 = 18.0;
const BULLET_INDENT: f64 = 5.0;
const ROW_GAP: f64 = 2.0;
const ACTION_TOP: f64 = 12.0;
const ACTION_HEIGHT: f64 = 28.0;
const TAG_GAP: f64 = 8.0;
const PATH_GAP: f64 = 4.0;
const BULLET: &str = "•";
const ITEM_DASH: &str = " — ";
const FOUND_ARROW: &str = "→";

pub(crate) fn chat_entry_view(
    marker: MainThreadMarker,
    entry: &ChatEntry,
    position: usize,
    max_width: f64,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> Retained<NSView> {
    let tokens = palette.tokens;
    let inner = max_width - 2.0 * BUBBLE_PADDING.width;
    let speech = interface_font(14.0, 400.0);
    let assistant_style = FillStyle {
        fill: Some(tokens.surface_2.ns_color()),
        border: Some(palette.line_soft()),
        radius: BUBBLE_RADIUS,
        dashed: false,
    };
    match entry {
        ChatEntry::Question { text } => {
            let (field, size) = text_block(marker, text, &speech, &tokens.text.ns_color(), inner);
            let style = FillStyle {
                fill: Some(tokens.wax_soft.ns_color()),
                border: Some(palette.line_soft()),
                radius: BUBBLE_RADIUS,
                dashed: false,
            };
            assemble(
                marker,
                vec![(field.into_super().into_super(), NSPoint::ZERO, size)],
                BUBBLE_PADDING,
                style,
            )
        }
        ChatEntry::ToolCall {
            tool, arguments, found, ..
        } => tool_call(
            marker,
            tool,
            arguments,
            *found,
            max_width - 2.0 * TOOL_PADDING.width,
            palette,
        ),
        ChatEntry::Answer { text, items, .. } => {
            let parts = answer_parts(
                marker,
                (text, items, entry.answer_total()),
                position,
                inner,
                palette,
                actions,
            );
            assemble(marker, parts, BUBBLE_PADDING, assistant_style)
        }
        ChatEntry::Hypothesis { path, text } => {
            let tag = provenance_tag(marker, &Provenance::Hypothesis, palette);
            let tag_size = tag.frame().size;
            let path_field = label(
                marker,
                path,
                &mono_font(12.0, 400.0),
                &tokens.dim.ns_color(),
                NSLineBreakMode::ByTruncatingMiddle,
            );
            let path_size = NSSize::new(
                path_field.fittingSize().width.min(inner).ceil(),
                path_field.fittingSize().height.ceil(),
            );
            let (field, size) = text_block(marker, text, &speech, &tokens.text.ns_color(), inner);
            let path_top = tag_size.height + TAG_GAP;
            let text_top = path_top + path_size.height + PATH_GAP;
            let parts = vec![
                (tag.into_super(), NSPoint::ZERO, tag_size),
                (
                    path_field.into_super().into_super(),
                    NSPoint::new(0.0, path_top),
                    path_size,
                ),
                (field.into_super().into_super(), NSPoint::new(0.0, text_top), size),
            ];
            assemble(marker, parts, BUBBLE_PADDING, assistant_style)
        }
        ChatEntry::Failure { text } => {
            let (field, size) = text_block(marker, text, &speech, &tokens.mute.ns_color(), inner);
            assemble(
                marker,
                vec![(field.into_super().into_super(), NSPoint::ZERO, size)],
                BUBBLE_PADDING,
                assistant_style,
            )
        }
    }
}

fn answer_parts(
    marker: MainThreadMarker,
    (text, items, total): (&str, &[AnswerItem], u64),
    position: usize,
    inner: f64,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> Parts {
    let tokens = palette.tokens;
    let mut parts = Parts::new();
    let (field, size) = text_block(
        marker,
        text,
        &interface_font(14.0, 400.0),
        &tokens.text.ns_color(),
        inner,
    );
    parts.push((field.into_super().into_super(), NSPoint::ZERO, size));
    let mut top = size.height;
    if !items.is_empty() {
        top = answer_items(marker, items, top + LIST_TOP, inner, palette, &mut parts);
    }
    if total > 0 {
        let button = ShellButton::new(
            marker,
            &format!("{ADD_TO_TRAY} · {}", format_size(total)),
            NSPoint::ZERO,
            ACTION_HEIGHT,
            ButtonStyle {
                fill: Some(tokens.wax.ns_color()),
                border: None,
                text: tokens.wax_ink.ns_color(),
                font: interface_font(12.0, 600.0),
            },
        );
        button.setTag(isize::try_from(position).expect("a transcript fits in isize"));
        button.on_click(actions, sel!(addAnswerToTray:));
        let size = button.frame().size;
        parts.push((
            button.into_super().into_super().into_super(),
            NSPoint::new(0.0, top + ACTION_TOP),
            size,
        ));
    }
    parts
}

fn tool_call(
    marker: MainThreadMarker,
    tool: &str,
    arguments: &str,
    found: Option<u64>,
    inner: f64,
    palette: &ShellPalette,
) -> Retained<NSView> {
    let tokens = palette.tokens;
    let font = mono_font(12.0, 400.0);
    let name = label(
        marker,
        tool,
        &mono_font(12.0, 500.0),
        &tokens.mute.ns_color(),
        NSLineBreakMode::ByClipping,
    );
    let name_size = name.fittingSize();
    let found = found.map(|count| {
        label(
            marker,
            &format!("{FOUND_ARROW} {}", format_count(count, &NODES)),
            &font,
            &tokens.dim.ns_color(),
            NSLineBreakMode::ByClipping,
        )
    });
    let found_width = found.as_ref().map_or(0.0, |field| field.fittingSize().width + TOOL_GAP);
    let room = (inner - name_size.width - TOOL_GAP - found_width).max(0.0);
    let (arguments_field, arguments_size) = text_block(marker, arguments, &font, &tokens.dim.ns_color(), room);
    let mut parts: Parts = vec![
        (name.into_super().into_super(), NSPoint::ZERO, name_size),
        (
            arguments_field.into_super().into_super(),
            NSPoint::new(name_size.width + TOOL_GAP, 0.0),
            arguments_size,
        ),
    ];
    if let Some(found) = found {
        let size = found.fittingSize();
        let left = name_size.width + TOOL_GAP + arguments_size.width + TOOL_GAP;
        parts.push((found.into_super().into_super(), NSPoint::new(left, 0.0), size));
    }
    assemble(
        marker,
        parts,
        TOOL_PADDING,
        FillStyle {
            fill: None,
            border: Some(palette.line()),
            radius: TOOL_RADIUS,
            dashed: true,
        },
    )
}

fn answer_items(
    marker: MainThreadMarker,
    items: &[AnswerItem],
    mut top: f64,
    inner: f64,
    palette: &ShellPalette,
    parts: &mut Parts,
) -> f64 {
    let tokens = palette.tokens;
    let speech = interface_font(14.0, 400.0);
    for item in items {
        let size_field = label(
            marker,
            &format_size(item.bytes),
            &mono_font(13.5, 500.0),
            &tokens.text.ns_color(),
            NSLineBreakMode::ByClipping,
        );
        let size_size = size_field.fittingSize();
        let title = label(
            marker,
            &format!("{}{ITEM_DASH}", item.title),
            &speech,
            &tokens.mute.ns_color(),
            NSLineBreakMode::ByTruncatingTail,
        );
        let title_natural = title.fittingSize();
        let title_size = NSSize::new(
            title_natural.width.min(inner - LIST_INDENT - size_size.width).ceil(),
            title_natural.height,
        );
        let bullet = label(
            marker,
            BULLET,
            &speech,
            &tokens.mute.ns_color(),
            NSLineBreakMode::ByClipping,
        );
        let bullet_size = bullet.fittingSize();
        let row_height = title_size.height.max(size_size.height);
        parts.push((
            bullet.into_super().into_super(),
            NSPoint::new(BULLET_INDENT, top),
            bullet_size,
        ));
        parts.push((
            title.into_super().into_super(),
            NSPoint::new(LIST_INDENT, top),
            title_size,
        ));
        parts.push((
            size_field.into_super().into_super(),
            NSPoint::new(
                LIST_INDENT + title_size.width,
                top + (row_height - size_size.height) / 2.0,
            ),
            size_size,
        ));
        top += row_height + ROW_GAP;
    }
    top - ROW_GAP
}

pub(crate) fn text_block(
    marker: MainThreadMarker,
    text: &str,
    font: &NSFont,
    color: &NSColor,
    max_width: f64,
) -> (Retained<NSTextField>, NSSize) {
    let field = label(marker, text, font, color, NSLineBreakMode::ByWordWrapping);
    let natural = field.fittingSize();
    if natural.width <= max_width {
        return (field, NSSize::new(natural.width.ceil(), natural.height.ceil()));
    }
    field.setPreferredMaxLayoutWidth(max_width);
    let height = field.fittingSize().height;
    (field, NSSize::new(max_width.ceil(), height.ceil()))
}
