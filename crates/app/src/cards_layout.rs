use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSCollectionViewFlowLayout, NSCollectionViewFlowLayoutInvalidationContext, NSCollectionViewLayout,
    NSCollectionViewLayoutInvalidationContext,
};
use objc2_foundation::{NSObject, NSRect};

define_class!(
    #[unsafe(super(NSCollectionViewFlowLayout, NSCollectionViewLayout, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug)]
    pub struct CardsLayout;

    impl CardsLayout {
        #[unsafe(method(shouldInvalidateLayoutForBoundsChange:))]
        fn should_invalidate_layout_for_bounds_change(&self, new_bounds: NSRect) -> bool {
            self.collectionView()
                .is_some_and(|collection| collection.bounds().size.width.to_bits() != new_bounds.size.width.to_bits())
        }

        #[unsafe(method_id(invalidationContextForBoundsChange:))]
        fn invalidation_context_for_bounds_change(
            &self,
            new_bounds: NSRect,
        ) -> Retained<NSCollectionViewLayoutInvalidationContext> {
            let context: Retained<NSCollectionViewLayoutInvalidationContext> =
                unsafe { msg_send![super(self), invalidationContextForBoundsChange: new_bounds] };
            context
                .downcast_ref::<NSCollectionViewFlowLayoutInvalidationContext>()
                .expect("a flow layout invalidates with a flow layout context")
                .setInvalidateFlowLayoutDelegateMetrics(true);
            context
        }
    }
);

impl CardsLayout {
    pub(crate) fn new(marker: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![Self::alloc(marker), init] }
    }
}
