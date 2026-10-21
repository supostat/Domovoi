use std::cell::{Cell, RefCell};

use domovoi_theme::format_size;
use objc2::Message;
use objc2::rc::Retained;
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSCollectionView, NSLineBreakMode, NSScrollView, NSTextField};
use objc2_foundation::{NSArray, NSEdgeInsets, NSPoint, NSRect, NSSize, NSString};

use crate::button_style::ButtonStyle;
use crate::card_item::CardItem;
use crate::cards_layout::CardsLayout;
use crate::cards_model::CardsModel;
use crate::cards_source::{CARD_ITEM, CardsSource};
use crate::empty_state::empty_state;
use crate::label::{label, retext};
use crate::pane_head::pane_head;
use crate::probes::Probes;
use crate::recommendations_model::RecommendationsModel;
use crate::section::Section;
use crate::shell_actions::ShellActions;
use crate::shell_button::ShellButton;
use crate::shell_data::ShellData;
use crate::shell_layout::{PANE_PADDING, WINDOW, from_top};
use crate::shell_palette::ShellPalette;
use crate::strings::{NO_RECOMMENDATIONS, TO_TRAY};
use crate::ui_font::interface_font;
use crate::window_frame::WindowFrame;

const HEAD_HEIGHT: f64 = 76.0;
const TITLE_TOP: f64 = 17.0;
const SPLIT_GAP: f64 = 4.0;
const BUTTON_HEIGHT: f64 = 34.0;
const LIST_INSET: f64 = 16.0;
const CARD_SPACING: f64 = 10.0;

#[derive(Debug)]
pub struct RecommendationsScreen {
    pub frame: WindowFrame,
    pub total: Retained<NSTextField>,
    pub split: Retained<NSTextField>,
    pub send: Retained<ShellButton>,
    pub collection: Option<(Retained<NSCollectionView>, Retained<CardsSource>)>,
    pub probes: Probes,
}

impl RecommendationsScreen {
    pub fn build(marker: MainThreadMarker, data: &ShellData, palette: &ShellPalette, actions: &ShellActions) -> Self {
        let tokens = palette.tokens;
        let model = &data.care.recommendations;
        let frame = WindowFrame::build(marker, data, Section::Recommendations, palette, actions);
        let pane = WindowFrame::pane_size();
        let head = pane_head(marker, &frame.pane, HEAD_HEIGHT, palette);
        let head_size = head.frame().size;
        let pinned = NSAutoresizingMaskOptions::ViewMinYMargin;
        let total = label(
            marker,
            "",
            &interface_font(17.0, 600.0),
            &tokens.text.ns_color(),
            NSLineBreakMode::ByTruncatingTail,
        );
        total.setAutoresizingMask(pinned);
        head.addSubview(&total);
        let split = label(
            marker,
            "",
            &interface_font(12.5, 400.0),
            &tokens.dim.ns_color(),
            NSLineBreakMode::ByTruncatingTail,
        );
        split.setAutoresizingMask(pinned);
        head.addSubview(&split);
        let send = ShellButton::new(
            marker,
            "",
            NSPoint::ZERO,
            BUTTON_HEIGHT,
            ButtonStyle {
                fill: Some(tokens.wax.ns_color()),
                border: None,
                text: tokens.wax_ink.ns_color(),
                font: interface_font(13.0, 600.0),
            },
        );
        send.on_click(actions, sel!(sendCardsToTray:));
        let send_size = send.frame().size;
        send.setFrame(from_top(
            head_size,
            pane.width - PANE_PADDING - send_size.width,
            (HEAD_HEIGHT - BUTTON_HEIGHT) / 2.0,
            send_size,
        ));
        send.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin | pinned);
        head.addSubview(&send);
        let body = NSRect::new(NSPoint::ZERO, NSSize::new(pane.width, pane.height - HEAD_HEIGHT));
        let collection = if model.cards.is_empty() {
            let empty = empty_state(marker, body.size, NO_RECOMMENDATIONS, None, palette);
            frame.pane.addSubview(&empty);
            None
        } else {
            Some(cards(marker, &frame, body, palette, actions))
        };
        let mut screen = Self {
            frame,
            total,
            split,
            send,
            collection,
            probes: Probes {
                window_background: [WINDOW.width - 8.0, WINDOW.height - 8.0],
                sidebar: Some([8.0, 8.0]),
                accent_button: None,
            },
        };
        screen.show(model);
        let send_frame = screen.send.frame();
        screen.probes.accent_button = Some([
            WindowFrame::pane_origin_x() + send_frame.origin.x,
            (HEAD_HEIGHT - BUTTON_HEIGHT) / 2.0,
            send_frame.size.width,
            send_frame.size.height,
        ]);
        screen
    }

    pub fn update(&self, data: &ShellData) {
        self.frame.update(data, Section::Recommendations);
        self.show(&data.care.recommendations);
    }

    pub fn show_selection(&self, model: &RecommendationsModel) {
        self.send.retitle(&send_title(model));
        self.send.setEnabled(model.selected_total() > 0);
    }

    fn show(&self, model: &RecommendationsModel) {
        let head = NSSize::new(WindowFrame::pane_size().width, HEAD_HEIGHT);
        retext(&self.total, &model.total_line());
        let total_size = self.total.frame().size;
        self.total
            .setFrameOrigin(from_top(head, PANE_PADDING, TITLE_TOP, total_size).origin);
        retext(&self.split, &model.split_line());
        self.split.setFrameOrigin(
            from_top(
                head,
                PANE_PADDING,
                TITLE_TOP + total_size.height + SPLIT_GAP,
                self.split.frame().size,
            )
            .origin,
        );
        self.show_selection(model);
        if let Some((collection, source)) = &self.collection
            && source.show(&model.cards)
        {
            collection.reloadData();
        }
    }
}

fn send_title(model: &RecommendationsModel) -> String {
    format!("{TO_TRAY} · {}", format_size(model.selected_total()))
}

fn cards(
    marker: MainThreadMarker,
    frame: &WindowFrame,
    body: NSRect,
    palette: &ShellPalette,
    actions: &ShellActions,
) -> (Retained<NSCollectionView>, Retained<CardsSource>) {
    let source = CardsSource::new(
        marker,
        CardsModel {
            cards: RefCell::new(Vec::new()),
            inset: PANE_PADDING,
            palette: *palette,
            actions: actions.retain(),
            heights: RefCell::new(None),
            measurements: Cell::new(0),
        },
    );
    let layout = CardsLayout::new(marker);
    layout.setMinimumLineSpacing(CARD_SPACING);
    layout.setSectionInset(NSEdgeInsets {
        top: LIST_INSET,
        left: PANE_PADDING,
        bottom: LIST_INSET,
        right: PANE_PADDING,
    });
    let collection = NSCollectionView::initWithFrame(NSCollectionView::alloc(marker), body);
    collection.setCollectionViewLayout(Some(&layout));
    collection.setBackgroundColors(Some(&NSArray::from_retained_slice(&[palette.tokens.bg.ns_color()])));
    unsafe {
        collection.registerClass_forItemWithIdentifier(Some(CardItem::class()), &NSString::from_str(CARD_ITEM));
        collection.setDataSource(Some(objc2::runtime::ProtocolObject::from_ref(&*source)));
        collection.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(&*source)));
    }
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(marker), body);
    scroll.setDrawsBackground(false);
    scroll.setHasVerticalScroller(true);
    scroll.setDocumentView(Some(&collection));
    scroll.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    frame.pane.addSubview(&scroll);
    (collection, source)
}
