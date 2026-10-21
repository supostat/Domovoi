use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, Message, sel};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSLineBreakMode, NSScrollView, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::availability::Availability;
use crate::button_style::ButtonStyle;
use crate::chat_entry::ChatEntry;
use crate::chat_entry_view::chat_entry_view;
use crate::chat_input::{ChatInput, INPUT_HEIGHT};
use crate::chat_model::ChatModel;
use crate::document_swap::swap_document_following_bottom;
use crate::empty_state::empty_state;
use crate::flipped_view::FlippedView;
use crate::label::label;
use crate::probes::Probes;
use crate::section::Section;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_data::ShellData;
use crate::shell_layout::WINDOW;
use crate::shell_palette::ShellPalette;
use crate::strings::{ASSISTANT_NO_MODEL, ASSISTANT_OFF, CHAT_FOOT, GO_TO_MODEL};
use crate::ui_font::{interface_font, mono_font};
use crate::window_frame::WindowFrame;

const CHAT_PADDING: f64 = 22.0;
const ENTRY_GAP: f64 = 14.0;
const BUBBLE_SHARE: f64 = 0.88;
const INPUT_TOP_GAP: f64 = 14.0;
const FOOT_GAP: f64 = 22.0;
const FOOT_BOTTOM: f64 = 16.0;
const GO_HEIGHT: f64 = 34.0;
const PROBE_INSET: f64 = 8.0;

#[derive(Debug)]
pub struct AssistantScreen {
    pub frame: WindowFrame,
    pub transcript: Option<Retained<NSScrollView>>,
    pub input: Option<ChatInput>,
    pub probes: Probes,
    palette: ShellPalette,
    actions: Retained<ShellActions>,
}

impl AssistantScreen {
    pub fn build(marker: MainThreadMarker, data: &ShellData, palette: &ShellPalette, actions: &ShellActions) -> Self {
        let chat = &data.ai.chat;
        let frame = WindowFrame::build(marker, data, Section::Assistant, palette, actions);
        let mut probes = Probes {
            window_background: [WINDOW.width - PROBE_INSET, WINDOW.height - PROBE_INSET],
            sidebar: Some([PROBE_INSET, PROBE_INSET]),
            accent_button: None,
        };
        if !chat.availability.shows_transcript() {
            probes.accent_button = Some(unavailable(marker, &frame.pane, chat.availability, palette, actions));
            return Self {
                frame,
                transcript: None,
                input: None,
                probes,
                palette: *palette,
                actions: actions.retain(),
            };
        }
        let pane = WindowFrame::pane_size();
        let foot = label(
            marker,
            CHAT_FOOT,
            &mono_font(11.5, 400.0),
            &palette.tokens.dim.ns_color(),
            NSLineBreakMode::ByTruncatingTail,
        );
        let foot_height = foot.fittingSize().height;
        foot.setFrame(NSRect::new(
            NSPoint::new(CHAT_PADDING, FOOT_BOTTOM),
            NSSize::new(pane.width - 2.0 * CHAT_PADDING, foot_height),
        ));
        foot.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMaxYMargin,
        );
        frame.pane.addSubview(&foot);
        let input_bottom = FOOT_BOTTOM + foot_height + FOOT_GAP;
        let input = ChatInput::build(
            marker,
            &frame.pane,
            NSRect::new(
                NSPoint::new(CHAT_PADDING, input_bottom),
                NSSize::new(pane.width - 2.0 * CHAT_PADDING, INPUT_HEIGHT),
            ),
            &chat.draft,
            palette,
            actions,
        );
        let transcript_bottom = input_bottom + INPUT_HEIGHT + INPUT_TOP_GAP;
        let transcript = NSScrollView::initWithFrame(
            NSScrollView::alloc(marker),
            NSRect::new(
                NSPoint::new(0.0, transcript_bottom),
                NSSize::new(pane.width, pane.height - transcript_bottom),
            ),
        );
        transcript.setDrawsBackground(false);
        transcript.setHasVerticalScroller(true);
        transcript.setAutohidesScrollers(true);
        transcript.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        frame.pane.addSubview(&transcript);
        let mut screen = Self {
            frame,
            transcript: Some(transcript),
            input: Some(input),
            probes,
            palette: *palette,
            actions: actions.retain(),
        };
        screen.show(chat);
        screen
    }

    pub fn update(&mut self, data: &ShellData) {
        self.frame.update(data, Section::Assistant);
        self.show(&data.ai.chat);
    }

    pub fn clear_draft(&self) {
        if let Some(input) = &self.input {
            input.clear();
        }
    }

    pub fn draft(&self) -> String {
        self.input.as_ref().map(ChatInput::draft).unwrap_or_default()
    }

    fn show(&mut self, chat: &ChatModel) {
        if let Some(transcript) = &self.transcript {
            let document = transcript_document(
                MainThreadMarker::from(&**transcript),
                transcript.frame().size,
                chat,
                &self.palette,
                &self.actions,
            );
            swap_document_following_bottom(transcript, &document);
        }
        if let Some(input) = &mut self.input {
            input.show(chat.availability, &self.palette, &self.actions);
        }
    }
}

fn unavailable(
    marker: MainThreadMarker,
    pane: &NSView,
    availability: Availability,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> [f64; 4] {
    let tokens = palette.tokens;
    let text = if availability == Availability::Off {
        ASSISTANT_OFF
    } else {
        ASSISTANT_NO_MODEL
    };
    let go = ShellButton::new(
        marker,
        GO_TO_MODEL,
        NSPoint::ZERO,
        GO_HEIGHT,
        ButtonStyle {
            fill: Some(tokens.wax.ns_color()),
            border: None,
            text: tokens.wax_ink.ns_color(),
            font: interface_font(13.0, 600.0),
        },
    );
    go.on_click(actions, sel!(openModelSection:));
    let size = WindowFrame::pane_size();
    let empty = empty_state(marker, size, text, Some(&go), palette);
    pane.addSubview(&empty);
    let button = go.frame();
    [
        WindowFrame::pane_origin_x() + button.origin.x,
        size.height - button.origin.y - button.size.height,
        button.size.width,
        button.size.height,
    ]
}

fn transcript_document(
    marker: MainThreadMarker,
    size: NSSize,
    chat: &ChatModel,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> Retained<FlippedView> {
    let width = size.width;
    let bubble_max = ((width - 2.0 * CHAT_PADDING) * BUBBLE_SHARE).floor();
    let document = FlippedView::new(marker, NSRect::new(NSPoint::ZERO, size));
    let mut top = CHAT_PADDING;
    for (position, entry) in chat.entries.iter().enumerate() {
        let view = chat_entry_view(marker, entry, position, bubble_max, palette, actions);
        let size = view.frame().size;
        let asked = matches!(entry, ChatEntry::Question { .. });
        let left = if asked {
            width - CHAT_PADDING - size.width
        } else {
            CHAT_PADDING
        };
        view.setFrame(NSRect::new(NSPoint::new(left, top), size));
        if asked {
            view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
        }
        document.addSubview(&view);
        top += size.height + ENTRY_GAP;
    }
    let content_height = top - ENTRY_GAP + CHAT_PADDING;
    document.setFrameSize(NSSize::new(width, content_height.max(size.height)));
    document.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    document
}
