use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAccessibility, NSBackingStoreType, NSColor, NSLineBreakMode, NSPanel, NSTextField, NSWindow,
    NSWindowOrderingMode, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::fill_view::{FillStyle, FillView};
use crate::hover_tip::HoverTip;
use crate::label::label;
use crate::shell_palette::ShellPalette;
use crate::ui_font::{interface_font, mono_font};

const WIDTH: f64 = 320.0;
const PADDING: f64 = 12.0;
const LINE_GAP: f64 = 4.0;
const RADIUS: f64 = 8.0;
const POINTER_OFFSET: f64 = 16.0;

#[derive(Debug)]
pub struct HoverPanel {
    panel: Retained<NSPanel>,
    card: Retained<FillView>,
    pub name: Retained<NSTextField>,
    pub size: Retained<NSTextField>,
    pub path: Retained<NSTextField>,
    pub source: Retained<NSTextField>,
    pub note: Retained<NSTextField>,
}

impl HoverPanel {
    pub fn new(marker: MainThreadMarker) -> Self {
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(marker),
            NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, 1.0)),
            NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::Buffered,
            true,
        );
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(true);
        panel.setIgnoresMouseEvents(true);
        let card = FillView::new(
            marker,
            NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, 1.0)),
            FillStyle {
                fill: None,
                border: None,
                radius: RADIUS,
                dashed: false,
            },
        );
        panel.setContentView(Some(&card));
        let name = label(
            marker,
            "",
            &interface_font(13.0, 600.0),
            &NSColor::labelColor(),
            NSLineBreakMode::ByTruncatingTail,
        );
        let size = label(
            marker,
            "",
            &mono_font(12.0, 500.0),
            &NSColor::labelColor(),
            NSLineBreakMode::ByClipping,
        );
        let path = label(
            marker,
            "",
            &mono_font(11.5, 400.0),
            &NSColor::labelColor(),
            NSLineBreakMode::ByTruncatingMiddle,
        );
        let source = label(
            marker,
            "",
            &interface_font(11.5, 400.0),
            &NSColor::labelColor(),
            NSLineBreakMode::ByTruncatingTail,
        );
        let note = label(
            marker,
            "",
            &interface_font(12.0, 400.0),
            &NSColor::labelColor(),
            NSLineBreakMode::ByWordWrapping,
        );
        for field in [&name, &size, &path, &source, &note] {
            card.addSubview(field);
        }
        Self {
            panel,
            card,
            name,
            size,
            path,
            source,
            note,
        }
    }

    pub(crate) fn show(&self, tip: &HoverTip, palette: &ShellPalette, parent: &NSWindow, pointer_in_window: NSPoint) {
        let height = self.fill(tip, palette);
        let anchor = parent.convertPointToScreen(pointer_in_window);
        let frame = NSRect::new(
            NSPoint::new(anchor.x + POINTER_OFFSET, anchor.y - POINTER_OFFSET - height),
            NSSize::new(WIDTH, height),
        );
        self.panel.setFrame_display(frame, true);
        if self.panel.parentWindow().is_none() {
            unsafe { parent.addChildWindow_ordered(&self.panel, NSWindowOrderingMode::Above) };
        }
        self.panel.orderFront(None);
    }

    pub fn fill(&self, tip: &HoverTip, palette: &ShellPalette) -> f64 {
        let tokens = palette.tokens;
        self.card.restyle(FillStyle {
            fill: Some(tokens.surface.ns_color()),
            border: Some(palette.line()),
            radius: RADIUS,
            dashed: false,
        });
        let path = tip.path.as_deref().unwrap_or_default();
        let rows: [(&NSTextField, &str, _); 5] = [
            (&self.name, tip.name.as_str(), tokens.text),
            (&self.size, tip.size_text.as_str(), tokens.text),
            (&self.path, path, tokens.dim),
            (&self.source, tip.source, tokens.dim),
            (&self.note, tip.note.unwrap_or_default(), tokens.mute),
        ];
        let inner = WIDTH - 2.0 * PADDING;
        self.note.setPreferredMaxLayoutWidth(inner);
        let visible: Vec<_> = rows.iter().filter(|(_, text, _)| !text.is_empty()).collect();
        let heights: Vec<f64> = visible
            .iter()
            .map(|(field, text, color)| {
                let full = NSString::from_str(text);
                field.setStringValue(&full);
                field.setToolTip(Some(&full));
                field.setAccessibilityLabel(Some(&full));
                field.setTextColor(Some(&color.ns_color()));
                field.fittingSize().height
            })
            .collect();
        let height = 2.0 * PADDING + heights.iter().sum::<f64>() + LINE_GAP * (heights.len().saturating_sub(1)) as f64;
        let mut top = PADDING;
        for (field, _, _) in &rows {
            field.setHidden(true);
        }
        for ((field, _, _), line_height) in visible.iter().zip(&heights) {
            field.setHidden(false);
            field.setFrame(NSRect::new(
                NSPoint::new(PADDING, height - top - line_height),
                NSSize::new(inner, *line_height),
            ));
            top += line_height + LINE_GAP;
        }
        height
    }

    pub(crate) fn hide(&self) {
        if let Some(parent) = self.panel.parentWindow() {
            parent.removeChildWindow(&self.panel);
        }
        self.panel.orderOut(None);
    }
}
