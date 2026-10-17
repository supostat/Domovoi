use std::ffi::c_void;
use std::path::Path;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::{NSString, NSURL};
use objc2_metal::{
    MTLBlendFactor, MTLBuffer, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
    MTLCreateSystemDefaultDevice, MTLDevice, MTLLibrary, MTLLoadAction, MTLOrigin, MTLPixelFormat, MTLPrimitiveType,
    MTLRegion, MTLRenderCommandEncoder, MTLRenderPassDescriptor, MTLRenderPipelineDescriptor, MTLRenderPipelineState,
    MTLResourceOptions, MTLSize, MTLStorageMode, MTLStoreAction, MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};

use crate::bgra_image::BgraImage;
use crate::frame_uniforms::FrameUniforms;
use crate::glyph_atlas::{ATLAS_SIDE, GlyphAtlas};
use crate::offscreen_frame::OffscreenFrame;
use crate::render_error::RenderError;
use crate::scene::Scene;

pub const BUILT_METALLIB: &str = env!("DOMOVOI_DIAGRAM_METALLIB");
pub const PIXEL_FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm;
const QUAD_VERTICES: usize = 4;
const BYTES_PER_PIXEL: usize = 4;

#[derive(Debug)]
pub struct DiagramRenderer {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    shapes: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    glyphs: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    atlas: Retained<ProtocolObject<dyn MTLTexture>>,
    atlas_version: (u64, u64),
}

impl DiagramRenderer {
    pub fn system_default(metallib: &Path) -> Result<Self, RenderError> {
        let device = MTLCreateSystemDefaultDevice().ok_or_else(|| RenderError::new("this Mac has no Metal device"))?;
        Self::new(device, metallib)
    }

    pub fn new(device: Retained<ProtocolObject<dyn MTLDevice>>, metallib: &Path) -> Result<Self, RenderError> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&metallib.to_string_lossy()));
        let library = device.newLibraryWithURL_error(&url).map_err(|error| {
            RenderError::new(format!(
                "the diagram shaders did not load: {}",
                error.localizedDescription()
            ))
        })?;
        let shapes = pipeline(&device, &library, "shape_vertex", "shape_fragment")?;
        let glyphs = pipeline(&device, &library, "glyph_vertex", "glyph_fragment")?;
        let queue = device
            .newCommandQueue()
            .ok_or_else(|| RenderError::new("Metal gave no command queue"))?;
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                MTLPixelFormat::R8Unorm,
                ATLAS_SIDE as usize,
                ATLAS_SIDE as usize,
                false,
            )
        };
        descriptor.setStorageMode(MTLStorageMode::Shared);
        descriptor.setUsage(MTLTextureUsage::ShaderRead);
        let atlas = device
            .newTextureWithDescriptor(&descriptor)
            .ok_or_else(|| RenderError::new("Metal gave no glyph atlas"))?;
        Ok(Self {
            device,
            queue,
            shapes,
            glyphs,
            atlas,
            atlas_version: (0, 0),
        })
    }

    pub fn device(&self) -> &ProtocolObject<dyn MTLDevice> {
        &self.device
    }

    pub fn queue(&self) -> &ProtocolObject<dyn MTLCommandQueue> {
        &self.queue
    }

    pub fn encode(
        &mut self,
        scene: &Scene,
        atlas: &GlyphAtlas,
        command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
        target: &ProtocolObject<dyn MTLTexture>,
    ) {
        self.upload_atlas(atlas);
        let pass = MTLRenderPassDescriptor::new();
        let attachment = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setTexture(Some(target));
        attachment.setLoadAction(MTLLoadAction::Clear);
        attachment.setStoreAction(MTLStoreAction::Store);
        let [red, green, blue, alpha] = scene.clear.map(f64::from);
        attachment.setClearColor(MTLClearColor {
            red,
            green,
            blue,
            alpha,
        });
        let encoder = command_buffer
            .renderCommandEncoderWithDescriptor(&pass)
            .expect("a render pass starts");
        let uniforms = FrameUniforms {
            viewport: [scene.width, scene.height],
            scale: scene.scale,
            progress: scene.progress,
        };
        let uniform_bytes = bytemuck::bytes_of(&uniforms);
        if !scene.shapes.is_empty() {
            let buffer = self.buffer(bytemuck::cast_slice(&scene.shapes));
            encoder.setRenderPipelineState(&self.shapes);
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(Some(&buffer), 0, 0);
                encoder.setFragmentBuffer_offset_atIndex(Some(&buffer), 0, 0);
                encoder.setVertexBytes_length_atIndex(bytes_pointer(uniform_bytes), uniform_bytes.len(), 1);
                encoder.setFragmentBytes_length_atIndex(bytes_pointer(uniform_bytes), uniform_bytes.len(), 1);
                encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                    MTLPrimitiveType::TriangleStrip,
                    0,
                    QUAD_VERTICES,
                    scene.shapes.len(),
                );
            }
        }
        if !scene.glyphs.is_empty() {
            let buffer = self.buffer(bytemuck::cast_slice(&scene.glyphs));
            encoder.setRenderPipelineState(&self.glyphs);
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(Some(&buffer), 0, 0);
                encoder.setVertexBytes_length_atIndex(bytes_pointer(uniform_bytes), uniform_bytes.len(), 1);
                encoder.setFragmentTexture_atIndex(Some(&self.atlas), 0);
                encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                    MTLPrimitiveType::TriangleStrip,
                    0,
                    QUAD_VERTICES,
                    scene.glyphs.len(),
                );
            }
        }
        encoder.endEncoding();
    }

    pub fn render_offscreen(&mut self, scene: &Scene, atlas: &GlyphAtlas) -> OffscreenFrame {
        let width = (scene.width * scene.scale).round() as usize;
        let height = (scene.height * scene.scale).round() as usize;
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                PIXEL_FORMAT,
                width,
                height,
                false,
            )
        };
        descriptor.setStorageMode(MTLStorageMode::Shared);
        descriptor.setUsage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
        let target = self
            .device
            .newTextureWithDescriptor(&descriptor)
            .expect("Metal gives an offscreen target");
        let command_buffer = self.queue.commandBuffer().expect("Metal gives a command buffer");
        self.encode(scene, atlas, &command_buffer, &target);
        command_buffer.commit();
        command_buffer.waitUntilCompleted();
        let mut pixels = vec![0_u8; width * height * BYTES_PER_PIXEL];
        unsafe {
            target.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                NonNull::new(pixels.as_mut_ptr().cast::<c_void>()).expect("a vector is not null"),
                width * BYTES_PER_PIXEL,
                region(width, height),
                0,
            );
        }
        OffscreenFrame {
            image: BgraImage { width, height, pixels },
            gpu_seconds: command_buffer.GPUEndTime() - command_buffer.GPUStartTime(),
        }
    }

    fn upload_atlas(&mut self, atlas: &GlyphAtlas) {
        if atlas.version() == self.atlas_version {
            return;
        }
        let side = ATLAS_SIDE as usize;
        unsafe {
            self.atlas.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                region(side, side),
                0,
                NonNull::new(atlas.pixels().as_ptr().cast_mut().cast::<c_void>()).expect("the atlas is not null"),
                side,
            );
        }
        self.atlas_version = atlas.version();
    }

    fn buffer(&self, bytes: &[u8]) -> Retained<ProtocolObject<dyn MTLBuffer>> {
        unsafe {
            self.device.newBufferWithBytes_length_options(
                NonNull::new(bytes.as_ptr().cast_mut().cast::<c_void>()).expect("instance bytes are not null"),
                bytes.len(),
                MTLResourceOptions::StorageModeShared,
            )
        }
        .expect("Metal gives an instance buffer")
    }
}

fn pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    vertex: &str,
    fragment: &str,
) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>, RenderError> {
    let function = |name: &str| {
        library
            .newFunctionWithName(&NSString::from_str(name))
            .ok_or_else(|| RenderError::new(format!("the shader {name} is missing")))
    };
    let descriptor = MTLRenderPipelineDescriptor::new();
    descriptor.setVertexFunction(Some(&*function(vertex)?));
    descriptor.setFragmentFunction(Some(&*function(fragment)?));
    let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
    attachment.setPixelFormat(PIXEL_FORMAT);
    attachment.setBlendingEnabled(true);
    attachment.setSourceRGBBlendFactor(MTLBlendFactor::One);
    attachment.setSourceAlphaBlendFactor(MTLBlendFactor::One);
    attachment.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
    device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .map_err(|error| {
            RenderError::new(format!(
                "the {vertex} pipeline failed: {}",
                error.localizedDescription()
            ))
        })
}

fn region(width: usize, height: usize) -> MTLRegion {
    MTLRegion {
        origin: MTLOrigin { x: 0, y: 0, z: 0 },
        size: MTLSize {
            width,
            height,
            depth: 1,
        },
    }
}

fn bytes_pointer(bytes: &[u8]) -> NonNull<c_void> {
    NonNull::new(bytes.as_ptr().cast_mut().cast::<c_void>()).expect("uniform bytes are not null")
}
