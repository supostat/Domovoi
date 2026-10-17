use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSAutoresizingMaskOptions, NSLineBreakMode, NSTextField};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::fill_view::{FillStyle, FillView};
use crate::label::{measure_label, name_label};
use crate::shell_palette::ShellPalette;
use crate::sidebar_entry::SidebarEntry;
use crate::ui_font::{interface_font, mono_font};

const INSET: f64 = 10.0;
const GAP: f64 = 8.0;
const SELECTION_RADIUS: f64 = 7.0;

#[derive(Debug)]
pub struct SidebarItemView {
    pub view: Retained<FillView>,
    pub name: Retained<NSTextField>,
    pub counter: Option<Retained<NSTextField>>,
}

impl SidebarItemView {
    pub fn build(
        marker: MainThreadMarker,
        entry: &SidebarEntry,
        width: f64,
        height: f64,
        palette: &ShellPalette,
    ) -> Self {
        let tokens = palette.tokens;
        let view = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, NSSize::new(width, height)),
            FillStyle {
                fill: entry.selected.then(|| tokens.surface_2.ns_color()),
                border: None,
                radius: SELECTION_RADIUS,
                dashed: false,
            },
        );
        let counter = entry.counter.as_ref().map(|counter| {
            let field = measure_label(marker, counter, &mono_font(11.5, 400.0), &tokens.dim.ns_color());
            let size = field.fittingSize();
            field.setFrame(NSRect::new(
                NSPoint::new(width - INSET - size.width, (height - size.height) / 2.0),
                size,
            ));
            field.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
            view.addSubview(&field);
            field
        });
        let available = width - 2.0 * INSET - counter.as_ref().map_or(0.0, |field| field.frame().size.width + GAP);
        let name = name_label(
            marker,
            &entry.title,
            &interface_font(13.5, 500.0),
            &if entry.selected { tokens.text } else { tokens.mute }.ns_color(),
            NSLineBreakMode::ByTruncatingTail,
        );
        let name_height = name.fittingSize().height;
        name.setFrame(NSRect::new(
            NSPoint::new(INSET, (height - name_height) / 2.0),
            NSSize::new(available.max(0.0), name_height),
        ));
        name.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        view.addSubview(&name);
        Self { view, name, counter }
    }
}
