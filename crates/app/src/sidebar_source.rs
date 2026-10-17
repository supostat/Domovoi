use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSControlTextEditingDelegate, NSFontAttributeName, NSForegroundColorAttributeName,
    NSKernAttributeName, NSLineBreakMode, NSOutlineView, NSOutlineViewDataSource, NSOutlineViewDelegate, NSTableColumn,
    NSTableViewDelegate, NSView,
};
use objc2_core_foundation::CGFloat;
use objc2_foundation::{NSAttributedString, NSDictionary, NSInteger, NSNumber, NSObject, NSString};

use crate::label::label;
use crate::shell_palette::ShellPalette;
use crate::sidebar_entry::SidebarEntry;
use crate::sidebar_item_view::SidebarItemView;
use crate::ui_font::interface_font;

const ROW_HEIGHT: CGFloat = 28.0;
const GROUP_HEIGHT: CGFloat = 26.0;
const GROUP_INSET: f64 = 10.0;
const GROUP_FONT_SIZE: f64 = 10.5;
const GROUP_TRACKING: f64 = 0.1;

#[derive(Debug)]
pub struct SidebarModel {
    pub groups: Vec<SidebarEntry>,
    pub palette: ShellPalette,
    pub width: f64,
    keys: RefCell<Vec<Retained<NSString>>>,
}

impl SidebarModel {
    pub fn new(groups: Vec<SidebarEntry>, palette: ShellPalette, width: f64) -> Self {
        Self {
            groups,
            palette,
            width,
            keys: RefCell::new(Vec::new()),
        }
    }

    fn key(&self, group: usize, child: Option<usize>) -> Retained<NSString> {
        let text = child.map_or_else(|| format!("{group}"), |child| format!("{group}.{child}"));
        let key = NSString::from_str(&text);
        self.keys.borrow_mut().push(key.clone());
        key
    }

    fn locate(item: &AnyObject) -> (usize, Option<usize>) {
        let key = item
            .downcast_ref::<NSString>()
            .expect("sidebar items are strings")
            .to_string();
        let mut parts = key
            .split('.')
            .map(|part| part.parse::<usize>().expect("sidebar keys are numeric"));
        (parts.next().expect("a key names a group"), parts.next())
    }

    fn entry(&self, item: &AnyObject) -> &SidebarEntry {
        match Self::locate(item) {
            (group, None) => &self.groups[group],
            (group, Some(child)) => &self.groups[group].children[child],
        }
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SidebarModel]
    #[derive(Debug)]
    pub struct SidebarSource;

    unsafe impl NSObjectProtocol for SidebarSource {}

    unsafe impl NSOutlineViewDataSource for SidebarSource {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _outline: &NSOutlineView, item: Option<&AnyObject>) -> NSInteger {
            let model = self.ivars();
            let count = match item {
                None => model.groups.len(),
                Some(item) => match SidebarModel::locate(item) {
                    (group, None) => model.groups[group].children.len(),
                    (_, Some(_)) => 0,
                },
            };
            NSInteger::try_from(count).expect("the sidebar is small")
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _outline: &NSOutlineView, index: NSInteger, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let model = self.ivars();
            let index = usize::try_from(index).expect("AppKit asks for a valid child");
            let key = match item {
                None => model.key(index, None),
                Some(item) => model.key(SidebarModel::locate(item).0, Some(index)),
            };
            key.into()
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _outline: &NSOutlineView, item: &AnyObject) -> bool {
            SidebarModel::locate(item).1.is_none()
        }
    }

    unsafe impl NSControlTextEditingDelegate for SidebarSource {}

    unsafe impl NSTableViewDelegate for SidebarSource {}

    unsafe impl NSOutlineViewDelegate for SidebarSource {
        #[unsafe(method(outlineView:isGroupItem:))]
        fn group_item(&self, _outline: &NSOutlineView, item: &AnyObject) -> bool {
            SidebarModel::locate(item).1.is_none()
        }

        #[unsafe(method(outlineView:heightOfRowByItem:))]
        fn row_height(&self, _outline: &NSOutlineView, item: &AnyObject) -> CGFloat {
            if SidebarModel::locate(item).1.is_none() {
                GROUP_HEIGHT
            } else {
                ROW_HEIGHT
            }
        }

        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(
            &self,
            _outline: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            let model = self.ivars();
            let marker = MainThreadMarker::from(self);
            let entry = model.entry(item);
            let view = if SidebarModel::locate(item).1.is_none() {
                let field = label(
                    marker,
                    "",
                    &interface_font(GROUP_FONT_SIZE, 600.0),
                    &model.palette.tokens.dim.ns_color(),
                    NSLineBreakMode::ByTruncatingTail,
                );
                field.setAttributedStringValue(&group_title(&entry.title, &model.palette));
                field.setAccessibilityLabel(Some(&NSString::from_str(&entry.title)));
                let size = field.fittingSize();
                field.setFrameOrigin(objc2_foundation::NSPoint::new(
                    GROUP_INSET,
                    (GROUP_HEIGHT - size.height) / 2.0,
                ));
                field.into_super().into_super()
            } else {
                SidebarItemView::build(marker, entry, model.width, ROW_HEIGHT, &model.palette)
                    .view
                    .into_super()
            };
            Some(view)
        }
    }
);

impl SidebarSource {
    pub fn new(marker: MainThreadMarker, model: SidebarModel) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(model);
        unsafe { msg_send![super(this), init] }
    }
}

fn group_title(title: &str, palette: &ShellPalette) -> Retained<NSAttributedString> {
    let font = interface_font(GROUP_FONT_SIZE, 600.0);
    let color = palette.tokens.dim.ns_color();
    let tracking = NSNumber::new_f64(GROUP_FONT_SIZE * GROUP_TRACKING);
    let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName, NSKernAttributeName] };
    let values: [&AnyObject; 3] = [font.as_ref(), color.as_ref(), tracking.as_ref()];
    let attributes = NSDictionary::from_slices(&keys, &values);
    unsafe {
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(&title.to_uppercase()),
            Some(&attributes),
        )
    }
}
