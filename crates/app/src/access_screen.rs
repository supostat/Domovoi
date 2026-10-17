use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, MainThreadMarker, sel};
use objc2_app_kit::{NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSLineBreakMode};
use objc2_foundation::{
    NSAttributedString, NSDictionary, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString,
};

use crate::access_state::AccessState;
use crate::button_style::ButtonStyle;
use crate::field_ornament::{FieldOrnament, OrnamentInk};
use crate::fill_view::{FillStyle, FillView};
use crate::label::{label, place};
use crate::probes::Probes;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_layout::{NARROW_WINDOW, from_top};
use crate::shell_palette::ShellPalette;
use crate::strings::{
    ACCESS_BODY, ACCESS_STEPS, ACCESS_TITLE, OPEN_SETTINGS, RELAUNCH, RELAUNCH_HINT, SCAN_WITHOUT_ACCESS,
};
use crate::ui_font::{interface_font, mono_font};

const MARGIN: f64 = 40.0;
const BADGE: f64 = 24.0;
const STEP_GAP: f64 = 12.0;
const STEP_SPACING: f64 = 10.0;
const CODE_SCALE: f64 = 0.88;
const BUTTON_HEIGHT: f64 = 34.0;

#[derive(Debug)]
pub struct AccessScreen {
    pub root: Retained<FillView>,
    pub primary: Retained<ShellButton>,
    pub secondary: Option<Retained<ShellButton>>,
    pub probes: Probes,
}

impl AccessScreen {
    pub fn size(&self) -> NSSize {
        self.root.frame().size
    }
}

impl AccessScreen {
    pub fn build(
        marker: MainThreadMarker,
        access: AccessState,
        palette: &ShellPalette,
        actions: &ShellActions,
    ) -> Self {
        let tokens = palette.tokens;
        let size = NARROW_WINDOW;
        let width = size.width - 2.0 * MARGIN;
        let root = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, size),
            FillStyle {
                fill: Some(tokens.bg.ns_color()),
                border: None,
                radius: 0.0,
            },
        );
        let ornament = FieldOrnament::new(
            marker,
            from_top(size, MARGIN, MARGIN, NSSize::new(width, FieldOrnament::height())),
            OrnamentInk {
                outline: tokens.wax,
                fill: tokens.berry,
                center: tokens.text,
            },
        );
        root.addSubview(&ornament);
        let mut top = MARGIN + FieldOrnament::height() + 24.0;
        top += add_paragraph(
            &root,
            ACCESS_TITLE,
            &interface_font(22.0, 600.0),
            &tokens.text.ns_color(),
            top,
        );
        top += 12.0;
        top += add_paragraph(
            &root,
            ACCESS_BODY,
            &interface_font(14.0, 400.0),
            &tokens.mute.ns_color(),
            top,
        );
        top += 20.0;
        top = add_steps(marker, &root, top, width, palette);
        top += 14.0;
        if access == AccessState::GrantedAfterLaunch {
            top += add_paragraph(
                &root,
                RELAUNCH_HINT,
                &interface_font(13.0, 400.0),
                &tokens.mute.ns_color(),
                top,
            );
            top += 10.0;
        }
        let (primary, secondary) = action_buttons(marker, access, palette, actions);
        let primary_frame = from_top(size, MARGIN, top, primary.frame().size);
        primary.setFrame(primary_frame);
        root.addSubview(&primary);
        if let Some(ghost) = &secondary {
            ghost.setFrame(from_top(
                size,
                MARGIN + primary_frame.size.width + 10.0,
                top,
                ghost.frame().size,
            ));
            root.addSubview(ghost);
        }
        let used = top + primary_frame.size.height + MARGIN;
        fit_height(&root, used);
        let size = root.frame().size;
        let probes = Probes {
            window_background: [size.width - 6.0, size.height - 6.0],
            sidebar: None,
            accent_button: Some([MARGIN, top, primary_frame.size.width, primary_frame.size.height]),
        };
        Self {
            root,
            primary,
            secondary,
            probes,
        }
    }
}

fn add_steps(marker: MainThreadMarker, root: &FillView, mut top: f64, width: f64, palette: &ShellPalette) -> f64 {
    let tokens = palette.tokens;
    let text_left = MARGIN + BADGE + STEP_GAP;
    let text_width = width - BADGE - STEP_GAP;
    for (position, parts) in ACCESS_STEPS.iter().enumerate() {
        let badge = FillView::new(
            marker,
            from_top(NARROW_WINDOW, MARGIN, top, NSSize::new(BADGE, BADGE)),
            FillStyle {
                fill: Some(tokens.wax_soft.ns_color()),
                border: None,
                radius: BADGE / 2.0,
            },
        );
        let digit = label(
            marker,
            &format!("{}", position + 1),
            &mono_font(12.0, 600.0),
            &tokens.wax.ns_color(),
            NSLineBreakMode::ByClipping,
        );
        let digit_size = digit.fittingSize();
        digit.setFrameOrigin(NSPoint::new(
            (BADGE - digit_size.width) / 2.0,
            (BADGE - digit_size.height) / 2.0,
        ));
        badge.addSubview(&digit);
        root.addSubview(&badge);
        let text = label(
            marker,
            "",
            &interface_font(14.0, 400.0),
            &tokens.mute.ns_color(),
            NSLineBreakMode::ByWordWrapping,
        );
        text.setAttributedStringValue(&step_text(parts, palette));
        text.setPreferredMaxLayoutWidth(text_width);
        let height = text.fittingSize().height;
        place(
            &text,
            from_top(NARROW_WINDOW, text_left, top + 2.0, NSSize::new(text_width, height)).origin,
            text_width,
        );
        root.addSubview(&text);
        top += height.max(BADGE) + STEP_SPACING;
    }
    top
}

fn step_text(parts: &[&str; 3], palette: &ShellPalette) -> Retained<NSAttributedString> {
    let text = NSMutableAttributedString::new();
    let prose = palette.tokens.mute.ns_color();
    let code = palette.tokens.text.ns_color();
    let styles = [
        (interface_font(14.0, 400.0), &prose),
        (mono_font(14.0 * CODE_SCALE, 400.0), &code),
        (interface_font(14.0, 400.0), &prose),
    ];
    for (part, (font, color)) in parts.iter().zip(&styles) {
        let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
        let values: [&AnyObject; 2] = [font.as_ref(), color.as_ref()];
        let attributes = NSDictionary::from_slices(&keys, &values);
        let piece = unsafe {
            NSAttributedString::initWithString_attributes(
                NSAttributedString::alloc(),
                &NSString::from_str(part),
                Some(&attributes),
            )
        };
        text.appendAttributedString(&piece);
    }
    text.into_super()
}

fn fit_height(root: &FillView, used: f64) {
    let shift = root.frame().size.height - used;
    for child in &root.subviews() {
        let mut frame = child.frame();
        frame.origin.y -= shift;
        child.setFrame(frame);
    }
    root.setFrameSize(NSSize::new(root.frame().size.width, used));
}

fn add_paragraph(root: &FillView, text: &str, font: &NSFont, color: &NSColor, top: f64) -> f64 {
    let width = NARROW_WINDOW.width - 2.0 * MARGIN;
    let paragraph = label(
        MainThreadMarker::from(root),
        text,
        font,
        color,
        NSLineBreakMode::ByWordWrapping,
    );
    paragraph.setPreferredMaxLayoutWidth(width);
    let height = paragraph.fittingSize().height;
    place(
        &paragraph,
        from_top(NARROW_WINDOW, MARGIN, top, NSSize::new(width, height)).origin,
        width,
    );
    root.addSubview(&paragraph);
    height
}

fn action_buttons(
    marker: MainThreadMarker,
    access: AccessState,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> (Retained<ShellButton>, Option<Retained<ShellButton>>) {
    let tokens = palette.tokens;
    let primary_style = ButtonStyle {
        fill: Some(tokens.wax.ns_color()),
        border: None,
        text: tokens.wax_ink.ns_color(),
        font: interface_font(14.0, 600.0),
    };
    if access == AccessState::GrantedAfterLaunch {
        let primary = ShellButton::new(marker, RELAUNCH, NSPoint::ZERO, BUTTON_HEIGHT, primary_style);
        primary.on_click(actions, sel!(relaunch:));
        return (primary, None);
    }
    let primary = ShellButton::new(marker, OPEN_SETTINGS, NSPoint::ZERO, BUTTON_HEIGHT, primary_style);
    let ghost = ShellButton::new(
        marker,
        SCAN_WITHOUT_ACCESS,
        NSPoint::ZERO,
        BUTTON_HEIGHT,
        ButtonStyle {
            fill: None,
            border: Some(palette.line()),
            text: tokens.text.ns_color(),
            font: interface_font(14.0, 500.0),
        },
    );
    primary.on_click(actions, sel!(openSettings:));
    ghost.on_click(actions, sel!(scanWithoutAccess:));
    (primary, Some(ghost))
}
