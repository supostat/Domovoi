use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSLineBreakMode, NSMenu, NSMenuItem, NSStackView, NSUserInterfaceLayoutOrientation,
};
use objc2_foundation::{NSInteger, NSString};

use crate::crumb::Crumb;
use crate::label::label;
use crate::shell_actions::ShellActions;
use crate::shell_palette::ShellPalette;
use crate::strings::SHOW_IN_FINDER;
use crate::ui_font::interface_font;

const COLLAPSE_AFTER: usize = 4;
const SEPARATOR: &str = "›";
const ELLIPSIS: &str = "…";
const SPACING: f64 = 6.0;

#[derive(Debug)]
pub struct Breadcrumbs {
    pub stack: Retained<NSStackView>,
    pub buttons: Vec<Retained<NSButton>>,
    pub collapsed: Vec<Crumb>,
}

impl Breadcrumbs {
    pub fn build(marker: MainThreadMarker, path: &[Crumb], palette: &ShellPalette, actions: &ShellActions) -> Self {
        let stack = NSStackView::new(marker);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        stack.setSpacing(SPACING);
        let (shown, collapsed): (Vec<Option<&Crumb>>, Vec<Crumb>) = if path.len() > COLLAPSE_AFTER {
            let middle = path[1..path.len() - 2].to_vec();
            (
                vec![
                    Some(&path[0]),
                    None,
                    Some(&path[path.len() - 2]),
                    Some(&path[path.len() - 1]),
                ],
                middle,
            )
        } else {
            (path.iter().map(Some).collect(), Vec::new())
        };
        let mut buttons = Vec::new();
        for (position, crumb) in shown.iter().enumerate() {
            if position > 0 {
                let separator = label(
                    marker,
                    SEPARATOR,
                    &interface_font(13.0, 400.0),
                    &palette.tokens.dim.ns_color(),
                    NSLineBreakMode::ByClipping,
                );
                stack.addArrangedSubview(&separator);
            }
            let title = crumb.map_or(ELLIPSIS, |crumb| crumb.title.as_str());
            let button =
                unsafe { NSButton::buttonWithTitle_target_action(&NSString::from_str(title), None, None, marker) };
            button.setBordered(false);
            button.setFont(Some(&interface_font(
                13.0,
                if position + 1 == shown.len() { 600.0 } else { 500.0 },
            )));
            button.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
            let full = match crumb {
                Some(crumb) => crumb.title.clone(),
                None => collapsed
                    .iter()
                    .map(|level| level.title.as_str())
                    .collect::<Vec<_>>()
                    .join(" › "),
            };
            let full = NSString::from_str(&full);
            button.setToolTip(Some(&full));
            button.setAccessibilityLabel(Some(&full));
            let menu = match crumb {
                Some(crumb) => {
                    unsafe {
                        button.setTarget(Some(actions.target()));
                        button.setAction(Some(sel!(focusNode:)));
                    }
                    button.setTag(tag(crumb.node));
                    crumb_menu(marker, crumb, actions)
                }
                None => collapsed_menu(marker, &collapsed, actions),
            };
            unsafe { button.setMenu(Some(&menu)) };
            stack.addArrangedSubview(&button);
            buttons.push(button);
        }
        Self {
            stack,
            buttons,
            collapsed,
        }
    }
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
