use domovoi_theme::Rgba;
use objc2_app_kit::{NSAccessibility, NSTextField};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

pub(crate) struct FactRows<'a> {
    pub(crate) width: f64,
    pub(crate) padding: f64,
    pub(crate) gap: f64,
    pub(crate) below: f64,
    pub(crate) rows: &'a [(&'a NSTextField, &'a str, Rgba)],
}

impl FactRows<'_> {
    pub(crate) fn lay_out(&self) -> f64 {
        let inner = self.width - 2.0 * self.padding;
        let visible: Vec<_> = self.rows.iter().filter(|(_, text, _)| !text.is_empty()).collect();
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
        let height = 2.0 * self.padding
            + heights.iter().sum::<f64>()
            + self.gap * heights.len().saturating_sub(1) as f64
            + self.below;
        for (field, _, _) in self.rows {
            field.setHidden(true);
        }
        let mut top = self.padding;
        for ((field, _, _), line_height) in visible.iter().zip(&heights) {
            field.setHidden(false);
            field.setFrame(NSRect::new(
                NSPoint::new(self.padding, height - top - line_height),
                NSSize::new(inner, *line_height),
            ));
            top += line_height + self.gap;
        }
        height
    }
}
