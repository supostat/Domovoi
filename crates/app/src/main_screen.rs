use domovoi_render::{DiagramMode, DiagramState};
use domovoi_theme::format_size;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSLineBreakMode, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::breadcrumbs::Breadcrumbs;
use crate::button_style::ButtonStyle;
use crate::label::{label, measure_label};
use crate::legend::legend;
use crate::level_list::LevelList;
use crate::probes::Probes;
use crate::section::Section;
use crate::segmented_switch::SegmentedSwitch;
use crate::shell_actions::ShellActions;
use crate::shell_actions::{SUNBURST_SEGMENT, TREEMAP_SEGMENT};
use crate::shell_button::ShellButton;
use crate::shell_data::ShellData;
use crate::shell_layout::{PANE_PADDING, WINDOW, from_top};
use crate::shell_palette::ShellPalette;
use crate::strings::{DIAGRAM, IN_CLOUD, OUTSIDE_TOTAL, SOURCE_SCAN, SUNBURST, TRAY, TREEMAP};
use crate::ui_font::{interface_font, mono_font};
use crate::window_frame::WindowFrame;

const HEAD_TOP: f64 = 20.0;
const HEAD_HEIGHT: f64 = 30.0;
const HEAD_GAP: f64 = 16.0;
const DIAGRAM_SIDE: f64 = 440.0;
const BODY_TOP: f64 = 70.0;
const LIST_GAP: f64 = 24.0;
const LEGEND_TOP_GAP: f64 = 16.0;
const NOTICE_GAP: f64 = 12.0;

#[derive(Debug)]
pub struct MainScreen {
    pub frame: WindowFrame,
    pub crumbs: Retained<Breadcrumbs>,
    pub list: LevelList,
    pub free: Retained<NSTextField>,
    pub in_cloud: Option<Retained<NSTextField>>,
    pub tray: Retained<ShellButton>,
    pub modes: SegmentedSwitch,
    pub diagram_frame: NSRect,
    pub probes: Probes,
}

impl MainScreen {
    pub fn build(
        marker: MainThreadMarker,
        data: &ShellData,
        focus: usize,
        mode: DiagramMode,
        palette: &ShellPalette,
        actions: &ShellActions,
    ) -> Self {
        let tokens = palette.tokens;
        let frame = WindowFrame::build(marker, data, Section::Disk, palette, actions);
        let pane = WindowFrame::pane_size();
        let left = PANE_PADDING;
        let right = pane.width - PANE_PADDING;
        let pinned_top = NSAutoresizingMaskOptions::ViewMinYMargin;
        let pinned_top_right = NSAutoresizingMaskOptions::ViewMinXMargin | NSAutoresizingMaskOptions::ViewMinYMargin;
        let tray = ShellButton::new(
            marker,
            TRAY,
            NSPoint::ZERO,
            HEAD_HEIGHT,
            ButtonStyle {
                fill: Some(tokens.wax.ns_color()),
                border: None,
                text: tokens.wax_ink.ns_color(),
                font: interface_font(13.0, 600.0),
            },
        );
        tray.on_click(actions, sel!(openTray:));
        let tray_size = tray.frame().size;
        let tray_frame = from_top(pane, right - tray_size.width, HEAD_TOP, tray_size);
        tray.setFrame(tray_frame);
        tray.setAutoresizingMask(pinned_top_right);
        frame.pane.addSubview(&tray);
        let mut head_right = tray_frame.origin.x - HEAD_GAP;
        let free = measure_label(
            marker,
            &data.free_short(),
            &mono_font(12.0, 400.0),
            &tokens.dim.ns_color(),
        );
        head_right = place_head(&free, pane, head_right, pinned_top_right, &frame.pane);
        let in_cloud = (data.in_cloud_bytes > 0).then(|| {
            let cloud = measure_label(
                marker,
                &format!("{IN_CLOUD} {}", format_size(data.in_cloud_bytes)),
                &mono_font(12.0, 400.0),
                &tokens.dim.ns_color(),
            );
            cloud.setToolTip(Some(&NSString::from_str(&format!("{OUTSIDE_TOTAL} · {SOURCE_SCAN}"))));
            head_right = place_head(&cloud, pane, head_right, pinned_top_right, &frame.pane);
            cloud
        });
        let modes = SegmentedSwitch::build(
            marker,
            &[(SUNBURST, true), (TREEMAP, true)],
            match mode {
                DiagramMode::Sunburst => SUNBURST_SEGMENT,
                DiagramMode::Treemap => TREEMAP_SEGMENT,
            },
            palette,
            (actions, sel!(selectMode:)),
        );
        let crumbs_right = place_head(&modes.view, pane, head_right, pinned_top_right, &frame.pane);
        let crumbs = Breadcrumbs::new(
            marker,
            from_top(pane, left, HEAD_TOP, NSSize::new(crumbs_right - left, HEAD_HEIGHT)),
            &data.crumbs(focus),
            palette,
            actions,
        );
        crumbs.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | pinned_top);
        frame.pane.addSubview(&crumbs);
        let diagram_frame = from_top(pane, left, BODY_TOP, NSSize::new(DIAGRAM_SIDE, DIAGRAM_SIDE));
        legend_block(marker, &frame.pane, data, palette);
        let list_left = left + DIAGRAM_SIDE + LIST_GAP;
        let list_frame = from_top(pane, list_left, BODY_TOP, NSSize::new(right - list_left, DIAGRAM_SIDE));
        let list = LevelList::build(marker, list_frame, data.level_rows(focus), *palette, actions);
        list.scroll.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        frame.pane.addSubview(&list.scroll);
        let pane_x = WindowFrame::pane_origin_x();
        let probes = Probes {
            window_background: [WINDOW.width - 8.0, WINDOW.height - 8.0],
            sidebar: Some([8.0, 8.0]),
            accent_button: Some([
                pane_x + tray_frame.origin.x,
                HEAD_TOP,
                tray_size.width,
                tray_size.height,
            ]),
        };
        Self {
            frame,
            crumbs,
            list,
            free,
            in_cloud,
            tray,
            modes,
            diagram_frame,
            probes,
        }
    }
}

impl MainScreen {
    pub fn diagram_state(data: &ShellData, focus: usize, mode: DiagramMode, palette: &ShellPalette) -> DiagramState {
        let side = DIAGRAM_SIDE as f32;
        let mut state = DiagramState::new(data.tree.clone(), mode, DIAGRAM, palette.tokens, side, side);
        state.jump_to(focus);
        state
    }
}

fn legend_block(marker: MainThreadMarker, container: &NSView, data: &ShellData, palette: &ShellPalette) {
    let tokens = palette.tokens;
    let pane = WindowFrame::pane_size();
    let left = PANE_PADDING;
    let right = pane.width - PANE_PADDING;
    let pinned_top = NSAutoresizingMaskOptions::ViewMinYMargin;
    let chips = legend(marker, &data.legend(), palette, right - left);
    let chips_size = chips.frame().size;
    chips.setFrame(from_top(
        pane,
        left,
        BODY_TOP + DIAGRAM_SIDE + LEGEND_TOP_GAP,
        chips_size,
    ));
    chips.setAutoresizingMask(pinned_top);
    container.addSubview(&chips);
    if let Some(notice) = data.hidden_notice() {
        let field = label(
            marker,
            &notice,
            &interface_font(13.0, 400.0),
            &tokens.mute.ns_color(),
            NSLineBreakMode::ByWordWrapping,
        );
        field.setPreferredMaxLayoutWidth(right - left);
        let height = field.fittingSize().height;
        field.setFrame(from_top(
            pane,
            left,
            BODY_TOP + DIAGRAM_SIDE + LEGEND_TOP_GAP + chips_size.height + NOTICE_GAP,
            NSSize::new(right - left, height),
        ));
        field.setAutoresizingMask(pinned_top);
        container.addSubview(&field);
    }
}

fn place_head(
    view: &NSView,
    pane: NSSize,
    right_edge: f64,
    mask: NSAutoresizingMaskOptions,
    container: &NSView,
) -> f64 {
    let size = view.frame().size;
    let frame = from_top(
        pane,
        right_edge - size.width,
        HEAD_TOP + (HEAD_HEIGHT - size.height) / 2.0,
        size,
    );
    view.setFrame(frame);
    view.setAutoresizingMask(mask);
    container.addSubview(view);
    frame.origin.x - HEAD_GAP
}
