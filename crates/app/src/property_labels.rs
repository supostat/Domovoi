use domovoi_theme::Palette;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSLineBreakMode, NSTextField, NSView};

use crate::fact_rows::FactRows;
use crate::index_facts::IndexFacts;
use crate::label::label;
use crate::node_facts::NodeFacts;
use crate::ui_font::{interface_font, mono_font};

#[derive(Debug)]
pub(crate) struct PropertyLabels {
    name: Retained<NSTextField>,
    size: Retained<NSTextField>,
    path: Retained<NSTextField>,
    source: Retained<NSTextField>,
    created: Retained<NSTextField>,
    modified: Retained<NSTextField>,
    inside: Retained<NSTextField>,
    note: Retained<NSTextField>,
}

impl PropertyLabels {
    pub(crate) fn new(marker: MainThreadMarker, width: f64) -> Self {
        let color = NSColor::labelColor();
        let plain = |size: f64, weight: f64, mode: NSLineBreakMode| {
            label(marker, "", &interface_font(size, weight), &color, mode)
        };
        let note = plain(12.0, 400.0, NSLineBreakMode::ByWordWrapping);
        note.setPreferredMaxLayoutWidth(width);
        Self {
            name: plain(13.0, 600.0, NSLineBreakMode::ByTruncatingTail),
            size: label(marker, "", &mono_font(12.0, 500.0), &color, NSLineBreakMode::ByClipping),
            path: label(
                marker,
                "",
                &mono_font(11.5, 400.0),
                &color,
                NSLineBreakMode::ByTruncatingMiddle,
            ),
            source: plain(11.5, 400.0, NSLineBreakMode::ByTruncatingTail),
            created: plain(12.0, 400.0, NSLineBreakMode::ByTruncatingTail),
            modified: plain(12.0, 400.0, NSLineBreakMode::ByTruncatingTail),
            inside: plain(12.0, 400.0, NSLineBreakMode::ByTruncatingTail),
            note,
        }
    }

    pub(crate) fn attach(&self, container: &NSView) {
        for field in self.fields() {
            container.addSubview(field);
        }
    }

    pub(crate) fn lay_out(
        &self,
        facts: &NodeFacts,
        tokens: &Palette,
        width: f64,
        padding: f64,
        gap: f64,
        below: f64,
    ) -> f64 {
        let tip = &facts.tip;
        let indexed = facts.indexed.as_ref();
        let fact = |pick: fn(&IndexFacts) -> Option<&str>| indexed.and_then(pick).unwrap_or_default();
        let rows = [
            (&*self.name, tip.name.as_str(), tokens.text),
            (&*self.size, tip.size_text.as_str(), tokens.text),
            (&*self.path, tip.path.as_deref().unwrap_or_default(), tokens.dim),
            (&*self.source, tip.source, tokens.dim),
            (&*self.created, fact(|facts| facts.created.as_deref()), tokens.dim),
            (&*self.modified, fact(|facts| facts.modified.as_deref()), tokens.dim),
            (&*self.inside, fact(|facts| facts.inside.as_deref()), tokens.text),
            (&*self.note, tip.note.unwrap_or_default(), tokens.mute),
        ];
        FactRows {
            width,
            padding,
            gap,
            below,
            rows: &rows,
        }
        .lay_out()
    }

    fn fields(&self) -> [&NSTextField; 8] {
        [
            &self.name,
            &self.size,
            &self.path,
            &self.source,
            &self.created,
            &self.modified,
            &self.inside,
            &self.note,
        ]
    }
}
