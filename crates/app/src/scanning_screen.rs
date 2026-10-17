use domovoi_render::{DiagramMode, DiagramState};
use domovoi_theme::{format_count, format_size};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSAccessibility, NSAutoresizingMaskOptions, NSLineBreakMode, NSTextField};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::button_style::ButtonStyle;
use crate::label::{label, measure_label};
use crate::probes::Probes;
use crate::scan_status::ScanStatus;
use crate::section::Section;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_data::ShellData;
use crate::shell_layout::{PANE_PADDING, WINDOW, from_top};
use crate::shell_palette::ShellPalette;
use crate::strings::{DIAGRAM, FILES_MILLIONS, FULL_FIRST_SCAN, SCAN_ENGINE, SCANNING, STOP, THREADS, WALKED};
use crate::ui_font::{interface_font, mono_font};
use crate::window_frame::WindowFrame;

const DIAGRAM_SIDE: f64 = 380.0;
const HEAD_TOP: f64 = 22.0;
const DIAGRAM_TOP: f64 = 80.0;
const META_GAP: f64 = 14.0;
const PATH_WIDTH: f64 = 280.0;
const PATH_GAP: f64 = 28.0;
const NOTICE_GAP: f64 = 8.0;
const MILLION: f64 = 1_000_000.0;

#[derive(Debug)]
pub struct ScanningScreen {
    pub frame: WindowFrame,
    pub stop: Retained<ShellButton>,
    pub engine: Retained<NSTextField>,
    pub path: Retained<NSTextField>,
    pub diagram_frame: NSRect,
    pub probes: Probes,
}

impl ScanningScreen {
    pub fn build(marker: MainThreadMarker, data: &ShellData, palette: &ShellPalette, actions: &ShellActions) -> Self {
        let tokens = palette.tokens;
        let frame = WindowFrame::build(marker, data, Section::Disk, palette, actions);
        let pane = WindowFrame::pane_size();
        let left = PANE_PADDING;
        let pane_width = pane.width - 2.0 * PANE_PADDING;
        let pinned_top = NSAutoresizingMaskOptions::ViewMinYMargin;
        let title = label(
            marker,
            &format!("{SCANNING} {}", data.disk_name),
            &interface_font(17.0, 600.0),
            &tokens.text.ns_color(),
            NSLineBreakMode::ByTruncatingTail,
        );
        title.setFrame(from_top(pane, left, HEAD_TOP, title.fittingSize()));
        title.setAutoresizingMask(pinned_top);
        frame.pane.addSubview(&title);
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
        stop.on_click(actions, sel!(stopScan:));
        stop.setFrame(from_top(
            pane,
            left + pane_width - stop.frame().size.width,
            HEAD_TOP - 4.0,
            stop.frame().size,
        ));
        stop.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin | pinned_top);
        frame.pane.addSubview(&stop);
        let diagram_left = (pane.width - DIAGRAM_SIDE) / 2.0;
        let diagram_frame = from_top(pane, diagram_left, DIAGRAM_TOP, NSSize::new(DIAGRAM_SIDE, DIAGRAM_SIDE));
        let meta_top = DIAGRAM_TOP + DIAGRAM_SIDE + META_GAP;
        let (engine, path) = meta_line(marker, data.scan.threads, palette, meta_top);
        frame.pane.addSubview(&engine);
        frame.pane.addSubview(&path);
        let engine_size = engine.frame().size;
        if data.scan.full_first_scan {
            let notice = label(
                marker,
                FULL_FIRST_SCAN,
                &interface_font(13.0, 400.0),
                &tokens.mute.ns_color(),
                NSLineBreakMode::ByWordWrapping,
            );
            let notice_size = notice.fittingSize();
            notice.setFrame(from_top(
                pane,
                (pane.width - notice_size.width) / 2.0,
                meta_top + engine_size.height + NOTICE_GAP,
                notice_size,
            ));
            notice.setAutoresizingMask(
                pinned_top | NSAutoresizingMaskOptions::ViewMinXMargin | NSAutoresizingMaskOptions::ViewMaxXMargin,
            );
            frame.pane.addSubview(&notice);
        }
        let screen = Self {
            frame,
            stop,
            engine,
            path,
            diagram_frame,
            probes: Probes {
                window_background: [WINDOW.width - 8.0, WINDOW.height - 8.0],
                sidebar: Some([8.0, 8.0]),
                accent_button: None,
            },
        };
        screen.show_progress(&data.scan);
        screen
    }

    pub fn diagram_state(data: &ShellData, palette: &ShellPalette) -> DiagramState {
        let side = DIAGRAM_SIDE as f32;
        let mut state = DiagramState::new(
            data.tree.clone(),
            DiagramMode::Sunburst,
            DIAGRAM,
            palette.tokens,
            side,
            side,
        );
        state.set_sweep(data.scan.sweep);
        state.center = Some(center_lines(&data.scan));
        state
    }

    pub fn show_progress(&self, scan: &ScanStatus) {
        let full = NSString::from_str(&scan.current);
        self.path.setStringValue(&full);
        self.path.setToolTip(Some(&full));
        self.path.setAccessibilityLabel(Some(&full));
        self.engine
            .setStringValue(&NSString::from_str(&engine_line(scan.threads)));
    }
}

fn meta_line(
    marker: MainThreadMarker,
    threads: usize,
    palette: &ShellPalette,
    top: f64,
) -> (Retained<NSTextField>, Retained<NSTextField>) {
    let pane = WindowFrame::pane_size();
    let dim = palette.tokens.dim.ns_color();
    let engine = measure_label(marker, &engine_line(threads), &mono_font(12.0, 400.0), &dim);
    let engine_size = engine.fittingSize();
    let left = (pane.width - engine_size.width - PATH_GAP - PATH_WIDTH) / 2.0;
    let centered = NSAutoresizingMaskOptions::ViewMinYMargin
        | NSAutoresizingMaskOptions::ViewMinXMargin
        | NSAutoresizingMaskOptions::ViewMaxXMargin;
    engine.setFrame(from_top(pane, left, top, engine_size));
    engine.setAutoresizingMask(centered);
    let path = label(
        marker,
        "",
        &mono_font(12.0, 400.0),
        &dim,
        NSLineBreakMode::ByTruncatingMiddle,
    );
    path.setFrame(from_top(
        pane,
        left + engine_size.width + PATH_GAP,
        top,
        NSSize::new(PATH_WIDTH, engine_size.height),
    ));
    path.setAutoresizingMask(centered);
    (engine, path)
}

pub(crate) fn center_lines(scan: &ScanStatus) -> [String; 3] {
    [
        WALKED.to_owned(),
        format_size(scan.bytes),
        format!("{} {FILES_MILLIONS}", format_count_millions(scan.files)),
    ]
}

fn engine_line(threads: usize) -> String {
    format!("{SCAN_ENGINE} · {}", format_count(threads as u64, &THREADS))
}

fn format_count_millions(files: u64) -> String {
    let text = format!("{:.2}", files as f64 / MILLION);
    text.trim_end_matches('0').trim_end_matches('.').replace('.', ",")
}
