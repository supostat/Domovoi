use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2_app_kit::{NSColor, NSFont, NSLineBreakMode, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::button_style::ButtonStyle;
use crate::field_ornament::{FieldOrnament, OrnamentInk};
use crate::fill_view::FillView;
use crate::label::{label, measure_label};
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_layout::NARROW_WINDOW;
use crate::shell_palette::ShellPalette;
use crate::tray_button::{ESCAPE, TrayButton};
use crate::ui_font::{interface_font, mono_font};

pub(crate) const MARGIN: f64 = 28.0;
pub(crate) const GAP: f64 = 12.0;
pub(crate) const SECTION_GAP: f64 = 22.0;
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

    pub(crate) fn paragraph(&mut self, text: &str, font: &NSFont, color: &NSColor) -> Retained<NSTextField> {
        let field = label(self.marker, text, font, color, NSLineBreakMode::ByWordWrapping);
        field.setPreferredMaxLayoutWidth(self.width);
        let height = field.fittingSize().height;
        self.add(
            field.clone().into_super().into_super(),
            MARGIN,
            NSSize::new(self.width, height),
        );
        self.top += height;
        field
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

    pub(crate) fn figure(&mut self, caption: &str, figure: &str) {
        let tokens = self.palette.tokens;
        let caption = measure_label(
            self.marker,
            caption,
            &interface_font(13.5, 400.0),
            &tokens.mute.ns_color(),
        );
        let figure = measure_label(self.marker, figure, &mono_font(26.0, 600.0), &tokens.wax.ns_color());
        let caption_size = caption.fittingSize();
        let figure_size = figure.fittingSize();
        let line = figure_size.height.max(caption_size.height);
        self.placed.push((
            caption.into_super().into_super(),
            MARGIN,
            self.top + line - caption_size.height,
            caption_size,
        ));
        self.add(
            figure.into_super().into_super(),
            MARGIN + self.width - figure_size.width,
            figure_size,
        );
        self.top += line;
    }

    pub(crate) fn ornament(&mut self) -> Retained<FieldOrnament> {
        let tokens = self.palette.tokens;
        let stitched = FieldOrnament::new(
            self.marker,
            NSRect::new(NSPoint::ZERO, NSSize::new(self.width, FieldOrnament::height())),
            OrnamentInk {
                outline: tokens.wax,
                fill: tokens.berry,
                center: tokens.text,
            },
        );
        self.add(
            stitched.clone().into_super(),
            MARGIN,
            NSSize::new(self.width, FieldOrnament::height()),
        );
        self.top += FieldOrnament::height() + SECTION_GAP;
        stitched
    }

    pub(crate) fn buttons(&mut self, buttons: &[(&str, Sel, TrayButton)]) -> Retained<ShellButton> {
        let tokens = self.palette.tokens;
        let mut right = MARGIN + self.width;
        let mut primary = None;
        for (title, selector, role) in buttons.iter().rev() {
            let is_primary = *role == TrayButton::Primary;
            let style = if is_primary {
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
            if *role == TrayButton::Cancel {
                button.setKeyEquivalent(&NSString::from_str(ESCAPE));
            }
            let size = button.frame().size;
            right -= size.width;
            self.add(button.clone().into_super().into_super().into_super(), right, size);
            if is_primary {
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
