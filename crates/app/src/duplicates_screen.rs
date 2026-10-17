use std::cell::RefCell;

use domovoi_theme::format_size;
use objc2::Message;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSLineBreakMode, NSOutlineView, NSScrollView, NSTableColumn,
    NSTableViewColumnAutoresizingStyle, NSTableViewStyle, NSTextField, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::button_style::ButtonStyle;
use crate::duplicates_layout::DuplicatesLayout;
use crate::duplicates_model::DuplicatesModel;
use crate::duplicates_source::DuplicatesSource;
use crate::duplicates_source_model::DuplicatesSourceModel;
use crate::duplicates_state::DuplicatesState;
use crate::empty_state::empty_state;
use crate::fill_view::{FillStyle, FillView};
use crate::label::{label, measure_label};
use crate::pane_head::pane_head;
use crate::probes::Probes;
use crate::section::Section;
use crate::segmented_switch::SegmentedSwitch;
use crate::shell_actions::{CLONE_SEGMENT, ShellActions, TRASH_SEGMENT};
use crate::shell_button::ShellButton;
use crate::shell_data::ShellData;
use crate::shell_layout::{PANE_PADDING, WINDOW, from_top};
use crate::shell_palette::ShellPalette;
use crate::strings::{
    ACTION_CLONE, ACTION_TRASH, CLONE_EXPLANATION, DUPLICATES, DUPLICATES_IDLE, DUPLICATES_NOTE, FIND_DUPLICATES,
    NO_DUPLICATES, OF, OTHER_USERS_FILES, STOP, TO_TRAY,
};
use crate::toggle_switch::{SIZE as TOGGLE_SIZE, ToggleSwitch};
use crate::ui_font::{interface_font, mono_font};
use crate::window_frame::WindowFrame;

const HEAD_HEIGHT: f64 = 76.0;
const TITLE_TOP: f64 = 17.0;
const LINE_GAP: f64 = 4.0;
const HEAD_GAP: f64 = 16.0;
const TOGGLE_GAP: f64 = 10.0;
const BUTTON_HEIGHT: f64 = 34.0;
const FOOTER_PADDING: f64 = 16.0;
const PROGRESS_WIDTH: f64 = 360.0;
const PROGRESS_HEIGHT: f64 = 6.0;
const PROGRESS_GAP: f64 = 14.0;

#[derive(Debug)]
pub struct DuplicatesScreen {
    pub frame: WindowFrame,
    pub title: Retained<NSTextField>,
    pub toggle: Retained<ToggleSwitch>,
    pub action: Option<SegmentedSwitch>,
    pub apply: Option<Retained<ShellButton>>,
    pub outline: Option<(Retained<NSOutlineView>, Retained<DuplicatesSource>)>,
    pub progress: Option<(Retained<FillView>, Retained<NSTextField>, Retained<NSTextField>)>,
    pub probes: Probes,
}

impl DuplicatesScreen {
    pub fn build(marker: MainThreadMarker, data: &ShellData, palette: &ShellPalette, actions: &ShellActions) -> Self {
        let model = &data.care.duplicates;
        let frame = WindowFrame::build(marker, data, Section::Duplicates, palette, actions);
        let pane = WindowFrame::pane_size();
        let head = pane_head(marker, &frame.pane, HEAD_HEIGHT, palette);
        let title = head_titles(marker, &head, model, palette);
        let groups = model.groups();
        let (toggle, action) = head_controls(marker, &head, model, palette, actions);
        let (footer_height, apply) = if groups.is_empty() {
            (0.0, None)
        } else {
            let (height, button) = footer(marker, &frame.pane, model, palette, actions);
            (height, Some(button))
        };
        let body = NSRect::new(
            NSPoint::new(0.0, footer_height),
            NSSize::new(pane.width, pane.height - HEAD_HEIGHT - footer_height),
        );
        let mut outline = None;
        let mut progress = None;
        match &model.state {
            DuplicatesState::Idle => frame
                .pane
                .addSubview(&placed(empty_state(marker, body.size, DUPLICATES_IDLE, palette), body)),
            DuplicatesState::Searching { done, total, .. } => {
                progress = Some(progress_block(
                    marker,
                    &frame.pane,
                    body,
                    (*done, *total),
                    palette,
                    actions,
                ));
            }
            DuplicatesState::Found { .. } if groups.is_empty() => {
                frame
                    .pane
                    .addSubview(&placed(empty_state(marker, body.size, NO_DUPLICATES, palette), body));
            }
            DuplicatesState::Found { .. } => {
                outline = Some(groups_outline(marker, &frame.pane, body, model, palette, actions));
            }
        }
        let probes = Probes {
            window_background: [WINDOW.width - 8.0, WINDOW.height - HEAD_HEIGHT - 8.0],
            sidebar: Some([8.0, 8.0]),
            accent_button: None,
        };
        Self {
            frame,
            title,
            toggle,
            action,
            apply,
            outline,
            progress,
            probes,
        }
    }

    pub fn show_progress(&self, stage: &str, done: u64, total: u64) {
        if let Some((bar, count, title)) = &self.progress {
            let share = done as f64 / total.max(1) as f64;
            bar.setFrameSize(NSSize::new(PROGRESS_WIDTH * share.clamp(0.0, 1.0), PROGRESS_HEIGHT));
            count.setStringValue(&NSString::from_str(&progress_count(done, total)));
            title.setStringValue(&NSString::from_str(stage));
        }
    }

    pub fn show_selection(&self, model: &DuplicatesModel) {
        if let Some(title) = model.title_line() {
            self.title.setStringValue(&NSString::from_str(&title));
        }
        if let Some(apply) = &self.apply {
            apply.retitle(&apply_title(model));
            apply.setEnabled(model.will_free() > 0);
        }
    }
}

fn head_titles(
    marker: MainThreadMarker,
    head: &FillView,
    model: &DuplicatesModel,
    palette: &ShellPalette,
) -> Retained<NSTextField> {
    let tokens = palette.tokens;
    let head_size = head.frame().size;
    let pinned = NSAutoresizingMaskOptions::ViewMinYMargin;
    let title = label(
        marker,
        &model.title_line().unwrap_or_else(|| DUPLICATES.to_owned()),
        &interface_font(17.0, 600.0),
        &tokens.text.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    let title_size = title.fittingSize();
    title.setFrame(from_top(head_size, PANE_PADDING, TITLE_TOP, title_size));
    title.setAutoresizingMask(pinned);
    head.addSubview(&title);
    let subtitle = match &model.state {
        DuplicatesState::Found { .. } => model.checked_line(),
        DuplicatesState::Searching { stage, .. } => Some(stage.title().to_owned()),
        DuplicatesState::Idle => None,
    };
    if let Some(subtitle) = subtitle {
        let small = label(
            marker,
            &subtitle,
            &interface_font(12.5, 400.0),
            &tokens.dim.ns_color(),
            NSLineBreakMode::ByTruncatingTail,
        );
        small.setFrame(from_top(
            head_size,
            PANE_PADDING,
            TITLE_TOP + title_size.height + LINE_GAP,
            small.fittingSize(),
        ));
        small.setAutoresizingMask(pinned);
        head.addSubview(&small);
    }
    title
}

fn head_controls(
    marker: MainThreadMarker,
    head: &FillView,
    model: &DuplicatesModel,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> (Retained<ToggleSwitch>, Option<SegmentedSwitch>) {
    let tokens = palette.tokens;
    let pane = WindowFrame::pane_size();
    let head_size = head.frame().size;
    let pinned_right = NSAutoresizingMaskOptions::ViewMinXMargin | NSAutoresizingMaskOptions::ViewMinYMargin;
    let groups = model.groups();
    let mut right = pane.width - PANE_PADDING;
    let action = (!groups.is_empty()).then(|| {
        let switch = SegmentedSwitch::build(
            marker,
            &[(ACTION_TRASH, true), (ACTION_CLONE, model.clone_available())],
            if model.clone_mode { CLONE_SEGMENT } else { TRASH_SEGMENT },
            palette,
            (actions, sel!(selectDuplicateAction:)),
        );
        let size = switch.view.frame().size;
        switch.view.setFrame(from_top(
            head_size,
            right - size.width,
            (HEAD_HEIGHT - size.height) / 2.0,
            size,
        ));
        switch.view.setAutoresizingMask(pinned_right);
        head.addSubview(&switch.view);
        right -= size.width + HEAD_GAP;
        switch
    });
    let searching = !matches!(model.state, DuplicatesState::Idle);
    let toggle = ToggleSwitch::new(
        marker,
        FIND_DUPLICATES,
        searching,
        palette,
        actions,
        sel!(toggleDuplicateSearch:),
    );
    toggle.setFrame(from_top(
        head_size,
        right - TOGGLE_SIZE.width,
        (HEAD_HEIGHT - TOGGLE_SIZE.height) / 2.0,
        TOGGLE_SIZE,
    ));
    toggle.setAutoresizingMask(pinned_right);
    head.addSubview(&toggle);
    let caption = measure_label(
        marker,
        FIND_DUPLICATES,
        &interface_font(13.0, 400.0),
        &tokens.mute.ns_color(),
    );
    let caption_size = caption.fittingSize();
    caption.setFrame(from_top(
        head_size,
        right - TOGGLE_SIZE.width - TOGGLE_GAP - caption_size.width,
        (HEAD_HEIGHT - caption_size.height) / 2.0,
        caption_size,
    ));
    caption.setAutoresizingMask(pinned_right);
    head.addSubview(&caption);
    (toggle, action)
}

fn placed(view: Retained<NSView>, frame: NSRect) -> Retained<NSView> {
    view.setFrame(frame);
    view
}

fn apply_title(model: &DuplicatesModel) -> String {
    let verb = if model.clone_mode { ACTION_CLONE } else { TO_TRAY };
    format!("{verb} · {}", format_size(model.will_free()))
}

fn progress_count(done: u64, total: u64) -> String {
    format!("{done} {OF} {total}")
}

fn footer(
    marker: MainThreadMarker,
    pane: &NSView,
    model: &DuplicatesModel,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> (f64, Retained<ShellButton>) {
    let tokens = palette.tokens;
    let size = WindowFrame::pane_size();
    let apply = ShellButton::new(
        marker,
        &apply_title(model),
        NSPoint::ZERO,
        BUTTON_HEIGHT,
        ButtonStyle {
            fill: Some(tokens.wax.ns_color()),
            border: None,
            text: tokens.wax_ink.ns_color(),
            font: interface_font(13.0, 600.0),
        },
    );
    apply.on_click(actions, sel!(applyDuplicates:));
    apply.setEnabled(model.will_free() > 0);
    let apply_size = apply.frame().size;
    let text_width = size.width - 3.0 * PANE_PADDING - apply_size.width;
    let note_text = if model.clone_mode {
        format!("{CLONE_EXPLANATION} {DUPLICATES_NOTE}")
    } else {
        DUPLICATES_NOTE.to_owned()
    };
    let note = label(
        marker,
        &note_text,
        &interface_font(12.5, 400.0),
        &tokens.mute.ns_color(),
        NSLineBreakMode::ByWordWrapping,
    );
    note.setPreferredMaxLayoutWidth(text_width);
    let note_height = note.fittingSize().height;
    let others = label(
        marker,
        OTHER_USERS_FILES,
        &interface_font(12.0, 400.0),
        &tokens.dim.ns_color(),
        NSLineBreakMode::ByWordWrapping,
    );
    others.setPreferredMaxLayoutWidth(text_width);
    let others_height = others.fittingSize().height;
    let height = (note_height + LINE_GAP + others_height).max(BUTTON_HEIGHT) + 2.0 * FOOTER_PADDING;
    let band = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(size.width, height)),
        FillStyle {
            fill: Some(tokens.surface.ns_color()),
            border: None,
            radius: 0.0,
            dashed: false,
        },
    );
    band.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMaxYMargin);
    let rule = FillView::new(
        marker,
        NSRect::new(NSPoint::new(0.0, height - 1.0), NSSize::new(size.width, 1.0)),
        FillStyle {
            fill: Some(palette.line_soft()),
            border: None,
            radius: 0.0,
            dashed: false,
        },
    );
    rule.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    band.addSubview(&rule);
    let band_size = band.frame().size;
    note.setFrame(from_top(
        band_size,
        PANE_PADDING,
        FOOTER_PADDING,
        NSSize::new(text_width, note_height),
    ));
    others.setFrame(from_top(
        band_size,
        PANE_PADDING,
        FOOTER_PADDING + note_height + LINE_GAP,
        NSSize::new(text_width, others_height),
    ));
    apply.setFrame(from_top(
        band_size,
        size.width - PANE_PADDING - apply_size.width,
        (height - BUTTON_HEIGHT) / 2.0,
        apply_size,
    ));
    apply.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
    for child in [&*note as &NSView, &others, &apply] {
        band.addSubview(child);
    }
    pane.addSubview(&band);
    (height, apply)
}

fn progress_block(
    marker: MainThreadMarker,
    pane: &NSView,
    body: NSRect,
    (done, total): (u64, u64),
    palette: &ShellPalette,
    actions: &ShellActions,
) -> (Retained<FillView>, Retained<NSTextField>, Retained<NSTextField>) {
    let tokens = palette.tokens;
    let container = NSView::initWithFrame(NSView::alloc(marker), body);
    container.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    let title = label(
        marker,
        "",
        &interface_font(15.0, 600.0),
        &tokens.text.ns_color(),
        NSLineBreakMode::ByTruncatingTail,
    );
    let count = measure_label(
        marker,
        &progress_count(done, total),
        &mono_font(12.0, 400.0),
        &tokens.dim.ns_color(),
    );
    let stop = ShellButton::new(
        marker,
        STOP,
        NSPoint::ZERO,
        28.0,
        ButtonStyle {
            fill: None,
            border: Some(palette.line()),
            text: tokens.text.ns_color(),
            font: interface_font(13.0, 500.0),
        },
    );
    stop.on_click(actions, sel!(toggleDuplicateSearch:));
    let title_height = title.fittingSize().height.max(20.0);
    let count_size = count.fittingSize();
    let stop_size = stop.frame().size;
    let block = title_height
        + PROGRESS_GAP
        + PROGRESS_HEIGHT
        + PROGRESS_GAP
        + count_size.height
        + PROGRESS_GAP
        + stop_size.height;
    let top = (body.size.height - block) / 2.0;
    let left = (body.size.width - PROGRESS_WIDTH) / 2.0;
    title.setFrame(from_top(
        body.size,
        left,
        top,
        NSSize::new(PROGRESS_WIDTH, title_height),
    ));
    let track_frame = from_top(
        body.size,
        left,
        top + title_height + PROGRESS_GAP,
        NSSize::new(PROGRESS_WIDTH, PROGRESS_HEIGHT),
    );
    let track = FillView::new(
        marker,
        track_frame,
        FillStyle {
            fill: Some(palette.line_soft()),
            border: None,
            radius: PROGRESS_HEIGHT / 2.0,
            dashed: false,
        },
    );
    let bar = FillView::new(
        marker,
        NSRect::new(track_frame.origin, NSSize::new(0.0, PROGRESS_HEIGHT)),
        FillStyle {
            fill: Some(tokens.wax.ns_color()),
            border: None,
            radius: PROGRESS_HEIGHT / 2.0,
            dashed: false,
        },
    );
    let count_top = top + title_height + 2.0 * PROGRESS_GAP + PROGRESS_HEIGHT;
    count.setFrame(from_top(body.size, left, count_top, count_size));
    stop.setFrame(from_top(
        body.size,
        left,
        count_top + count_size.height + PROGRESS_GAP,
        stop_size,
    ));
    let centered = NSAutoresizingMaskOptions::ViewMinXMargin
        | NSAutoresizingMaskOptions::ViewMaxXMargin
        | NSAutoresizingMaskOptions::ViewMinYMargin
        | NSAutoresizingMaskOptions::ViewMaxYMargin;
    for child in [&*title as &NSView, &track, &bar, &count, &stop] {
        child.setAutoresizingMask(centered);
        container.addSubview(child);
    }
    pane.addSubview(&container);
    let parts = (bar, count, title);
    parts.0.setFrameSize(NSSize::new(
        PROGRESS_WIDTH * (done as f64 / total.max(1) as f64).clamp(0.0, 1.0),
        PROGRESS_HEIGHT,
    ));
    parts
}

fn groups_outline(
    marker: MainThreadMarker,
    pane: &NSView,
    body: NSRect,
    model: &DuplicatesModel,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> (Retained<NSOutlineView>, Retained<DuplicatesSource>) {
    let inner = NSRect::new(
        NSPoint::new(PANE_PADDING, body.origin.y),
        NSSize::new(body.size.width - 2.0 * PANE_PADDING, body.size.height),
    );
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(marker), inner);
    scroll.setDrawsBackground(false);
    scroll.setHasVerticalScroller(true);
    scroll.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    let outline = NSOutlineView::initWithFrame(NSOutlineView::alloc(marker), NSRect::new(NSPoint::ZERO, inner.size));
    outline.setStyle(NSTableViewStyle::Plain);
    outline.setHeaderView(None);
    outline.setBackgroundColor(&palette.tokens.bg.ns_color());
    outline.setIndentationPerLevel(0.0);
    outline.setIntercellSpacing(NSSize::new(0.0, 0.0));
    outline.setFloatsGroupRows(false);
    outline.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle);
    let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(marker), &NSString::from_str("duplicates"));
    column.setWidth(inner.size.width);
    outline.addTableColumn(&column);
    unsafe { outline.setOutlineTableColumn(Some(&column)) };
    let groups = model.groups().to_vec();
    let source = DuplicatesSource::new(
        marker,
        DuplicatesSourceModel {
            layout: DuplicatesLayout::of(&groups),
            groups,
            clone_mode: model.clone_mode,
            palette: *palette,
            width: inner.size.width,
            actions: actions.retain(),
            keys: RefCell::new(Vec::new()),
        },
    );
    unsafe {
        outline.setDataSource(Some(ProtocolObject::from_ref(&*source)));
        outline.setDelegate(Some(ProtocolObject::from_ref(&*source)));
    }
    outline.reloadData();
    unsafe { outline.expandItem_expandChildren(None, true) };
    scroll.setDocumentView(Some(&outline));
    outline.sizeLastColumnToFit();
    pane.addSubview(&scroll);
    (outline, source)
}
