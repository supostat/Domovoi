use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_core_foundation::CGSize;
use objc2_foundation::NSObject;
use objc2_metal::{MTLCommandBuffer, MTLCommandQueue};
use objc2_metal_kit::{MTKView, MTKViewDelegate};
use objc2_quartz_core::CAMetalDrawable;

use crate::diagram_graphics::DiagramGraphics;
use crate::diagram_state::DiagramState;

#[derive(Debug)]
pub(crate) struct DrawerState {
    state: Rc<RefCell<DiagramState>>,
    graphics: Rc<RefCell<DiagramGraphics>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DrawerState]
    #[derive(Debug)]
    pub(crate) struct DiagramDrawer;

    unsafe impl NSObjectProtocol for DiagramDrawer {}

    unsafe impl MTKViewDelegate for DiagramDrawer {
        #[unsafe(method(mtkView:drawableSizeWillChange:))]
        fn drawable_size_changed(&self, _view: &MTKView, _size: CGSize) {}

        #[unsafe(method(drawInMTKView:))]
        fn draw(&self, view: &MTKView) {
            let now = Instant::now();
            let state = self.ivars().state.borrow();
            let mut graphics = self.ivars().graphics.borrow_mut();
            let graphics = &mut *graphics;
            let Some(drawable) = view.currentDrawable() else {
                return;
            };
            let scale = (view.drawableSize().width / view.bounds().size.width.max(1.0)) as f32;
            let scene = state.scene(now, scale, &mut graphics.atlas);
            let Some(command_buffer) = graphics.renderer.queue().commandBuffer() else {
                return;
            };
            graphics
                .renderer
                .encode(&scene, &graphics.atlas, &command_buffer, &drawable.texture());
            command_buffer.presentDrawable(ProtocolObject::from_ref(&*drawable));
            command_buffer.commit();
            if !state.is_animating(now) {
                view.setPaused(true);
            }
        }
    }
);

impl DiagramDrawer {
    pub(crate) fn new(
        marker: MainThreadMarker,
        state: Rc<RefCell<DiagramState>>,
        graphics: Rc<RefCell<DiagramGraphics>>,
    ) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(DrawerState { state, graphics });
        unsafe { msg_send![super(this), init] }
    }
}
