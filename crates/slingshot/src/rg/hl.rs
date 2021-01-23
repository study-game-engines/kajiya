use ash::vk;
use vk_sync::AccessType;

use crate::{backend::image::ImageViewDescBuilder, Image};

use super::{
    BindRgRef, Buffer, GpuSrv, GpuUav, Handle, PassBuilder, Ref, RenderPassApi, RenderPassBinding,
    Resource, RgComputePipelineHandle,
};

trait ConstBlob {
    fn push_self(
        self: Box<Self>,
        dynamic_constants: &mut crate::dynamic_constants::DynamicConstants,
    ) -> u32;
}

impl<T> ConstBlob for T
where
    T: Copy + 'static,
{
    fn push_self(
        self: Box<Self>,
        dynamic_constants: &mut crate::dynamic_constants::DynamicConstants,
    ) -> u32 {
        dynamic_constants.push(*self)
    }
}

pub struct SimpleComputePass<'rg> {
    pass: PassBuilder<'rg>,
    pipeline: RgComputePipelineHandle,
    bindings: Vec<RenderPassBinding>,
    const_blobs: Vec<(usize, Box<dyn ConstBlob>)>,
}

impl<'rg> SimpleComputePass<'rg> {
    pub fn new(mut pass: PassBuilder<'rg>, pipeline_path: &str) -> Self {
        let pipeline = pass.register_compute_pipeline(pipeline_path);

        Self {
            pass,
            pipeline,
            bindings: Vec::new(),
            const_blobs: Vec::new(),
        }
    }

    pub fn read<Res>(mut self, handle: &Handle<Res>) -> Self
    where
        Res: Resource + 'static,
        Ref<Res, GpuSrv>: BindRgRef,
    {
        let handle_ref = self.pass.read(
            handle,
            AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer,
        );

        self.bindings.push(BindRgRef::bind(&handle_ref));

        self
    }

    pub fn read_aspect(
        mut self,
        handle: &Handle<Image>,
        aspect_mask: vk::ImageAspectFlags,
    ) -> Self {
        let handle_ref = self.pass.read(
            handle,
            AccessType::ComputeShaderReadSampledImageOrUniformTexelBuffer,
        );

        self.bindings
            .push(handle_ref.bind_view(ImageViewDescBuilder::default().aspect_mask(aspect_mask)));

        self
    }

    pub fn write<Res>(mut self, handle: &mut Handle<Res>) -> Self
    where
        Res: Resource + 'static,
        Ref<Res, GpuUav>: BindRgRef,
    {
        let handle_ref = self.pass.write(handle, AccessType::ComputeShaderWrite);

        self.bindings.push(BindRgRef::bind(&handle_ref));

        self
    }

    pub fn constants<T: Copy + 'static>(mut self, consts: T) -> Self {
        let binding_idx = self.bindings.len();

        self.bindings.push(RenderPassBinding::DynamicConstants(0));
        self.const_blobs.push((binding_idx, Box::new(consts)));

        self
    }

    fn patch_const_blobs(
        api: &mut RenderPassApi,
        bindings: &mut Vec<RenderPassBinding>,
        const_blobs: Vec<(usize, Box<dyn ConstBlob>)>,
    ) {
        let dynamic_constants = api.dynamic_constants();

        for (binding_idx, blob) in const_blobs {
            let dynamic_constants_offset = ConstBlob::push_self(blob, dynamic_constants);
            match &mut bindings[binding_idx] {
                RenderPassBinding::DynamicConstants(offset) => {
                    *offset = dynamic_constants_offset;
                }
                _ => unreachable!(),
            }
        }
    }

    pub fn dispatch(self, extent: [u32; 3]) {
        let pipeline = self.pipeline;
        let mut bindings = self.bindings;
        let const_blobs = self.const_blobs;

        self.pass.render(move |api| {
            Self::patch_const_blobs(api, &mut bindings, const_blobs);

            let pipeline =
                api.bind_compute_pipeline(pipeline.into_binding().descriptor_set(0, &bindings));

            pipeline.dispatch(extent);
        });
    }

    pub fn dispatch_indirect(mut self, args_buffer: &Handle<Buffer>, args_buffer_offset: u64) {
        let args_buffer_ref = self.pass.read(args_buffer, AccessType::IndirectBuffer);

        let pipeline = self.pipeline;
        let mut bindings = self.bindings;
        let const_blobs = self.const_blobs;

        self.pass.render(move |api| {
            Self::patch_const_blobs(api, &mut bindings, const_blobs);

            let pipeline =
                api.bind_compute_pipeline(pipeline.into_binding().descriptor_set(0, &bindings));

            pipeline.dispatch_indirect(args_buffer_ref, args_buffer_offset);
        });
    }
}
