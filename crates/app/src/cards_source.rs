use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSCollectionView, NSCollectionViewDataSource, NSCollectionViewDelegate, NSCollectionViewDelegateFlowLayout,
    NSCollectionViewItem, NSCollectionViewLayout, NSIndexPathNSCollectionViewAdditions,
};
use objc2_foundation::{NSIndexPath, NSInteger, NSObject, NSSize, NSString};

use crate::cards_model::CardsModel;
use crate::recommendation_card::RecommendationCard;

pub(crate) const CARD_ITEM: &str = "recommendation-card";

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = CardsModel]
    #[derive(Debug)]
    pub struct CardsSource;

    unsafe impl NSObjectProtocol for CardsSource {}

    unsafe impl NSCollectionViewDataSource for CardsSource {
        #[unsafe(method(collectionView:numberOfItemsInSection:))]
        fn item_count(&self, _collection: &NSCollectionView, _section: NSInteger) -> NSInteger {
            NSInteger::try_from(self.ivars().cards.len()).expect("cards are few")
        }

        #[unsafe(method_id(collectionView:itemForRepresentedObjectAtIndexPath:))]
        fn item(&self, collection: &NSCollectionView, index_path: &NSIndexPath) -> Retained<NSCollectionViewItem> {
            let model = self.ivars();
            let position = usize::try_from(index_path.item()).expect("item positions are non-negative");
            let item = collection.makeItemWithIdentifier_forIndexPath(&NSString::from_str(CARD_ITEM), index_path);
            let host = item.view();
            for child in &host.subviews() {
                child.removeFromSuperview();
            }
            let card = RecommendationCard::build(
                MainThreadMarker::from(self),
                &model.cards[position],
                position,
                self.card_width(collection),
                &model.palette,
                &model.actions,
            );
            host.addSubview(&card.view);
            item
        }
    }

    unsafe impl NSCollectionViewDelegate for CardsSource {}

    unsafe impl NSCollectionViewDelegateFlowLayout for CardsSource {
        #[unsafe(method(collectionView:layout:sizeForItemAtIndexPath:))]
        fn item_size(
            &self,
            collection: &NSCollectionView,
            _layout: &NSCollectionViewLayout,
            index_path: &NSIndexPath,
        ) -> NSSize {
            let position = usize::try_from(index_path.item()).expect("item positions are non-negative");
            let width = self.card_width(collection);
            NSSize::new(width, self.heights(width)[position])
        }
    }
);

impl CardsSource {
    pub(crate) fn new(marker: MainThreadMarker, model: CardsModel) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(model);
        unsafe { msg_send![super(this), init] }
    }

    fn card_width(&self, collection: &NSCollectionView) -> f64 {
        (collection.bounds().size.width - 2.0 * self.ivars().inset).max(0.0)
    }

    fn heights(&self, width: f64) -> Vec<f64> {
        let model = self.ivars();
        let mut cached = model.heights.borrow_mut();
        if cached.0.to_bits() != width.to_bits() || cached.1.len() != model.cards.len() {
            let marker = MainThreadMarker::from(self);
            cached.1 = model
                .cards
                .iter()
                .enumerate()
                .map(|(position, card)| {
                    RecommendationCard::build(marker, card, position, width, &model.palette, &model.actions).height
                })
                .collect();
            cached.0 = width;
        }
        cached.1.clone()
    }
}
