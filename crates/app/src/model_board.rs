use domovoi_theme::{RADII, format_size};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSGridCell, NSGridCellPlacement, NSGridView, NSLineBreakMode, NSView};
use objc2_foundation::{NSArray, NSPoint, NSRange, NSRect, NSSize};

use crate::button_kind::ButtonKind;
use crate::fill_view::{FillStyle, FillView};
use crate::label::label;
use crate::model_screen_model::ModelScreenModel;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_palette::ShellPalette;
use crate::strings::{CHANGE, CHOOSE_FILE, DOWNLOAD_SOURCE, ORPHANS, TO_TRASH, UNLOAD_AFTER_IDLE, UNLOAD_EXPLANATION};
use crate::ui_font::{interface_font, mono_font};

const PADDING: f64 = 16.0;
const ROW_SPACING: f64 = 12.0;
const COLUMN_SPACING: f64 = 16.0;
const CELL_GAP: f64 = 10.0;
const BUTTON_HEIGHT: f64 = 26.0;
const PATH_SHARE: f64 = 0.6;
const MENU_MARK: &str = "▾";

pub(crate) fn model_board(
    marker: MainThreadMarker,
    model: &ModelScreenModel,
    width: f64,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> (Retained<NSView>, Retained<ShellButton>) {
    let tokens = palette.tokens;
    let inner = width - 2.0 * PADDING;
    let text = |value: &str| {
        label(
            marker,
            value,
            &interface_font(14.0, 400.0),
            &tokens.text.ns_color(),
            NSLineBreakMode::ByTruncatingTail,
        )
        .into_super()
        .into_super()
    };
    let button = |title: &str, kind: ButtonKind| {
        ShellButton::new(marker, title, NSPoint::ZERO, BUTTON_HEIGHT, kind.small_style(palette))
    };
    let unload = button(&format!("{} {MENU_MARK}", model.unload.title()), ButtonKind::Plain);
    unload.on_click(actions, sel!(chooseUnloadDelay:));
    let explanation = label(
        marker,
        UNLOAD_EXPLANATION,
        &interface_font(12.5, 400.0),
        &tokens.dim.ns_color(),
        NSLineBreakMode::ByWordWrapping,
    );
    explanation.setPreferredMaxLayoutWidth(inner);
    let change = button(CHANGE, ButtonKind::Ghost);
    change.on_click(actions, sel!(changeSource:));
    let choose = button(CHOOSE_FILE, ButtonKind::Ghost);
    choose.on_click(actions, sel!(chooseModelFile:));
    let source = label(
        marker,
        &model.source,
        &mono_font(12.5, 400.0),
        &tokens.mute.ns_color(),
        NSLineBreakMode::ByClipping,
    );
    let mut rows: Vec<[Retained<NSView>; 2]> = vec![
        [
            text(UNLOAD_AFTER_IDLE),
            pinned(unload.clone().into_super().into_super().into_super()),
        ],
        [
            explanation.into_super().into_super(),
            NSGridCell::emptyContentView(marker),
        ],
        [
            text(DOWNLOAD_SOURCE),
            row_cell(
                marker,
                &[
                    &source,
                    &change.into_super().into_super().into_super(),
                    &choose.into_super().into_super().into_super(),
                ],
            ),
        ],
    ];
    let mut merged = vec![1];
    if !model.orphans.is_empty() {
        merged.push(rows.len());
        rows.push([text(ORPHANS), NSGridCell::emptyContentView(marker)]);
        rows.extend(orphan_rows(marker, model, inner, palette, actions));
    }
    let trailing_width = rows
        .iter()
        .map(|[_, trailing]| trailing.fittingSize().width)
        .fold(0.0, f64::max);
    let grid_rows: Vec<Retained<NSArray<NSView>>> =
        rows.into_iter().map(|row| NSArray::from_retained_slice(&row)).collect();
    let grid = NSGridView::gridViewWithViews(&NSArray::from_retained_slice(&grid_rows), marker);
    grid.setRowSpacing(ROW_SPACING);
    grid.setColumnSpacing(COLUMN_SPACING);
    grid.setYPlacement(NSGridCellPlacement::Center);
    grid.columnAtIndex(0)
        .setWidth((inner - trailing_width - COLUMN_SPACING).floor());
    grid.columnAtIndex(1).setXPlacement(NSGridCellPlacement::Trailing);
    for row in merged {
        grid.rowAtIndex(isize::try_from(row).expect("rows fit in isize"))
            .mergeCellsInRange(NSRange::new(0, 2));
    }
    let height = grid.fittingSize().height.ceil();
    let board = FillView::new(
        marker,
        NSRect::new(NSPoint::ZERO, NSSize::new(width, height + 2.0 * PADDING)),
        FillStyle {
            fill: Some(tokens.bg_2.ns_color()),
            border: Some(palette.line_soft()),
            radius: f64::from(RADII.l),
            dashed: false,
        },
    );
    grid.setFrame(NSRect::new(NSPoint::new(PADDING, PADDING), NSSize::new(inner, height)));
    board.addSubview(&grid);
    (board.into_super(), unload)
}

fn orphan_rows(
    marker: MainThreadMarker,
    model: &ModelScreenModel,
    inner: f64,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> Vec<[Retained<NSView>; 2]> {
    let tokens = palette.tokens;
    let font = mono_font(12.0, 400.0);
    model
        .orphans
        .iter()
        .enumerate()
        .map(|(position, orphan)| {
            let path = label(
                marker,
                &orphan.path,
                &font,
                &tokens.dim.ns_color(),
                NSLineBreakMode::ByTruncatingMiddle,
            );
            let natural = path.fittingSize();
            path.setFrameSize(NSSize::new(
                natural.width.min(inner * PATH_SHARE).ceil(),
                natural.height,
            ));
            let detail = label(
                marker,
                &format!("{} · {}", format_size(orphan.bytes), orphan.reason),
                &font,
                &tokens.dim.ns_color(),
                NSLineBreakMode::ByClipping,
            );
            let trash = ShellButton::new(
                marker,
                TO_TRASH,
                NSPoint::ZERO,
                BUTTON_HEIGHT,
                ButtonKind::Ghost.small_style(palette),
            );
            trash.setTag(isize::try_from(position).expect("orphans fit in isize"));
            trash.on_click(actions, sel!(trashOrphan:));
            [
                pinned(path.into_super().into_super()),
                row_cell(marker, &[&detail, &trash.into_super().into_super().into_super()]),
            ]
        })
        .collect()
}

fn row_cell(marker: MainThreadMarker, views: &[&NSView]) -> Retained<NSView> {
    let sizes: Vec<NSSize> = views
        .iter()
        .map(|view| {
            let frame = view.frame().size;
            let fitting = view.fittingSize();
            NSSize::new(
                frame.width.max(fitting.width).ceil(),
                frame.height.max(fitting.height).ceil(),
            )
        })
        .collect();
    let height = sizes.iter().map(|size| size.height).fold(0.0, f64::max);
    let width = sizes.iter().map(|size| size.width).sum::<f64>() + CELL_GAP * (views.len() - 1) as f64;
    let cell = NSView::initWithFrame(
        NSView::alloc(marker),
        NSRect::new(NSPoint::ZERO, NSSize::new(width, height)),
    );
    let mut left = 0.0;
    for (view, size) in views.iter().zip(sizes) {
        view.setFrame(NSRect::new(NSPoint::new(left, (height - size.height) / 2.0), size));
        cell.addSubview(view);
        left += size.width + CELL_GAP;
    }
    pinned(cell)
}

fn pinned(view: Retained<NSView>) -> Retained<NSView> {
    let size = view.frame().size;
    view.setTranslatesAutoresizingMaskIntoConstraints(false);
    view.widthAnchor().constraintEqualToConstant(size.width).setActive(true);
    view.heightAnchor()
        .constraintEqualToConstant(size.height)
        .setActive(true);
    view
}
