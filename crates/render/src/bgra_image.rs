use std::ffi::c_void;
use std::path::Path;

use objc2::AnyThread;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGContext, CGImageAlphaInfo, CGImageByteOrderInfo,
    kCGColorSpaceSRGB,
};
use objc2_foundation::{NSData, NSDictionary, NSString};

use crate::render_error::RenderError;

const BYTES_PER_PIXEL: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BgraImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl BgraImage {
    pub fn draw(width: usize, height: usize, painter: impl FnOnce(&CGContext)) -> Self {
        let mut image = Self {
            width,
            height,
            pixels: vec![0; width * height * BYTES_PER_PIXEL],
        };
        let context = image.context();
        painter(&context);
        drop(context);
        image
    }

    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let start = (y * self.width + x) * BYTES_PER_PIXEL;
        [
            self.pixels[start],
            self.pixels[start + 1],
            self.pixels[start + 2],
            self.pixels[start + 3],
        ]
    }

    pub fn write_png(&self, path: &Path) -> Result<(), RenderError> {
        let mut copy = self.clone();
        let context = copy.context();
        let image =
            CGBitmapContextCreateImage(Some(&context)).ok_or_else(|| RenderError::new("the frame has no image"))?;
        let representation = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
        let data = unsafe {
            representation.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        }
        .ok_or_else(|| RenderError::new("AppKit did not encode a PNG"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| RenderError::new(error.to_string()))?;
        }
        let target = NSString::from_str(&path.to_string_lossy());
        if data.writeToFile_atomically(&target, true) {
            Ok(())
        } else {
            Err(RenderError::new(format!("{} cannot be written", path.display())))
        }
    }

    pub fn read_png(path: &Path) -> Result<Self, RenderError> {
        let data = NSData::dataWithContentsOfFile(&NSString::from_str(&path.to_string_lossy()))
            .ok_or_else(|| RenderError::new(format!("{} cannot be read", path.display())))?;
        let representation = NSBitmapImageRep::imageRepWithData(&data)
            .ok_or_else(|| RenderError::new(format!("{} is not an image", path.display())))?;
        let image = representation
            .CGImage()
            .ok_or_else(|| RenderError::new(format!("{} has no bitmap", path.display())))?;
        let width = objc2_core_graphics::CGImage::width(Some(&image));
        let height = objc2_core_graphics::CGImage::height(Some(&image));
        Ok(Self::draw(width, height, |context| {
            CGContext::draw_image(
                Some(context),
                CGRect::new(CGPoint::ZERO, CGSize::new(width as f64, height as f64)),
                Some(&image),
            );
        }))
    }

    fn context(&mut self) -> CFRetained<CGContext> {
        let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })).expect("the sRGB space exists");
        let info = CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0;
        unsafe {
            CGBitmapContextCreate(
                self.pixels.as_mut_ptr().cast::<c_void>(),
                self.width,
                self.height,
                8,
                self.width * BYTES_PER_PIXEL,
                Some(&space),
                info,
            )
        }
        .expect("a bitmap context wraps the pixels")
    }
}
