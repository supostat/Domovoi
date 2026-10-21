use domovoi_render::{DiagramMode, DiagramState};
use domovoi_theme::format_size;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSLineBreakMode, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::breadcrumbs::Breadcrumbs;
use crate::button_style::ButtonStyle;
use crate::label::{label, measure_label, retext};
use crate::legend::legend;
use crate::legend_entry::LegendEntry;
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
    palette: ShellPalette,
    legend_content: (Vec<LegendEntry>, Option<String>),
    legend_views: Vec<Retained<NSView>>,
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
        let free = measure_label(marker, "", &mono_font(12.0, 400.0), &tokens.dim.ns_color());
        let in_cloud = (data.in_cloud_bytes > 0).then(|| {
            let cloud = measure_label(marker, "", &mono_font(12.0, 400.0), &tokens.dim.ns_color());
            cloud.setToolTip(Some(&NSString::from_str(&format!("{OUTSIDE_TOTAL} · {SOURCE_SCAN}"))));
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
        let pin_to_head = |view: &NSView| {
            view.setAutoresizingMask(pinned_top_right);
            frame.pane.addSubview(view);
        };
        pin_to_head(&tray);
        pin_to_head(&free);
        pin_to_head(&modes.view);
        if let Some(cloud) = &in_cloud {
            pin_to_head(cloud);
        }
        let crumbs = Breadcrumbs::new(
            marker,
            NSRect::new(NSPoint::ZERO, NSSize::new(0.0, HEAD_HEIGHT)),
            &data.crumbs(focus),
            palette,
            actions,
        );
        crumbs.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | pinned_top);
        frame.pane.addSubview(&crumbs);
        let diagram_frame = from_top(pane, left, BODY_TOP, NSSize::new(DIAGRAM_SIDE, DIAGRAM_SIDE));
        let list_left = left + DIAGRAM_SIDE + LIST_GAP;
        let list_frame = from_top(pane, list_left, BODY_TOP, NSSize::new(right - list_left, DIAGRAM_SIDE));
        let list = LevelList::build(marker, list_frame, *palette, actions);
        list.scroll.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        frame.pane.addSubview(&list.scroll);
        let mut screen = Self {
            frame,
            crumbs,
            list,
            free,
            in_cloud,
            tray,
            modes,
            diagram_frame,
            probes: Probes {
                window_background: [WINDOW.width - 8.0, WINDOW.height - 8.0],
                sidebar: Some([8.0, 8.0]),
                accent_button: None,
            },
            palette: *palette,
            legend_content: (Vec::new(), None),
            legend_views: Vec::new(),
        };
        screen.show(data, focus);
        let tray_frame = screen.tray.frame();
        screen.probes.accent_button = Some([
            WindowFrame::pane_origin_x() + tray_frame.origin.x,
            HEAD_TOP,
            tray_frame.size.width,
            tray_frame.size.height,
        ]);
        screen
    }

    pub fn update(&mut self, data: &ShellData, focus: usize) {
        self.frame.update(data, Section::Disk);
        self.show(data, focus);
    }

    fn show(&mut self, data: &ShellData, focus: usize) {
        retext(&self.free, &data.free_short());
        if let Some(cloud) = &self.in_cloud {
            retext(cloud, &format!("{IN_CLOUD} {}", format_size(data.in_cloud_bytes)));
        }
        self.crumbs.show_nodes(&data.crumbs(focus));
        self.lay_out_head();
        self.list.show(data.level_rows(focus));
        self.show_legend(data);
    }

    fn lay_out_head(&self) {
        let pane = self.frame.pane.frame().size;
        let mut right_edge = place_head(&self.tray, pane, pane.width - PANE_PADDING);
        right_edge = place_head(&self.free, pane, right_edge);
        if let Some(cloud) = &self.in_cloud {
            right_edge = place_head(cloud, pane, right_edge);
        }
        let crumbs_right = place_head(&self.modes.view, pane, right_edge);
        self.crumbs.setFrame(from_top(
            pane,
            PANE_PADDING,
            HEAD_TOP,
            NSSize::new(crumbs_right - PANE_PADDING, HEAD_HEIGHT),
        ));
    }

    fn show_legend(&mut self, data: &ShellData) {
        let content = (data.legend(), data.hidden_notice());
        if content == self.legend_content {
            return;
        }
        for view in self.legend_views.drain(..) {
            view.removeFromSuperview();
        }
        self.legend_views = legend_block(
            MainThreadMarker::from(&*self.frame.pane),
            &self.frame.pane,
            &content,
            &self.palette,
        );
        self.legend_content = content;
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

fn legend_block(
    marker: MainThreadMarker,
    container: &NSView,
    (entries, notice): &(Vec<LegendEntry>, Option<String>),
    palette: &ShellPalette,
) -> Vec<Retained<NSView>> {
    let tokens = palette.tokens;
    let pane = NSSize::new(WindowFrame::pane_size().width, container.frame().size.height);
    let left = PANE_PADDING;
    let right = pane.width - PANE_PADDING;
    let pinned_top = NSAutoresizingMaskOptions::ViewMinYMargin;
    let chips = legend(marker, entries, palette, right - left);
    let chips_size = chips.frame().size;
    chips.setFrame(from_top(
        pane,
        left,
        BODY_TOP + DIAGRAM_SIDE + LEGEND_TOP_GAP,
        chips_size,
    ));
    chips.setAutoresizingMask(pinned_top);
    container.addSubview(&chips);
    let mut views = vec![chips];
    if let Some(notice) = notice {
        let field = label(
            marker,
            notice,
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
        views.push(field.into_super().into_super());
    }
    views
}

fn place_head(view: &NSView, pane: NSSize, right_edge: f64) -> f64 {
    let size = view.frame().size;
    let frame = from_top(
        pane,
        right_edge - size.width,
        HEAD_TOP + (HEAD_HEIGHT - size.height) / 2.0,
        size,
    );
    view.setFrame(frame);
    frame.origin.x - HEAD_GAP
}
