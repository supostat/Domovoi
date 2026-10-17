use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly, Message, sel};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSLineBreakMode, NSScrollView, NSTextView, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::availability::Availability;
use crate::button_style::ButtonStyle;
use crate::chat_entry::ChatEntry;
use crate::chat_entry_view::chat_entry_view;
use crate::chat_input_delegate::ChatInputDelegate;
use crate::chat_model::ChatModel;
use crate::empty_state::empty_state;
use crate::fill_view::{FillStyle, FillView};
use crate::flipped_view::FlippedView;
use crate::label::label;
use crate::probes::Probes;
use crate::section::Section;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_data::ShellData;
use crate::shell_layout::WINDOW;
use crate::shell_palette::ShellPalette;
use crate::strings::{
    ANSWERING, ASK_PLACEHOLDER, ASK_PLACEHOLDER_UNLOADED, ASSISTANT_NO_MODEL, ASSISTANT_OFF, CHAT_FOOT, GO_TO_MODEL,
    MODEL_LOADING, SEND_KEY, STOP_ANSWER,
};
use crate::ui_font::{interface_font, mono_font};
use crate::window_frame::WindowFrame;

const CHAT_PADDING: f64 = 22.0;
const ENTRY_GAP: f64 = 14.0;
const BUBBLE_SHARE: f64 = 0.88;
const INPUT_HEIGHT: f64 = 40.0;
const INPUT_RADIUS: f64 = 10.0;
const INPUT_PADDING: f64 = 12.0;
const INPUT_LINE: f64 = 20.0;
const INPUT_TOP_GAP: f64 = 14.0;
const TRAILING_GAP: f64 = 10.0;
const FOOT_GAP: f64 = 22.0;
const FOOT_BOTTOM: f64 = 16.0;
const STOP_HEIGHT: f64 = 26.0;
const GO_HEIGHT: f64 = 34.0;
const PROBE_INSET: f64 = 8.0;

#[derive(Debug)]
pub struct AssistantScreen {
    pub frame: WindowFrame,
    pub transcript: Option<Retained<NSScrollView>>,
    pub input: Option<Retained<NSTextView>>,
    _input_delegate: Option<Retained<ChatInputDelegate>>,
    pub probes: Probes,
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
                _input_delegate: None,
                probes,
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
        let (input, input_delegate) = input_box(marker, &frame.pane, input_bottom, chat, palette, actions);
        let transcript_bottom = input_bottom + INPUT_HEIGHT + INPUT_TOP_GAP;
        let transcript = transcript(
            marker,
            NSRect::new(
                NSPoint::new(0.0, transcript_bottom),
                NSSize::new(pane.width, pane.height - transcript_bottom),
            ),
            chat,
            palette,
            actions,
        );
        frame.pane.addSubview(&transcript);
        Self {
            frame,
            transcript: Some(transcript),
            input: Some(input),
            _input_delegate: Some(input_delegate),
            probes,
        }
    }

    pub fn clear_draft(&self) {
        if let Some(input) = &self.input {
            input.setString(&NSString::new());
        }
    }

    pub fn draft(&self) -> String {
        self.input
            .as_ref()
            .map(|input| input.string().to_string())
            .unwrap_or_default()
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

fn input_box(
    marker: MainThreadMarker,
    pane: &NSView,
    bottom: f64,
    chat: &ChatModel,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> (Retained<NSTextView>, Retained<ChatInputDelegate>) {
    let tokens = palette.tokens;
    let width = WindowFrame::pane_size().width - 2.0 * CHAT_PADDING;
    let field = FillView::new(
        marker,
        NSRect::new(NSPoint::new(CHAT_PADDING, bottom), NSSize::new(width, INPUT_HEIGHT)),
        FillStyle {
            fill: Some(tokens.bg_2.ns_color()),
            border: Some(palette.line()),
            radius: INPUT_RADIUS,
            dashed: false,
        },
    );
    field.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMaxYMargin);
    pane.addSubview(&field);
    let trailing = trailing_control(marker, chat.availability, palette, actions);
    let trailing_size = trailing.frame().size;
    trailing.setFrameOrigin(NSPoint::new(
        width - INPUT_PADDING - trailing_size.width,
        (INPUT_HEIGHT - trailing_size.height) / 2.0,
    ));
    trailing.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
    field.addSubview(&trailing);
    let text_frame = NSRect::new(
        NSPoint::new(INPUT_PADDING, (INPUT_HEIGHT - INPUT_LINE) / 2.0),
        NSSize::new(
            width - 2.0 * INPUT_PADDING - trailing_size.width - TRAILING_GAP,
            INPUT_LINE,
        ),
    );
    let placeholder = label(
        marker,
        match chat.availability {
            Availability::Loading => MODEL_LOADING,
            Availability::Answering => ANSWERING,
            Availability::Unloaded => ASK_PLACEHOLDER_UNLOADED,
            Availability::Off | Availability::NoModel | Availability::Ready => ASK_PLACEHOLDER,
        },
        &interface_font(14.0, 400.0),
        &tokens.dim.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    let placeholder_height = placeholder.fittingSize().height;
    placeholder.setFrame(NSRect::new(
        NSPoint::new(
            text_frame.origin.x,
            text_frame.origin.y + (INPUT_LINE - placeholder_height) / 2.0,
        ),
        NSSize::new(text_frame.size.width, placeholder_height),
    ));
    placeholder.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    placeholder.setHidden(!chat.draft.is_empty());
    field.addSubview(&placeholder);
    let scroll = NSTextView::scrollableTextView(marker);
    scroll.setFrame(text_frame);
    scroll.setDrawsBackground(false);
    scroll.setHasVerticalScroller(false);
    scroll.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    let input = scroll
        .documentView()
        .and_then(|document| document.downcast::<NSTextView>().ok())
        .expect("a scrollable text view hosts a text view");
    input.setFont(Some(&interface_font(14.0, 400.0)));
    input.setTextColor(Some(&tokens.text.ns_color()));
    input.setInsertionPointColor(Some(&tokens.text.ns_color()));
    input.setDrawsBackground(false);
    input.setRichText(false);
    input.setTextContainerInset(NSSize::new(0.0, 0.0));
    if let Some(container) = unsafe { input.textContainer() } {
        container.setLineFragmentPadding(0.0);
    }
    input.setString(&NSString::from_str(&chat.draft));
    input.setEditable(chat.availability.accepts_questions());
    input.setSelectable(chat.availability.accepts_questions());
    field.addSubview(&scroll);
    let delegate = ChatInputDelegate::new(marker, actions.retain(), input.clone(), placeholder);
    input.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    (input, delegate)
}

fn trailing_control(
    marker: MainThreadMarker,
    availability: Availability,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> Retained<NSView> {
    let tokens = palette.tokens;
    if availability == Availability::Answering {
        let stop = ShellButton::new(
            marker,
            STOP_ANSWER,
            NSPoint::ZERO,
            STOP_HEIGHT,
            ButtonStyle {
                fill: Some(tokens.surface_2.ns_color()),
                border: Some(palette.line()),
                text: tokens.text.ns_color(),
                font: interface_font(12.0, 600.0),
            },
        );
        stop.on_click(actions, sel!(stopAnswer:));
        return stop.into_super().into_super().into_super();
    }
    let key = label(
        marker,
        SEND_KEY,
        &mono_font(11.5, 400.0),
        &tokens.dim.ns_color(),
        NSLineBreakMode::ByClipping,
    );
    key.setFrameSize(key.fittingSize());
    key.into_super().into_super()
}

fn transcript(
    marker: MainThreadMarker,
    frame: NSRect,
    chat: &ChatModel,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> Retained<NSScrollView> {
    let width = frame.size.width;
    let bubble_max = ((width - 2.0 * CHAT_PADDING) * BUBBLE_SHARE).floor();
    let document = FlippedView::new(marker, NSRect::new(NSPoint::ZERO, frame.size));
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
    document.setFrameSize(NSSize::new(width, content_height.max(frame.size.height)));
    document.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(marker), frame);
    scroll.setDrawsBackground(false);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    scroll.setDocumentView(Some(&document));
    if content_height > frame.size.height {
        document.scrollPoint(NSPoint::new(0.0, content_height - frame.size.height));
    }
    scroll
}
