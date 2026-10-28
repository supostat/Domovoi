use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2_app_kit::{NSColor, NSFont, NSLineBreakMode, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::button_style::ButtonStyle;
use crate::fill_view::FillView;
use crate::label::label;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_layout::NARROW_WINDOW;
use crate::shell_palette::ShellPalette;
use crate::ui_font::interface_font;

pub(crate) const MARGIN: f64 = 28.0;
const BUTTON_HEIGHT: f64 = 34.0;
const BUTTON_GAP: f64 = 10.0;

pub(crate) struct TrayColumn<'a> {
    pub(crate) marker: MainThreadMarker,
    pub(crate) root: &'a FillView,
    pub(crate) palette: &'a ShellPalette,
    pub(crate) actions: &'a ShellActions,
    pub(crate) width: f64,
    pub(crate) top: f64,
    pub(crate) placed: Vec<(Retained<NSView>, f64, f64, NSSize)>,
    pub(crate) primary: Option<[f64; 4]>,
}

impl TrayColumn<'_> {
    pub(crate) fn add(&mut self, view: Retained<NSView>, x: f64, size: NSSize) {
        self.placed.push((view, x, self.top, size));
    }

    pub(crate) fn paragraph(&mut self, text: &str, font: &NSFont, color: &NSColor) {
        let field = label(self.marker, text, font, color, NSLineBreakMode::ByWordWrapping);
        field.setPreferredMaxLayoutWidth(self.width);
        let height = field.fittingSize().height;
        self.add(field.into_super().into_super(), MARGIN, NSSize::new(self.width, height));
        self.top += height;
    }

    pub(crate) fn indented_line(
        &mut self,
        text: &str,
        (font, color): (&NSFont, &NSColor),
        indent: f64,
    ) -> Retained<NSTextField> {
        let field = label(self.marker, text, font, color, NSLineBreakMode::ByWordWrapping);
        field.setPreferredMaxLayoutWidth(self.width - indent);
        let height = field.fittingSize().height;
        self.add(
            field.clone().into_super().into_super(),
            MARGIN + indent,
            NSSize::new(self.width - indent, height),
        );
        self.top += height;
        field
    }

    pub(crate) fn buttons(&mut self, buttons: &[(&str, Sel, bool)]) -> Retained<ShellButton> {
        let tokens = self.palette.tokens;
        let mut right = MARGIN + self.width;
        let mut primary = None;
        for (title, selector, is_primary) in buttons.iter().rev() {
            let style = if *is_primary {
                ButtonStyle {
                    fill: Some(tokens.wax.ns_color()),
                    border: None,
                    text: tokens.wax_ink.ns_color(),
                    font: interface_font(13.0, 600.0),
                }
            } else {
                ButtonStyle {
                    fill: None,
                    border: None,
                    text: tokens.mute.ns_color(),
                    font: interface_font(13.0, 600.0),
                }
            };
            let button = ShellButton::new(self.marker, title, NSPoint::ZERO, BUTTON_HEIGHT, style);
            button.on_click(self.actions, *selector);
            let size = button.frame().size;
            right -= size.width;
            self.add(button.clone().into_super().into_super().into_super(), right, size);
            if *is_primary {
                self.primary = Some([right, self.top, size.width, size.height]);
                primary = Some(button);
            }
            right -= BUTTON_GAP;
        }
        self.top += BUTTON_HEIGHT;
        primary.expect("a button row has a primary button")
    }

    pub(crate) fn finish(self) -> f64 {
        let height = self.top + MARGIN;
        self.root.setFrameSize(NSSize::new(NARROW_WINDOW.width, height));
        for (view, x, top, size) in self.placed {
            view.setFrame(NSRect::new(NSPoint::new(x, height - top - size.height), size));
            self.root.addSubview(&view);
        }
        height
    }
}
