use domovoi_theme::RADII;
use objc2::Message;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSLineBreakMode, NSScrollView, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::chat_entry_view::text_block;
use crate::document_swap::swap_document_keeping_offset;
use crate::fill_view::{FillStyle, FillView};
use crate::flipped_view::FlippedView;
use crate::label::{label, measure_label};
use crate::model_board::model_board;
use crate::model_card_view::model_card_view;
use crate::model_screen_model::ModelScreenModel;
use crate::probes::Probes;
use crate::section::Section;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_data::ShellData;
use crate::shell_layout::WINDOW;
use crate::shell_palette::ShellPalette;
use crate::strings::{ASSISTANT_SWITCH, LOCAL_MODEL};
use crate::toggle_switch::{SIZE as TOGGLE_SIZE, ToggleSwitch};
use crate::ui_font::interface_font;
use crate::window_frame::WindowFrame;

const PANE_TOP: f64 = 20.0;
const PANE_SIDE: f64 = 22.0;
const BLOCK_GAP: f64 = 14.0;
const SUBTITLE_GAP: f64 = 4.0;
const TOGGLE_GAP: f64 = 10.0;
const HEAD_GAP: f64 = 16.0;
const NOTE_PADDING: NSSize = NSSize::new(16.0, 12.0);
const NOTE_BAR: f64 = 2.0;
const PANE_BOTTOM: f64 = 20.0;
const PROBE_INSET: f64 = 8.0;

#[derive(Debug)]
pub struct ModelScreen {
    pub frame: WindowFrame,
    pub content: Retained<NSScrollView>,
    pub toggle: Retained<ToggleSwitch>,
    pub unload: Retained<ShellButton>,
    pub probes: Probes,
    palette: ShellPalette,
    actions: Retained<ShellActions>,
}

impl ModelScreen {
    pub fn build(marker: MainThreadMarker, data: &ShellData, palette: &ShellPalette, actions: &ShellActions) -> Self {
        let frame = WindowFrame::build(marker, data, Section::Model, palette, actions);
        let content = NSScrollView::initWithFrame(
            NSScrollView::alloc(marker),
            NSRect::new(NSPoint::ZERO, WindowFrame::pane_size()),
        );
        content.setDrawsBackground(false);
        content.setHasVerticalScroller(true);
        content.setAutohidesScrollers(true);
        content.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        frame.pane.addSubview(&content);
        let (toggle, unload) = show(&content, &data.ai.model, palette, actions);
        Self {
            frame,
            content,
            toggle,
            unload,
            probes: Probes {
                window_background: [WINDOW.width - PROBE_INSET, WINDOW.height - PROBE_INSET],
                sidebar: Some([PROBE_INSET, PROBE_INSET]),
                accent_button: None,
            },
            palette: *palette,
            actions: actions.retain(),
        }
    }

    pub fn update(&mut self, data: &ShellData) {
        self.frame.update(data, Section::Model);
        (self.toggle, self.unload) = show(&self.content, &data.ai.model, &self.palette, &self.actions);
    }
}

fn show(
    content: &NSScrollView,
    model: &ModelScreenModel,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> (Retained<ToggleSwitch>, Retained<ShellButton>) {
    let marker = MainThreadMarker::from(content);
    let size = content.frame().size;
    let width = size.width - 2.0 * PANE_SIDE;
    let document = FlippedView::new(marker, NSRect::new(NSPoint::ZERO, size));
    let mut top = PANE_TOP;
    let (head_height, toggle) = head(marker, &document, model, width, palette, actions);
    top += head_height + BLOCK_GAP;
    for (position, card) in model.cards.iter().enumerate() {
        let view = model_card_view(marker, card, position, model.fits(card), width, palette, actions);
        top = stack(&document, &view, top);
    }
    let (board, unload) = model_board(marker, model, width, palette, actions);
    top = stack(&document, &board, top);
    let note = catalog_note(marker, model, width, palette);
    top = stack(&document, &note, top) - BLOCK_GAP + PANE_BOTTOM;
    document.setFrameSize(NSSize::new(size.width, top.max(size.height)));
    document.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    swap_document_keeping_offset(content, &document);
    (toggle, unload)
}

fn stack(document: &NSView, view: &NSView, top: f64) -> f64 {
    let size = view.frame().size;
    view.setFrame(NSRect::new(NSPoint::new(PANE_SIDE, top), size));
    document.addSubview(view);
    top + size.height + BLOCK_GAP
}

fn head(
    marker: MainThreadMarker,
    document: &NSView,
    model: &ModelScreenModel,
    width: f64,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> (f64, Retained<ToggleSwitch>) {
    let tokens = palette.tokens;
    let toggle = ToggleSwitch::new(
        marker,
        ASSISTANT_SWITCH,
        model.enabled,
        palette,
        actions,
        sel!(toggleAssistant:),
    );
    let caption = measure_label(
        marker,
        ASSISTANT_SWITCH,
        &interface_font(13.0, 400.0),
        &tokens.mute.ns_color(),
    );
    let caption_size = caption.fittingSize();
    let switch_width = caption_size.width + TOGGLE_GAP + TOGGLE_SIZE.width;
    let title = label(
        marker,
        LOCAL_MODEL,
        &interface_font(17.0, 600.0),
        &tokens.text.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    let title_size = title.fittingSize();
    let line = label(
        marker,
        &model.memory_line(),
        &interface_font(12.5, 400.0),
        &tokens.dim.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    let line_size = NSSize::new(
        line.fittingSize().width.min(width - switch_width - HEAD_GAP),
        line.fittingSize().height,
    );
    let height = title_size.height + SUBTITLE_GAP + line_size.height;
    title.setFrame(NSRect::new(NSPoint::new(PANE_SIDE, PANE_TOP), title_size));
    line.setFrame(NSRect::new(
        NSPoint::new(PANE_SIDE, PANE_TOP + title_size.height + SUBTITLE_GAP),
        line_size,
    ));
    let right = PANE_SIDE + width;
    toggle.setFrame(NSRect::new(
        NSPoint::new(
            right - TOGGLE_SIZE.width,
            PANE_TOP + (height - TOGGLE_SIZE.height) / 2.0,
        ),
        TOGGLE_SIZE,
    ));
    caption.setFrame(NSRect::new(
        NSPoint::new(right - switch_width, PANE_TOP + (height - caption_size.height) / 2.0),
        caption_size,
    ));
    for view in [&*title, &*line, &*caption] {
        document.addSubview(view);
    }
    document.addSubview(&toggle);
    (height, toggle)
}

fn catalog_note(
    marker: MainThreadMarker,
    model: &ModelScreenModel,
    width: f64,
    palette: &ShellPalette,
) -> Retained<NSView> {
    let tokens = palette.tokens;
    let inner = width - 2.0 * NOTE_PADDING.width - NOTE_BAR;
    let (text, text_size) = text_block(
        marker,
        &model.catalog_note(),
        &interface_font(14.0, 400.0),
        &tokens.text.ns_color(),
        inner,
    );
    let height = (text_size.height + 2.0 * NOTE_PADDING.height).ceil();
    let note = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(width, height)),
        FillStyle {
            fill: Some(tokens.wax_soft.ns_color()),
            border: None,
            radius: f64::from(RADII.s),
            dashed: false,
        },
    );
    let bar = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(NOTE_BAR, height)),
        FillStyle {
            fill: Some(tokens.wax.ns_color()),
            border: None,
            radius: 0.0,
            dashed: false,
        },
    );
    note.addSubview(&bar);
    text.setFrame(NSRect::new(
        NSPoint::new(NOTE_BAR + NOTE_PADDING.width, NOTE_PADDING.height),
        text_size,
    ));
    note.addSubview(&text);
    note.into_super()
}
