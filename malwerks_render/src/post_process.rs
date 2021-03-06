// Copyright (c) 2020 Kyrylo Bazhenov
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at http://mozilla.org/MPL/2.0/.

use malwerks_vk::*;

use crate::forward_pass::*;
use crate::render_pass::*;

pub struct PostProcess {
    point_sampler: vk::Sampler,
    linear_sampler: vk::Sampler,

    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_set: vk::DescriptorSet,

    vert_module: vk::ShaderModule,
    frag_module: vk::ShaderModule,

    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl PostProcess {
    pub fn new<T>(
        vert_code: &[u32],
        frag_code: &[u32],
        source_pass: &ForwardPass, // TODO: make it a generic render pass
        destination_pass: &T,
        factory: &mut DeviceFactory,
    ) -> Self
    where
        T: RenderPass,
    {
        let vert_module = factory.create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(vert_code).build());
        let frag_module = factory.create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(frag_code).build());

        let entry_name = std::ffi::CString::new("main").expect("failed to allocate entry name");
        let post_process_vert = vk::PipelineShaderStageCreateInfo::builder()
            .name(&entry_name)
            .module(vert_module)
            .stage(vk::ShaderStageFlags::VERTEX);
        let post_process_frag = vk::PipelineShaderStageCreateInfo::builder()
            .name(&entry_name)
            .module(frag_module)
            .stage(vk::ShaderStageFlags::FRAGMENT);

        let point_sampler = factory.create_sampler(
            &vk::SamplerCreateInfo::builder()
                .mag_filter(vk::Filter::NEAREST)
                .min_filter(vk::Filter::NEAREST)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .build(),
        );
        let linear_sampler = factory.create_sampler(
            &vk::SamplerCreateInfo::builder()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .build(),
        );

        let descriptor_pool = factory.create_descriptor_pool(
            &vk::DescriptorPoolCreateInfo::builder().max_sets(1).pool_sizes(&[
                vk::DescriptorPoolSize::builder()
                    .ty(vk::DescriptorType::SAMPLER)
                    .descriptor_count(2)
                    .build(),
                vk::DescriptorPoolSize::builder()
                    .ty(vk::DescriptorType::SAMPLED_IMAGE)
                    .descriptor_count(1)
                    .build(),
            ]),
        );
        let descriptor_set_layout = factory.create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo::builder().bindings(&[
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(0)
                    .descriptor_type(vk::DescriptorType::SAMPLER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                    .build(),
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(1)
                    .descriptor_type(vk::DescriptorType::SAMPLER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                    .build(),
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(2)
                    .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                    .build(),
            ]),
        );
        let descriptor_set = factory.allocate_descriptor_sets(
            &vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&[descriptor_set_layout])
                .build(),
        )[0];

        factory.update_descriptor_sets(
            &[
                vk::WriteDescriptorSet::builder()
                    .dst_set(descriptor_set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::SAMPLER)
                    .image_info(&[vk::DescriptorImageInfo::builder().sampler(point_sampler).build()])
                    .build(),
                vk::WriteDescriptorSet::builder()
                    .dst_set(descriptor_set)
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::SAMPLER)
                    .image_info(&[vk::DescriptorImageInfo::builder().sampler(linear_sampler).build()])
                    .build(),
                vk::WriteDescriptorSet::builder()
                    .dst_set(descriptor_set)
                    .dst_binding(2)
                    .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                    .image_info(&[vk::DescriptorImageInfo::builder()
                        .image_view(source_pass.get_color_image_view())
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .build()])
                    .build(),
            ],
            &[],
        );

        let pipeline_layout = factory.create_pipeline_layout(
            &vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(&[descriptor_set_layout])
                .push_constant_ranges(&[vk::PushConstantRange::builder()
                    .stage_flags(vk::ShaderStageFlags::VERTEX)
                    .offset(0)
                    .size(64)
                    .build()])
                .build(),
        );
        let pipeline = factory.create_graphics_pipelines(
            vk::PipelineCache::null(),
            &[vk::GraphicsPipelineCreateInfo::builder()
                .stages(&[post_process_vert.build(), post_process_frag.build()])
                .vertex_input_state(
                    &vk::PipelineVertexInputStateCreateInfo::builder()
                        .vertex_binding_descriptions(&[])
                        .build(),
                )
                .input_assembly_state(
                    &vk::PipelineInputAssemblyStateCreateInfo::builder()
                        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
                        .primitive_restart_enable(false)
                        .build(),
                )
                .tessellation_state(&Default::default())
                .viewport_state(
                    &vk::PipelineViewportStateCreateInfo::builder()
                        .viewport_count(1)
                        .scissor_count(1)
                        .build(),
                )
                .rasterization_state(
                    &vk::PipelineRasterizationStateCreateInfo::builder()
                        .line_width(1.0)
                        .build(),
                )
                .multisample_state(
                    &vk::PipelineMultisampleStateCreateInfo::builder()
                        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
                        .build(),
                )
                .depth_stencil_state(&Default::default())
                .color_blend_state(
                    &vk::PipelineColorBlendStateCreateInfo::builder().attachments(&[
                        vk::PipelineColorBlendAttachmentState::builder()
                            .blend_enable(false)
                            .color_write_mask(
                                vk::ColorComponentFlags::R
                                    | vk::ColorComponentFlags::G
                                    | vk::ColorComponentFlags::B
                                    | vk::ColorComponentFlags::A,
                            )
                            .build(),
                    ]),
                )
                .dynamic_state(
                    &vk::PipelineDynamicStateCreateInfo::builder()
                        .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR])
                        .build(),
                )
                .layout(pipeline_layout)
                .render_pass(destination_pass.get_render_pass())
                .subpass(0)
                .base_pipeline_handle(vk::Pipeline::null())
                .base_pipeline_index(0)
                .build()],
        )[0];

        Self {
            point_sampler,
            linear_sampler,
            descriptor_pool,
            descriptor_set_layout,
            descriptor_set,
            vert_module,
            frag_module,
            pipeline_layout,
            pipeline,
        }
    }

    pub fn destroy(&mut self, factory: &mut DeviceFactory) {
        factory.destroy_sampler(self.point_sampler);
        factory.destroy_sampler(self.linear_sampler);
        factory.destroy_descriptor_pool(self.descriptor_pool);
        factory.destroy_descriptor_set_layout(self.descriptor_set_layout);
        factory.destroy_shader_module(self.vert_module);
        factory.destroy_shader_module(self.frag_module);
        factory.destroy_pipeline_layout(self.pipeline_layout);
        factory.destroy_pipeline(self.pipeline);
    }

    pub fn render<T>(&self, screen_area: vk::Rect2D, frame_context: &FrameContext, destination_pass: &mut T)
    where
        T: RenderPass,
    {
        let command_buffer = destination_pass.get_command_buffer(frame_context);

        command_buffer.bind_pipeline(vk::PipelineBindPoint::GRAPHICS, self.pipeline);
        command_buffer.bind_descriptor_sets(
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline_layout,
            0,
            &[self.descriptor_set],
            &[],
        );
        command_buffer.set_viewport(
            0,
            &[vk::Viewport {
                x: screen_area.offset.x as _,
                y: screen_area.offset.y as _,
                width: screen_area.extent.width as _,
                height: screen_area.extent.height as _,
                min_depth: 0.0,
                max_depth: 1.0,
            }],
        );
        command_buffer.set_scissor(0, &[screen_area]);
        command_buffer.draw(3, 1, 0, 0);
    }
}
