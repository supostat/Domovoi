use std::fmt;

use crate::diagram_change::DiagramChange;

pub struct DiagramObserver(Box<dyn Fn(DiagramChange)>);

impl DiagramObserver {
    pub fn new(notify: impl Fn(DiagramChange) + 'static) -> Self {
        Self(Box::new(notify))
    }

    pub(crate) fn notify(&self, change: DiagramChange) {
        (self.0)(change);
    }
}

impl fmt::Debug for DiagramObserver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("DiagramObserver").finish_non_exhaustive()
    }
}
