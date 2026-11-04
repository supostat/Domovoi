use objc2_foundation::{NSIndexSet, NSNotFound, NSUInteger};

pub(crate) fn selected_rows(indexes: &NSIndexSet) -> impl Iterator<Item = usize> + '_ {
    let absent = NSUInteger::try_from(NSNotFound).expect("NSNotFound is a positive sentinel");
    let mut next = indexes.firstIndex();
    std::iter::from_fn(move || {
        if next == absent {
            return None;
        }
        let current = next;
        next = indexes.indexGreaterThanIndex(next);
        Some(current)
    })
}
