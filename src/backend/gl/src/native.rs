use std::cell::Cell;
use std::ops::Range;
use std::sync::{Arc, Mutex, RwLock};

use crate::hal::backend::FastHashMap;
use crate::hal::memory::{Properties, Requirements};
use crate::hal::{buffer, format, image as i, pass, pso};

use crate::{Backend, GlContext};

pub type TextureType = u32;

// TODO: Consider being generic over `glow::Context` instead
pub type VertexArray = <GlContext as glow::Context>::VertexArray;
pub type RawBuffer = <GlContext as glow::Context>::Buffer;
pub type Shader = <GlContext as glow::Context>::Shader;
pub type Program = <GlContext as glow::Context>::Program;
pub type FrameBuffer = <GlContext as glow::Context>::Framebuffer;
pub type Surface = <GlContext as glow::Context>::Renderbuffer;
pub type Texture = <GlContext as glow::Context>::Texture;
pub type Sampler = <GlContext as glow::Context>::Sampler;
pub type UniformLocation = <GlContext as glow::Context>::UniformLocation;
pub type DescriptorSetLayout = Vec<pso::DescriptorSetLayoutBinding>;

#[derive(Debug)]
pub enum Buffer {
    Unbound {
        size: buffer::Offset,
        usage: buffer::Usage,
    },
    Bound {
        buffer: RawBuffer,
        range: Range<buffer::Offset>,
    },
}

impl Buffer {
    // Asserts that the buffer is bound and returns the raw gl buffer along with its sub-range.
    pub(crate) fn as_bound(&self) -> (RawBuffer, Range<u64>) {
        match self {
            Buffer::Unbound { .. } => panic!("Expected bound buffer!"),
            Buffer::Bound {
                buffer, range, ..
            } => (*buffer, range.clone()),
        }
    }
}

#[derive(Debug)]
pub struct BufferView;

#[derive(Debug)]
pub struct Fence(pub(crate) Cell<Option<<GlContext as glow::Context>::Fence>>);
unsafe impl Send for Fence {}
unsafe impl Sync for Fence {}

impl Fence {
    pub(crate) fn new(sync: Option<<GlContext as glow::Context>::Fence>) -> Self {
        Fence(Cell::new(sync))
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum BindingTypes {
    Images,
    UniformBuffers,
}

#[derive(Clone, Debug)]
pub struct DescRemapData {
    bindings: FastHashMap<
        (
            BindingTypes,
            pso::DescriptorSetIndex,
            pso::DescriptorBinding,
        ),
        Vec<pso::DescriptorBinding>,
    >,
    names: FastHashMap<
        String,
        (
            BindingTypes,
            pso::DescriptorSetIndex,
            pso::DescriptorBinding,
        ),
    >,
    next_binding: FastHashMap<BindingTypes, pso::DescriptorBinding>,
}

/// Stores where the descriptor bindings have been remaped too.
///
/// OpenGL doesn't support sets, so we have to flatten out the bindings.
impl DescRemapData {
    pub fn new() -> Self {
        DescRemapData {
            bindings: FastHashMap::default(),
            names: FastHashMap::default(),
            next_binding: FastHashMap::default(),
        }
    }

    pub fn insert_missing_binding_into_spare(
        &mut self,
        btype: BindingTypes,
        set: pso::DescriptorSetIndex,
        binding: pso::DescriptorBinding,
    ) -> &[pso::DescriptorBinding] {
        let nb = self.next_binding.entry(btype).or_insert(0);
        let val = self
            .bindings
            .entry((btype, set, binding))
            .or_insert(Vec::new());
        val.push(*nb);
        *nb += 1;
        &*val
    }

    pub fn reserve_binding(&mut self, btype: BindingTypes) -> pso::DescriptorBinding {
        let nb = self.next_binding.entry(btype).or_insert(0);
        *nb += 1;
        *nb - 1
    }

    pub fn insert_missing_binding(
        &mut self,
        nb: pso::DescriptorBinding,
        btype: BindingTypes,
        set: pso::DescriptorSetIndex,
        binding: pso::DescriptorBinding,
    ) -> &[pso::DescriptorBinding] {
        let val = self
            .bindings
            .entry((btype, set, binding))
            .or_insert(Vec::new());
        val.push(nb);
        &*val
    }

    pub fn get_binding(
        &self,
        btype: BindingTypes,
        set: pso::DescriptorSetIndex,
        binding: pso::DescriptorBinding,
    ) -> Option<&[pso::DescriptorBinding]> {
        self.bindings.get(&(btype, set, binding)).map(AsRef::as_ref)
    }
}

#[derive(Clone, Debug)]
pub struct GraphicsPipeline {
    pub(crate) program: Program,
    pub(crate) primitive: u32,
    pub(crate) patch_size: Option<i32>,
    pub(crate) blend_targets: Vec<pso::ColorBlendDesc>,
    pub(crate) attributes: Vec<AttributeDesc>,
    pub(crate) vertex_buffers: Vec<Option<pso::VertexBufferDesc>>,
    pub(crate) uniforms: Vec<UniformDesc>,
    pub(crate) rasterizer: pso::Rasterizer,
    pub(crate) depth: pso::DepthTest,
}

#[derive(Clone, Debug)]
pub struct ComputePipeline {
    pub(crate) program: Program,
}

#[derive(Copy, Clone, Debug)]
pub struct Image {
    pub(crate) kind: ImageKind,
    // Required for clearing operations
    pub(crate) channel: format::ChannelType,
    pub(crate) requirements: Requirements,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ImageKind {
    Surface(Surface),
    Texture(Texture, TextureType),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
/// Additionally storing the `SamplerInfo` for older OpenGL versions, which
/// don't support separate sampler objects.
pub enum FatSampler {
    Sampler(Sampler),
    Info(i::SamplerInfo),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ImageView {
    Surface(Surface),
    Texture(Texture, TextureType, i::Level),
    TextureLayer(Texture, TextureType, i::Level, i::Layer),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum DescSetBindings {
    Buffer {
        ty: BindingTypes,
        binding: pso::DescriptorBinding,
        buffer: RawBuffer,
        offset: i32,
        size: i32,
    },
    Texture(pso::DescriptorBinding, Texture, TextureType),
    Sampler(pso::DescriptorBinding, Sampler),
    SamplerInfo(pso::DescriptorBinding, i::SamplerInfo),
}

#[derive(Clone, Debug)]
pub struct DescriptorSet {
    layout: DescriptorSetLayout,
    pub(crate) bindings: Arc<Mutex<Vec<DescSetBindings>>>,
}

#[derive(Debug)]
pub struct DescriptorPool {}

impl pso::DescriptorPool<Backend> for DescriptorPool {
    unsafe fn allocate_set(
        &mut self,
        layout: &DescriptorSetLayout,
    ) -> Result<DescriptorSet, pso::AllocationError> {
        Ok(DescriptorSet {
            layout: layout.clone(),
            bindings: Arc::new(Mutex::new(Vec::new())),
        })
    }

    unsafe fn free_sets<I>(&mut self, _descriptor_sets: I)
    where
        I: IntoIterator<Item = DescriptorSet>,
    {
        // Poof!  Does nothing, because OpenGL doesn't have a meaningful concept of a `DescriptorSet`.
    }

    unsafe fn reset(&mut self) {
        // Poof!  Does nothing, because OpenGL doesn't have a meaningful concept of a `DescriptorSet`.
    }
}

#[derive(Clone, Debug, Hash)]
pub enum ShaderModule {
    Raw(Shader),
    Spirv(Vec<u32>),
}

#[derive(Debug)]
pub struct Memory {
    pub(crate) properties: Properties,
    /// Gl buffer and the target that should be used for map operations.  Image memory is faked and
    /// has no associated buffer, so this will be None for image memory.
    pub(crate) buffer: Option<(RawBuffer, u32)>,
    /// Allocation size
    pub(crate) size: u64,
    pub(crate) map_flags: u32,
    pub(crate) emulate_map_allocation: Cell<Option<*mut u8>>,
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {}

#[derive(Clone, Debug)]
pub struct RenderPass {
    pub(crate) attachments: Vec<pass::Attachment>,
    pub(crate) subpasses: Vec<SubpassDesc>,
}

#[derive(Clone, Debug)]
pub struct SubpassDesc {
    pub(crate) color_attachments: Vec<usize>,
    pub(crate) depth_stencil: Option<usize>,
}

impl SubpassDesc {
    /// Check if an attachment is used by this sub-pass.
    pub(crate) fn is_using(&self, at_id: pass::AttachmentId) -> bool {
        let uses_ds = match self.depth_stencil {
            Some(ds) => ds == at_id,
            None => false,
        };
        let uses_color = self.color_attachments.iter().any(|id| *id == at_id);
        uses_color || uses_ds
    }
}

#[derive(Debug)]
pub struct PipelineLayout {
    pub(crate) desc_remap_data: Arc<RwLock<DescRemapData>>,
}

#[derive(Debug)]
// No inter-queue synchronization required for GL.
pub struct Semaphore;

#[derive(Clone, Debug)]
pub struct AttributeDesc {
    pub(crate) location: u32,
    pub(crate) offset: u32,
    pub(crate) binding: u32,
    pub(crate) size: i32,
    pub(crate) format: u32,
    pub(crate) vertex_attrib_fn: VertexAttribFunction,
}

#[derive(Clone, Copy, Debug)]
pub struct UniformDesc {
    pub(crate) location: UniformLocation,
    pub(crate) offset: u32,
    pub(crate) utype: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum VertexAttribFunction {
    Float,   // glVertexAttribPointer
    Integer, // glVertexAttribIPointer
    Double,  // glVertexAttribLPointer
}
