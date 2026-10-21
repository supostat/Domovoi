use std::cell::RefCell;
use std::ops::Range;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{NSAccessibility, NSButton, NSLineBreakMode, NSMenu, NSMenuItem, NSResponder, NSTextField, NSView};
use objc2_foundation::{NSInteger, NSPoint, NSRect, NSSize, NSString};

use crate::crumb::Crumb;
use crate::label::label;
use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;
use crate::strings::SHOW_IN_FINDER;
use crate::ui_font::interface_font;

const SEPARATOR: &str = "›";
const ELLIPSIS: &str = "…";
const COLLAPSED_JOINER: &str = " › ";
const SPACING: f64 = 6.0;
const FONT_SIZE: f64 = 13.0;
const LEVEL_WEIGHT: f64 = 500.0;
const CURRENT_WEIGHT: f64 = 600.0;
const SEPARATOR_WEIGHT: f64 = 400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shown {
    first: bool,
    tail: usize,
}

impl Shown {
    fn hidden(self) -> Range<usize> {
        usize::from(self.first)..self.tail
    }
}

#[derive(Clone, Copy, Debug)]
enum Item {
    Level(usize),
    Ellipsis,
}

#[derive(Debug)]
pub struct BreadcrumbsState {
    path: RefCell<Vec<Crumb>>,
    levels: Vec<Retained<NSButton>>,
    ellipsis: Retained<NSButton>,
    separators: Vec<Retained<NSTextField>>,
    actions: Retained<ShellActions>,
    shown: RefCell<Option<Shown>>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[ivars = BreadcrumbsState]
    #[derive(Debug)]
    pub struct Breadcrumbs;

    impl Breadcrumbs {
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.lay_out();
        }
    }
);

impl Breadcrumbs {
    pub fn new(
        marker: MainThreadMarker,
        frame: NSRect,
        path: &[Crumb],
        palette: &ShellPalette,
        actions: &ShellActions,
    ) -> Retained<Self> {
        let levels = path
            .iter()
            .enumerate()
            .map(|(position, crumb)| level_button(marker, crumb, position + 1 == path.len(), actions))
            .collect();
        let separators = (1..path.len())
            .map(|_| {
                label(
                    marker,
                    SEPARATOR,
                    &interface_font(FONT_SIZE, SEPARATOR_WEIGHT),
                    &palette.tokens.dim.ns_color(),
                    NSLineBreakMode::ByClipping,
                )
            })
            .collect();
        let this = Self::alloc(marker).set_ivars(BreadcrumbsState {
            path: RefCell::new(path.to_vec()),
            levels,
            ellipsis: text_button(marker, ELLIPSIS, LEVEL_WEIGHT),
            separators,
            actions: actions.retain(),
            shown: RefCell::new(None),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.lay_out();
        this.show_nodes(path);
        this
    }

    pub fn show_nodes(&self, path: &[Crumb]) {
        let state = self.ivars();
        let marker = MainThreadMarker::from(self);
        for (button, crumb) in state.levels.iter().zip(path) {
            button.setTag(tag(crumb.node));
            unsafe { button.setMenu(Some(&crumb_menu(marker, crumb, &state.actions))) };
        }
        state.path.replace(path.to_vec());
        state.describe_collapsed(marker, state.current());
    }

    pub fn buttons(&self) -> Vec<Retained<NSButton>> {
        let state = self.ivars();
        state
            .items(state.current())
            .into_iter()
            .map(|item| state.button(item).clone())
            .collect()
    }

    pub fn collapsed(&self) -> Vec<Crumb> {
        let state = self.ivars();
        state.path.borrow()[state.current().hidden()].to_vec()
    }

    fn lay_out(&self) {
        let state = self.ivars();
        let bounds = self.bounds().size;
        let shown = state.fitting(bounds.width);
        let items = state.items(shown);
        if state.shown.replace(Some(shown)) != Some(shown) {
            for view in &self.subviews() {
                view.removeFromSuperview();
            }
            for (position, item) in items.iter().enumerate() {
                if position > 0 {
                    self.addSubview(&state.separators[position - 1]);
                }
                self.addSubview(state.button(*item));
            }
            state.describe_collapsed(MainThreadMarker::from(self), shown);
        }
        let mut x = 0.0;
        for (position, item) in items.iter().enumerate() {
            if position > 0 {
                let separator = &state.separators[position - 1];
                x += SPACING;
                place(separator, x, separator.frame().size.width, bounds.height);
                x += separator.frame().size.width + SPACING;
            }
            let button = state.button(*item);
            let wanted = state.width(*item);
            let width = if position + 1 == items.len() {
                wanted.min(bounds.width - x).max(0.0)
            } else {
                wanted
            };
            place(button, x, width, bounds.height);
            x += width;
        }
    }
}

impl BreadcrumbsState {
    fn current(&self) -> Shown {
        self.shown.borrow().expect("the crumbs are laid out on creation")
    }

    fn candidates(&self) -> Vec<Shown> {
        let count = self.levels.len();
        let mut candidates = vec![Shown { first: true, tail: 1 }];
        candidates.extend((2..count).map(|tail| Shown { first: true, tail }));
        if count > 1 {
            candidates.push(Shown {
                first: false,
                tail: count - 1,
            });
        }
        candidates
    }

    fn fitting(&self, width: f64) -> Shown {
        let candidates = self.candidates();
        let last = *candidates.last().expect("a path has at least its root");
        candidates
            .into_iter()
            .find(|shown| self.width_of(&self.items(*shown)) <= width)
            .unwrap_or(last)
    }

    fn items(&self, shown: Shown) -> Vec<Item> {
        let mut items = Vec::new();
        if shown.first {
            items.push(Item::Level(0));
        }
        if !shown.hidden().is_empty() {
            items.push(Item::Ellipsis);
        }
        items.extend((shown.tail..self.levels.len()).map(Item::Level));
        items
    }

    fn width_of(&self, items: &[Item]) -> f64 {
        let content: f64 = items.iter().map(|item| self.width(*item)).sum();
        let separator = self
            .separators
            .first()
            .map_or(0.0, |separator| separator.frame().size.width);
        content + items.len().saturating_sub(1) as f64 * (separator + 2.0 * SPACING)
    }

    fn width(&self, item: Item) -> f64 {
        self.button(item).fittingSize().width
    }

    fn button(&self, item: Item) -> &Retained<NSButton> {
        match item {
            Item::Level(level) => &self.levels[level],
            Item::Ellipsis => &self.ellipsis,
        }
    }

    fn describe_collapsed(&self, marker: MainThreadMarker, shown: Shown) {
        let path = self.path.borrow();
        let collapsed = &path[shown.hidden()];
        let joined = collapsed
            .iter()
            .map(|level| level.title.as_str())
            .collect::<Vec<_>>()
            .join(COLLAPSED_JOINER);
        let joined = NSString::from_str(&joined);
        self.ellipsis.setToolTip(Some(&joined));
        self.ellipsis.setAccessibilityLabel(Some(&joined));
        unsafe {
            self.ellipsis
                .setMenu(Some(&collapsed_menu(marker, collapsed, &self.actions)));
        }
    }
}

fn place(view: &NSView, x: f64, width: f64, height: f64) {
    let natural = view.fittingSize().height;
    view.setFrame(NSRect::new(
        NSPoint::new(x, (height - natural) / 2.0),
        NSSize::new(width, natural),
    ));
}

fn text_button(marker: MainThreadMarker, title: &str, weight: f64) -> Retained<NSButton> {
    let button = unsafe { NSButton::buttonWithTitle_target_action(&NSString::from_str(title), None, None, marker) };
    button.setBordered(false);
    button.setFont(Some(&interface_font(FONT_SIZE, weight)));
    button.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    button
}

fn level_button(marker: MainThreadMarker, crumb: &Crumb, current: bool, actions: &ShellActions) -> Retained<NSButton> {
    let button = text_button(
        marker,
        &crumb.title,
        if current { CURRENT_WEIGHT } else { LEVEL_WEIGHT },
    );
    let full = NSString::from_str(&crumb.title);
    button.setToolTip(Some(&full));
    button.setAccessibilityLabel(Some(&full));
    unsafe {
        button.setTarget(Some(actions.target()));
        button.setAction(Some(sel!(focusNode:)));
    }
    button
}

fn crumb_menu(marker: MainThreadMarker, crumb: &Crumb, actions: &ShellActions) -> Retained<NSMenu> {
    let menu = NSMenu::new(marker);
    menu.addItem(&item(
        marker,
        SHOW_IN_FINDER,
        sel!(showNodeInFinder:),
        crumb.node,
        actions,
    ));
    menu
}

fn collapsed_menu(marker: MainThreadMarker, collapsed: &[Crumb], actions: &ShellActions) -> Retained<NSMenu> {
    let menu = NSMenu::new(marker);
    for level in collapsed {
        menu.addItem(&item(marker, &level.title, sel!(focusNode:), level.node, actions));
    }
    menu
}

fn item(
    marker: MainThreadMarker,
    title: &str,
    selector: Sel,
    node: usize,
    actions: &ShellActions,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(marker),
            &NSString::from_str(title),
            Some(selector),
            &NSString::new(),
        )
    };
    unsafe { item.setTarget(Some(actions.target())) };
    item.setTag(tag(node));
    item
}

fn tag(node: usize) -> NSInteger {
    NSInteger::try_from(node).expect("a diagram tree fits in NSInteger")
}
