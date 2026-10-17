use domovoi_theme::{Palette, format_size};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSAccessibility, NSAutoresizingMaskOptions, NSLineBreakMode, NSTextField, NSView};
use objc2_foundation::{NSInteger, NSPoint, NSRect, NSSize, NSString};

use crate::check_box::{CheckBox, SIDE};
use crate::fill_view::{FillStyle, FillView};
use crate::group_model::GroupModel;
use crate::label::{label, measure_label};
use crate::member_model::MemberModel;
use crate::pill_badge::pill_badge;
use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;
use crate::strings::{APFS_CLONE, KEEP};
use crate::tracked_caps::tracked_caps;
use crate::ui_font::{interface_font, mono_font};

pub(crate) const GROUP_GAP: f64 = 12.0;
pub(crate) const GROUP_HEIGHT: f64 = 42.0;
pub(crate) const MEMBER_HEIGHT: f64 = 38.0;
const PADDING: f64 = 16.0;
const MARK_COLUMN: f64 = 64.0;
const GAP: f64 = 10.0;
const KEEP_TRACKING: f64 = 0.06;
const KEEP_SIZE: f64 = 10.5;

pub(crate) fn group_row(
    marker: MainThreadMarker,
    group: &GroupModel,
    width: f64,
    palette: &ShellPalette,
) -> Retained<NSView> {
    let tokens = palette.tokens;
    let row = NSView::initWithFrame(
        NSView::alloc(marker),
        NSRect::new(NSPoint::ZERO, NSSize::new(width, GROUP_GAP + GROUP_HEIGHT)),
    );
    let band = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(width, GROUP_HEIGHT)),
        FillStyle {
            fill: Some(tokens.surface_2.ns_color()),
            border: Some(palette.line_soft()),
            radius: 0.0,
            dashed: false,
        },
    );
    let summary = measure_label(
        marker,
        &group.summary(),
        &mono_font(12.0, 500.0),
        &tokens.mute.ns_color(),
    );
    let summary_size = summary.fittingSize();
    summary.setFrameOrigin(NSPoint::new(
        width - PADDING - summary_size.width,
        (GROUP_HEIGHT - summary_size.height) / 2.0,
    ));
    let name = label(
        marker,
        &group.name,
        &interface_font(13.0, 600.0),
        &tokens.text.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    let full = NSString::from_str(&group.name);
    name.setToolTip(Some(&full));
    name.setAccessibilityLabel(Some(&full));
    let name_height = name.fittingSize().height;
    name.setFrame(NSRect::new(
        NSPoint::new(PADDING, (GROUP_HEIGHT - name_height) / 2.0),
        NSSize::new((width - 3.0 * PADDING - summary_size.width).max(0.0), name_height),
    ));
    row.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    band.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    name.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    summary.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
    band.addSubview(&name);
    band.addSubview(&summary);
    row.addSubview(&band);
    row
}

#[derive(Debug)]
pub struct MemberRow {
    pub view: Retained<NSView>,
    pub path: Retained<NSTextField>,
    pub size: Option<Retained<NSTextField>>,
}

pub fn member_row(
    marker: MainThreadMarker,
    member: &MemberModel,
    toggle: Option<usize>,
    clone_mode: bool,
    width: f64,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> MemberRow {
    let tokens = palette.tokens;
    let row = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(width, MEMBER_HEIGHT)),
        FillStyle {
            fill: Some(tokens.surface.ns_color()),
            border: Some(palette.line_soft()),
            radius: 0.0,
            dashed: false,
        },
    );
    let mark: Retained<NSView> = match toggle {
        None => keep_label(marker, tokens).into_super().into_super(),
        Some(tag) => CheckBox::new(
            marker,
            &member.path,
            member.checked && member.can_check(clone_mode),
            member.can_check(clone_mode),
            palette,
            (
                actions,
                sel!(toggleCopy:),
                NSInteger::try_from(tag).expect("copies are few"),
            ),
        )
        .into_super()
        .into_super()
        .into_super(),
    };
    let mark_size = mark.frame().size;
    mark.setFrameOrigin(NSPoint::new(PADDING, (MEMBER_HEIGHT - mark_size.height) / 2.0));
    if toggle.is_some() {
        mark.setFrameSize(NSSize::new(SIDE, SIDE));
    }
    row.addSubview(&mark);
    let size = (!member.clone).then(|| {
        measure_label(
            marker,
            &format_size(member.size),
            &mono_font(12.5, 500.0),
            &tokens.mute.ns_color(),
        )
    });
    let trailing: Retained<NSView> = match &size {
        Some(size) => size.clone().into_super().into_super(),
        None => pill_badge(marker, APFS_CLONE, tokens.warn, tokens.warn_soft).into_super(),
    };
    let trailing_size = trailing.frame().size;
    let mut right = width - PADDING - trailing_size.width;
    trailing.setFrameOrigin(NSPoint::new(right, (MEMBER_HEIGHT - trailing_size.height) / 2.0));
    trailing.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
    row.addSubview(&trailing);
    if let Some(refusal) = member.refusal.as_ref().filter(|_| clone_mode) {
        let reason = measure_label(marker, refusal, &interface_font(12.0, 400.0), &tokens.dim.ns_color());
        let reason_size = reason.fittingSize();
        right -= GAP + reason_size.width;
        reason.setFrameOrigin(NSPoint::new(right, (MEMBER_HEIGHT - reason_size.height) / 2.0));
        reason.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
        row.addSubview(&reason);
    }
    let path_x = PADDING + MARK_COLUMN + GAP;
    let path = label(
        marker,
        &member.path,
        &mono_font(12.0, 400.0),
        &tokens.mute.ns_color(),
        NSLineBreakMode::ByTruncatingMiddle,
    );
    let full = NSString::from_str(&member.path);
    path.setToolTip(Some(&full));
    path.setAccessibilityLabel(Some(&full));
    let path_height = path.fittingSize().height;
    path.setFrame(NSRect::new(
        NSPoint::new(path_x, (MEMBER_HEIGHT - path_height) / 2.0),
        NSSize::new((right - GAP - path_x).max(0.0), path_height),
    ));
    path.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    row.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    row.addSubview(&path);
    MemberRow {
        view: row.into_super(),
        path,
        size,
    }
}

fn keep_label(marker: MainThreadMarker, tokens: &Palette) -> Retained<NSTextField> {
    let color = tokens.dim.ns_color();
    let field = label(
        marker,
        "",
        &interface_font(KEEP_SIZE, 600.0),
        &color,
        NSLineBreakMode::ByClipping,
    );
    field.setAttributedStringValue(&tracked_caps(KEEP, KEEP_SIZE, KEEP_TRACKING, &color));
    field.setAccessibilityLabel(Some(&NSString::from_str(KEEP)));
    field.sizeToFit();
    field
}
