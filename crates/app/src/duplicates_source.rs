use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSControlTextEditingDelegate, NSOutlineView, NSOutlineViewDataSource, NSOutlineViewDelegate, NSTableColumn,
    NSTableViewDelegate, NSView,
};
use objc2_core_foundation::CGFloat;
use objc2_foundation::{NSInteger, NSObject, NSString};

use crate::duplicate_rows::{GROUP_GAP, GROUP_HEIGHT, MEMBER_HEIGHT, group_row, member_row};
use crate::duplicates_source_model::DuplicatesSourceModel;

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DuplicatesSourceModel]
    #[derive(Debug)]
    pub struct DuplicatesSource;

    unsafe impl NSObjectProtocol for DuplicatesSource {}

    unsafe impl NSOutlineViewDataSource for DuplicatesSource {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn child_count(&self, _outline: &NSOutlineView, item: Option<&AnyObject>) -> NSInteger {
            let model = self.ivars();
            let count = match item.map(locate) {
                None => model.groups.len(),
                Some((group, None)) => model.groups[group].copies.len() + 1,
                Some((_, Some(_))) => 0,
            };
            NSInteger::try_from(count).expect("groups are few")
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _outline: &NSOutlineView, index: NSInteger, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let index = usize::try_from(index).expect("AppKit asks for a valid child");
            let key = match item.map(locate) {
                None => format!("{index}"),
                Some((group, _)) => format!("{group}.{index}"),
            };
            let key = NSString::from_str(&key);
            self.ivars().keys.borrow_mut().push(key.clone());
            key.into()
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _outline: &NSOutlineView, item: &AnyObject) -> bool {
            locate(item).1.is_none()
        }
    }

    unsafe impl NSControlTextEditingDelegate for DuplicatesSource {}

    unsafe impl NSTableViewDelegate for DuplicatesSource {}

    unsafe impl NSOutlineViewDelegate for DuplicatesSource {
        #[unsafe(method(outlineView:shouldSelectItem:))]
        fn should_select(&self, _outline: &NSOutlineView, _item: &AnyObject) -> bool {
            false
        }

        #[unsafe(method(outlineView:shouldShowOutlineCellForItem:))]
        fn shows_disclosure(&self, _outline: &NSOutlineView, _item: &AnyObject) -> bool {
            false
        }

        #[unsafe(method(outlineView:heightOfRowByItem:))]
        fn row_height(&self, _outline: &NSOutlineView, item: &AnyObject) -> CGFloat {
            if locate(item).1.is_none() {
                GROUP_GAP + GROUP_HEIGHT
            } else {
                MEMBER_HEIGHT
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
            Some(match locate(item) {
                (group, None) => group_row(marker, &model.groups[group], model.width, &model.palette),
                (group, Some(0)) => {
                    member_row(
                        marker,
                        &model.groups[group].keep,
                        None,
                        model.clone_mode,
                        model.width,
                        &model.palette,
                        &model.actions,
                    )
                    .view
                }
                (group, Some(member)) => {
                    member_row(
                        marker,
                        &model.groups[group].copies[member - 1],
                        Some(model.layout.tag(group, member - 1)),
                        model.clone_mode,
                        model.width,
                        &model.palette,
                        &model.actions,
                    )
                    .view
                }
            })
        }
    }
);

impl DuplicatesSource {
    pub(crate) fn new(marker: MainThreadMarker, model: DuplicatesSourceModel) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(model);
        unsafe { msg_send![super(this), init] }
    }
}

fn locate(item: &AnyObject) -> (usize, Option<usize>) {
    let key = item
        .downcast_ref::<NSString>()
        .expect("duplicate rows are keyed by strings")
        .to_string();
    let mut parts = key
        .split('.')
        .map(|part| part.parse::<usize>().expect("duplicate keys are numeric"));
    (parts.next().expect("a key names a group"), parts.next())
}
