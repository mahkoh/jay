use {
    crate::gfx_apis::vulkan::{
        descriptor::VulkanDescriptorSetLayout, device::VulkanDevice, shaders::VulkanShader,
        util::OnDrop, VulkanError,
    },
    arrayvec::ArrayVec,
    ash::{
        vk,
        vk::{
            BlendFactor, BlendOp, ColorComponentFlags, CullModeFlags, DynamicState, FrontFace,
            GraphicsPipelineCreateInfo, Pipeline, PipelineCache, PipelineColorBlendAttachmentState,
            PipelineColorBlendStateCreateInfo, PipelineDynamicStateCreateInfo,
            PipelineInputAssemblyStateCreateInfo, PipelineLayout, PipelineLayoutCreateInfo,
            PipelineMultisampleStateCreateInfo, PipelineRasterizationStateCreateInfo,
            PipelineRenderingCreateInfo, PipelineShaderStageCreateInfo,
            PipelineVertexInputStateCreateInfo, PipelineViewportStateCreateInfo, PolygonMode,
            PrimitiveTopology, PushConstantRange, SampleCountFlags, ShaderStageFlags,
        },
    },
    std::{mem, rc::Rc, slice},
};

pub(super) struct VulkanPipeline {
    pub(super) vert: Rc<VulkanShader>,
    pub(super) _frag: Rc<VulkanShader>,
    pub(super) frag_push_offset: u32,
    pub(super) pipeline_layout: PipelineLayout,
    pub(super) pipeline: Pipeline,
    pub(super) _frag_descriptor_set_layout: Option<Rc<VulkanDescriptorSetLayout>>,
}

pub(super) struct PipelineCreateInfo {
    pub(super) format: vk::Format,
    pub(super) vert: Rc<VulkanShader>,
    pub(super) frag: Rc<VulkanShader>,
    pub(super) alpha: bool,
    pub(super) frag_descriptor_set_layout: Option<Rc<VulkanDescriptorSetLayout>>,
}

impl VulkanDevice {
    pub(super) fn create_pipeline<V, F>(
        &self,
        info: PipelineCreateInfo,
    ) -> Result<Rc<VulkanPipeline>, VulkanError> {
        self.create_pipeline_(info, mem::size_of::<V>() as _, mem::size_of::<F>() as _)
    }

    fn create_pipeline_(
        &self,
        info: PipelineCreateInfo,
        vert_push_size: u32,
        frag_push_size: u32,
    ) -> Result<Rc<VulkanPipeline>, VulkanError> {
        let pipeline_layout = {
            let mut push_constant_ranges = ArrayVec::<_, 2>::new();
            let mut push_constant_offset = 0;
            if vert_push_size > 0 {
                push_constant_ranges.push(
                    PushConstantRange::default()
                        .stage_flags(ShaderStageFlags::VERTEX)
                        .offset(0)
                        .size(vert_push_size),
                );
                push_constant_offset += vert_push_size;
            }
            if frag_push_size > 0 {
                push_constant_ranges.push(
                    PushConstantRange::default()
                        .stage_flags(ShaderStageFlags::FRAGMENT)
                        .offset(push_constant_offset)
                        .size(frag_push_size),
                );
                #[expect(unused_assignments)]
                {
                    push_constant_offset += frag_push_size;
                }
            }
            let mut descriptor_set_layouts = ArrayVec::<_, 1>::new();
            descriptor_set_layouts
                .extend(info.frag_descriptor_set_layout.as_ref().map(|l| l.layout));
            let create_info = PipelineLayoutCreateInfo::default()
                .push_constant_ranges(&push_constant_ranges)
                .set_layouts(&descriptor_set_layouts);
            let layout = unsafe { self.device.create_pipeline_layout(&create_info, None) };
            layout.map_err(VulkanError::CreatePipelineLayout)?
        };
        let destroy_layout =
            OnDrop(|| unsafe { self.device.destroy_pipeline_layout(pipeline_layout, None) });
        let pipeline = {
            let stages = [
                PipelineShaderStageCreateInfo::default()
                    .stage(ShaderStageFlags::VERTEX)
                    .module(info.vert.module)
                    .name(c"main"),
                PipelineShaderStageCreateInfo::default()
                    .stage(ShaderStageFlags::FRAGMENT)
                    .module(info.frag.module)
                    .name(c"main"),
            ];
            let input_assembly_state = PipelineInputAssemblyStateCreateInfo::default()
                .topology(PrimitiveTopology::TRIANGLE_STRIP);
            let vertex_input_state = PipelineVertexInputStateCreateInfo::default();
            let rasterization_state = PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(PolygonMode::FILL)
                .cull_mode(CullModeFlags::NONE)
                .line_width(1.0)
                .front_face(FrontFace::COUNTER_CLOCKWISE);
            let multisampling_state = PipelineMultisampleStateCreateInfo::default()
                .sample_shading_enable(false)
                .rasterization_samples(SampleCountFlags::TYPE_1);
            let mut blending = PipelineColorBlendAttachmentState::default()
                .color_write_mask(ColorComponentFlags::RGBA);
            if info.alpha {
                blending = blending
                    .blend_enable(true)
                    .src_color_blend_factor(BlendFactor::ONE)
                    .dst_color_blend_factor(BlendFactor::ONE_MINUS_SRC_ALPHA)
                    .color_blend_op(BlendOp::ADD)
                    .src_alpha_blend_factor(BlendFactor::ONE)
                    .dst_alpha_blend_factor(BlendFactor::ONE_MINUS_SRC_ALPHA)
                    .alpha_blend_op(BlendOp::ADD);
            }
            let color_blend_state = PipelineColorBlendStateCreateInfo::default()
                .attachments(slice::from_ref(&blending));
            let dynamic_states = [DynamicState::VIEWPORT, DynamicState::SCISSOR];
            let dynamic_state =
                PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
            let viewport_state = PipelineViewportStateCreateInfo::default()
                .viewport_count(1)
                .scissor_count(1);
            let mut pipeline_rendering_create_info = PipelineRenderingCreateInfo::default()
                .color_attachment_formats(slice::from_ref(&info.format));
            let create_info = GraphicsPipelineCreateInfo::default()
                .push_next(&mut pipeline_rendering_create_info)
                .stages(&stages)
                .input_assembly_state(&input_assembly_state)
                .vertex_input_state(&vertex_input_state)
                .rasterization_state(&rasterization_state)
                .multisample_state(&multisampling_state)
                .color_blend_state(&color_blend_state)
                .dynamic_state(&dynamic_state)
                .viewport_state(&viewport_state)
                .layout(pipeline_layout);
            let pipelines = unsafe {
                self.device.create_graphics_pipelines(
                    PipelineCache::null(),
                    slice::from_ref(&create_info),
                    None,
                )
            };
            let mut pipelines = pipelines
                .map_err(|e| e.1)
                .map_err(VulkanError::CreatePipeline)?;
            assert_eq!(pipelines.len(), 1);
            pipelines.pop().unwrap()
        };
        destroy_layout.forget();
        Ok(Rc::new(VulkanPipeline {
            vert: info.vert,
            _frag: info.frag,
            frag_push_offset: vert_push_size,
            pipeline_layout,
            pipeline,
            _frag_descriptor_set_layout: info.frag_descriptor_set_layout,
        }))
    }
}

impl Drop for VulkanPipeline {
    fn drop(&mut self) {
        unsafe {
            let device = &self.vert.device.device;
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}
