use std::collections::BTreeMap;
use wgpu::util::DeviceExt;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    screen: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    textures: BTreeMap<u64, wgpu::BindGroup>,
}
fn float_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}
impl Renderer {
    pub async fn new(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> Result<Self, String> {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| e.to_string())?;
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("logical screen size"),
            contents: &float_bytes(&[1., 1., 0., 0.]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let screen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("screen"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let screen = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &screen_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("owned ImGui renderer"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&screen_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ImGui"), layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 20, step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4],
            })] },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview_mask: None, cache: None,
        });
        Ok(Self {
            device,
            queue,
            pipeline,
            uniform,
            screen,
            texture_layout,
            textures: BTreeMap::new(),
        })
    }
    pub fn texture(&mut self, id: u64, width: u32, height: u32, pixels: &[u8]) {
        assert_eq!(pixels.len(), width as usize * height as usize * 4);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("UI texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            texture.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            texture.size(),
        );
        let view = texture.create_view(&Default::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        self.textures.insert(id, group);
    }
    pub fn render(
        &self,
        view: &wgpu::TextureView,
        data: &bite_imgui::DrawData,
        width: u32,
        height: u32,
        scale: f32,
    ) {
        self.queue.write_buffer(
            &self.uniform,
            0,
            &float_bytes(&[width as f32 / scale, height as f32 / scale, 0., 0.]),
        );
        let buffers: Vec<_> = data
            .iter()
            .map(|list| {
                let mut vertices = Vec::with_capacity(list.vertices.len() * 20);
                for v in &list.vertices {
                    vertices.extend(float_bytes(&[v.pos[0], v.pos[1], v.uv[0], v.uv[1]]));
                    vertices.extend(v.color.to_ne_bytes());
                }
                let indices: Vec<u8> = list.indices.iter().flat_map(|i| i.to_ne_bytes()).collect();
                (
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: &vertices,
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: &indices,
                            usage: wgpu::BufferUsages::INDEX,
                        }),
                )
            })
            .collect();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.025,
                            g: 0.03,
                            b: 0.04,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.screen, &[]);
            for (list, (vertices, indices)) in data.iter().zip(&buffers) {
                pass.set_vertex_buffer(0, vertices.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                for cmd in &list.commands {
                    let Some(texture) = self.textures.get(&cmd.texture) else {
                        continue;
                    };
                    let x = (cmd.clip[0] * scale).max(0.).floor() as u32;
                    let y = (cmd.clip[1] * scale).max(0.).floor() as u32;
                    let right = ((cmd.clip[2] * scale).ceil() as u32).min(width);
                    let bottom = ((cmd.clip[3] * scale).ceil() as u32).min(height);
                    if right <= x || bottom <= y {
                        continue;
                    }
                    pass.set_scissor_rect(x, y, right - x, bottom - y);
                    pass.set_bind_group(1, texture, &[]);
                    pass.draw_indexed(
                        cmd.index_offset..cmd.index_offset + cmd.count,
                        cmd.vertex_offset as i32,
                        0..1,
                    );
                }
            }
        }
        self.queue.submit([encoder.finish()]);
    }
}
