use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, Message, sel};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSLineBreakMode, NSScrollView, NSTextField, NSTextView, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::availability::Availability;
use crate::button_style::ButtonStyle;
use crate::chat_input_delegate::ChatInputDelegate;
use crate::fill_view::{FillStyle, FillView};
use crate::label::label;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_palette::ShellPalette;
use crate::strings::{ANSWERING, ASK_PLACEHOLDER, ASK_PLACEHOLDER_UNLOADED, MODEL_LOADING, SEND_KEY, STOP_ANSWER};
use crate::ui_font::{interface_font, mono_font};

pub(crate) const INPUT_HEIGHT: f64 = 40.0;
const INPUT_RADIUS: f64 = 10.0;
const INPUT_PADDING: f64 = 12.0;
const INPUT_LINE: f64 = 20.0;
const TRAILING_GAP: f64 = 10.0;
const STOP_HEIGHT: f64 = 26.0;

#[derive(Debug)]
pub struct ChatInput {
    pub text: Retained<NSTextView>,
    pub trailing: Retained<NSView>,
    field: Retained<FillView>,
    scroll: Retained<NSScrollView>,
    placeholder: Retained<NSTextField>,
    availability: Option<Availability>,
    _delegate: Retained<ChatInputDelegate>,
}

impl ChatInput {
    pub(crate) fn build(
        marker: MainThreadMarker,
        pane: &NSView,
        frame: NSRect,
        draft: &str,
        palette: &ShellPalette,
        actions: &ShellActions,
    ) -> Self {
        let tokens = palette.tokens;
        let field = FillView::new(
            marker,
            frame,
            FillStyle {
                fill: Some(tokens.bg_2.ns_color()),
                border: Some(palette.line()),
                radius: INPUT_RADIUS,
                dashed: false,
            },
        );
        field.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMaxYMargin,
        );
        pane.addSubview(&field);
        let placeholder = label(
            marker,
            "",
            &interface_font(14.0, 400.0),
            &tokens.dim.ns_color(),
            NSLineBreakMode::ByTruncatingTail,
        );
        placeholder.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        field.addSubview(&placeholder);
        let scroll = NSTextView::scrollableTextView(marker);
        scroll.setDrawsBackground(false);
        scroll.setHasVerticalScroller(false);
        scroll.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        let text = scroll
            .documentView()
            .and_then(|document| document.downcast::<NSTextView>().ok())
            .expect("a scrollable text view hosts a text view");
        text.setFont(Some(&interface_font(14.0, 400.0)));
        text.setTextColor(Some(&tokens.text.ns_color()));
        text.setInsertionPointColor(Some(&tokens.text.ns_color()));
        text.setDrawsBackground(false);
        text.setRichText(false);
        text.setTextContainerInset(NSSize::new(0.0, 0.0));
        if let Some(container) = unsafe { text.textContainer() } {
            container.setLineFragmentPadding(0.0);
        }
        text.setString(&NSString::from_str(draft));
        field.addSubview(&scroll);
        let delegate = ChatInputDelegate::new(marker, actions.retain(), text.clone(), placeholder.clone());
        text.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        let trailing = NSView::new(marker);
        field.addSubview(&trailing);
        Self {
            text,
            trailing,
            field,
            scroll,
            placeholder,
            availability: None,
            _delegate: delegate,
        }
    }

    pub(crate) fn show(&mut self, availability: Availability, palette: &ShellPalette, actions: &ShellActions) {
        self.placeholder.setHidden(self.text.string().length() > 0);
        if self.availability == Some(availability) {
            return;
        }
        self.availability = Some(availability);
        self.trailing.removeFromSuperview();
        self.trailing = trailing_control(MainThreadMarker::from(&*self.text), availability, palette, actions);
        let width = self.field.frame().size.width;
        let trailing_size = self.trailing.frame().size;
        self.trailing.setFrameOrigin(NSPoint::new(
            width - INPUT_PADDING - trailing_size.width,
            (INPUT_HEIGHT - trailing_size.height) / 2.0,
        ));
        self.trailing
            .setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
        self.field.addSubview(&self.trailing);
        let text_frame = NSRect::new(
            NSPoint::new(INPUT_PADDING, (INPUT_HEIGHT - INPUT_LINE) / 2.0),
            NSSize::new(
                width - 2.0 * INPUT_PADDING - trailing_size.width - TRAILING_GAP,
                INPUT_LINE,
            ),
        );
        self.scroll.setFrame(text_frame);
        self.placeholder
            .setStringValue(&NSString::from_str(placeholder_text(availability)));
        let placeholder_height = self.placeholder.fittingSize().height;
        self.placeholder.setFrame(NSRect::new(
            NSPoint::new(
                text_frame.origin.x,
                text_frame.origin.y + (INPUT_LINE - placeholder_height) / 2.0,
            ),
            NSSize::new(text_frame.size.width, placeholder_height),
        ));
        self.text.setEditable(availability.accepts_questions());
        self.text.setSelectable(availability.accepts_questions());
    }

    pub(crate) fn draft(&self) -> String {
        self.text.string().to_string()
    }

    pub(crate) fn clear(&self) {
        self.text.setString(&NSString::new());
    }
}

fn placeholder_text(availability: Availability) -> &'static str {
    match availability {
        Availability::Loading => MODEL_LOADING,
        Availability::Answering => ANSWERING,
        Availability::Unloaded => ASK_PLACEHOLDER_UNLOADED,
        Availability::Off | Availability::NoModel | Availability::Ready => ASK_PLACEHOLDER,
    }
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
