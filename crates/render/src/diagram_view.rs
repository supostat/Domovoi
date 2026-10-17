use std::cell::{OnceCell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibilityElement, NSAccessibilityFocusedUIElementChangedNotification, NSAccessibilityGroupRole,
    NSAccessibilityPostNotification, NSAutoresizingMaskOptions, NSEvent, NSResponder, NSTrackingArea,
    NSTrackingAreaOptions, NSView,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
use objc2_foundation::{NSArray, NSCopying, NSRect, NSString};
use objc2_metal_kit::MTKView;

use crate::accessible_segment::AccessibleSegment;
use crate::diagram_drawer::DiagramDrawer;
use crate::diagram_graphics::DiagramGraphics;
use crate::diagram_renderer::PIXEL_FORMAT;
use crate::diagram_state::DiagramState;
use crate::point::Point;

const ANIMATION_RATE: isize = 120;
const LEFT_ARROW: u16 = 123;
const RIGHT_ARROW: u16 = 124;
const DOWN_ARROW: u16 = 125;
const UP_ARROW: u16 = 126;
const RETURN: u16 = 36;

#[derive(Debug)]
pub struct DiagramViewState {
    state: Rc<RefCell<DiagramState>>,
    surface: OnceCell<(Retained<MTKView>, Retained<DiagramDrawer>)>,
    tracking: RefCell<Option<Retained<NSTrackingArea>>>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DiagramViewState]
    #[derive(Debug)]
    pub struct DiagramView;

    impl DiagramView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let code = event.keyCode();
            match code {
                RIGHT_ARROW | DOWN_ARROW | LEFT_ARROW | UP_ARROW => {
                    let forward = code == RIGHT_ARROW || code == DOWN_ARROW;
                    self.move_focus(forward);
                }
                RETURN => {
                    self.ivars().state.borrow_mut().open_keyboard_focus(Instant::now());
                    self.changed();
                }
                _ => unsafe {
                    let _: () = msg_send![super(self), keyDown: event];
                },
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let point = self.local_point(event);
            self.ivars().state.borrow_mut().click(point, Instant::now());
            self.changed();
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            let point = self.local_point(event);
            self.ivars().state.borrow_mut().hover(Some(point), Instant::now());
            self.animate();
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.ivars().state.borrow_mut().hover(None, Instant::now());
            self.animate();
        }

        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            if let Some(area) = self.ivars().tracking.borrow_mut().take() {
                self.removeTrackingArea(&area);
            }
            let options = NSTrackingAreaOptions::MouseMoved
                | NSTrackingAreaOptions::MouseEnteredAndExited
                | NSTrackingAreaOptions::ActiveInKeyWindow
                | NSTrackingAreaOptions::InVisibleRect;
            let area = unsafe {
                NSTrackingArea::initWithRect_options_owner_userInfo(
                    NSTrackingArea::alloc(),
                    self.bounds(),
                    options,
                    Some(self),
                    None,
                )
            };
            self.addTrackingArea(&area);
            *self.ivars().tracking.borrow_mut() = Some(area);
            unsafe {
                let _: () = msg_send![super(self), updateTrackingAreas];
            }
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: CGSize) {
            unsafe {
                let _: () = msg_send![super(self), setFrameSize: size];
            }
            self.ivars().state.borrow_mut().resize(size.width as f32, size.height as f32);
            self.changed();
        }

        #[unsafe(method(isAccessibilityElement))]
        fn is_accessibility_element(&self) -> bool {
            true
        }

        #[unsafe(method_id(accessibilityRole))]
        fn accessibility_role(&self) -> Retained<NSString> {
            unsafe { NSAccessibilityGroupRole }.copy()
        }

        #[unsafe(method_id(accessibilityChildren))]
        fn accessibility_children(&self) -> Retained<NSArray<AnyObject>> {
            let elements: Vec<Retained<AnyObject>> = self
                .ivars()
                .state
                .borrow()
                .segments()
                .iter()
                .map(|segment| self.element(segment))
                .collect();
            NSArray::from_retained_slice(&elements)
        }
    }
);

impl DiagramView {
    pub fn new(
        marker: MainThreadMarker,
        frame: NSRect,
        state: DiagramState,
        graphics: DiagramGraphics,
    ) -> Retained<Self> {
        let state = Rc::new(RefCell::new(state));
        let this = Self::alloc(marker).set_ivars(DiagramViewState {
            state: Rc::clone(&state),
            surface: OnceCell::new(),
            tracking: RefCell::new(None),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let device = graphics.renderer.device();
        let surface = MTKView::initWithFrame_device(MTKView::alloc(marker), this.bounds(), Some(device));
        surface.setColorPixelFormat(PIXEL_FORMAT);
        surface.setFramebufferOnly(true);
        surface.setEnableSetNeedsDisplay(true);
        surface.setPaused(true);
        surface.setPreferredFramesPerSecond(ANIMATION_RATE);
        let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }));
        surface.setColorspace(space.as_deref());
        surface.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        let drawer = DiagramDrawer::new(marker, state, Rc::new(RefCell::new(graphics)));
        surface.setDelegate(Some(ProtocolObject::from_ref(&*drawer)));
        this.addSubview(&surface);
        this.ivars()
            .surface
            .set((surface, drawer))
            .expect("the surface is attached once");
        this
    }

    pub fn state(&self) -> std::cell::Ref<'_, DiagramState> {
        self.ivars().state.borrow()
    }

    pub fn update_state(&self, change: impl FnOnce(&mut DiagramState)) {
        change(&mut self.ivars().state.borrow_mut());
        self.changed();
    }

    fn move_focus(&self, forward: bool) {
        let focused = self
            .ivars()
            .state
            .borrow_mut()
            .move_keyboard_focus(forward, Instant::now());
        if let Some(segment) = focused {
            let element = self.element(&segment);
            unsafe { NSAccessibilityPostNotification(&element, NSAccessibilityFocusedUIElementChangedNotification) };
        }
        self.animate();
    }

    fn local_point(&self, event: &NSEvent) -> Point {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        Point {
            x: point.x as f32,
            y: point.y as f32,
        }
    }

    fn element(&self, segment: &AccessibleSegment) -> Retained<AnyObject> {
        let frame = CGRect::new(
            CGPoint::new(f64::from(segment.frame.x), f64::from(segment.frame.y)),
            CGSize::new(f64::from(segment.frame.width), f64::from(segment.frame.height)),
        );
        let element = unsafe {
            NSAccessibilityElement::accessibilityElementWithRole_frame_label_parent(
                NSAccessibilityGroupRole,
                frame,
                Some(&NSString::from_str(&segment.label)),
                Some(self),
            )
        };
        if let Some(element) = element.downcast_ref::<NSAccessibilityElement>() {
            element.setAccessibilityFrameInParentSpace(frame);
        }
        element
    }

    fn changed(&self) {
        if let Some((surface, _)) = self.ivars().surface.get() {
            surface.setNeedsDisplay(true);
        }
        self.animate();
    }

    fn animate(&self) {
        let animating = self.ivars().state.borrow().is_animating(Instant::now());
        if let Some((surface, _)) = self.ivars().surface.get() {
            if animating {
                surface.setPaused(false);
            } else {
                surface.setNeedsDisplay(true);
            }
        }
    }
}
