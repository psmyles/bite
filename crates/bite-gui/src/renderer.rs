use std::collections::BTreeMap;
use wgpu::util::DeviceExt;

/// One bound texture, and whether it holds color or the single channel coverage the font
/// atlas is made of, which the two pipelines read differently.
struct Texture {
    group: wgpu::BindGroup,
    coverage: bool,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    coverage_pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    screen: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    textures: BTreeMap<u64, Texture>,
    /// Reused between frames so that a redraw does not allocate a buffer per draw list.
    vertex_scratch: Vec<u8>,
    index_scratch: Vec<u8>,
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
        let make_pipeline = |entry: &'static str| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ImGui"), layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 20, step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4],
            })] },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(entry), compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview_mask: None, cache: None,
        }); 
        let pipeline = make_pipeline("fs");
        let coverage_pipeline = make_pipeline("fs_coverage");
        Ok(Self {
            device,
            queue,
            pipeline,
            coverage_pipeline,
            uniform,
            screen,
            texture_layout,
            textures: BTreeMap::new(),
            vertex_scratch: Vec::new(),
            index_scratch: Vec::new(),
        })
    }
    /// Uploads four channel color, which is what decoded images are.
    pub fn texture(&mut self, id: u64, width: u32, height: u32, pixels: &[u8]) {
        self.upload(id, width, height, pixels, wgpu::TextureFormat::Rgba8Unorm, 4);
    }

    /// Uploads one channel coverage, which is what the font atlas is.
    pub fn coverage_texture(&mut self, id: u64, width: u32, height: u32, pixels: &[u8]) {
        self.upload(id, width, height, pixels, wgpu::TextureFormat::R8Unorm, 1);
    }

    fn upload(
        &mut self,
        id: u64,
        width: u32,
        height: u32,
        pixels: &[u8],
        format: wgpu::TextureFormat,
        bytes_per_pixel: u32,
    ) {
        assert_eq!(
            pixels.len(),
            width as usize * height as usize * bytes_per_pixel as usize
        );
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
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            texture.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * bytes_per_pixel),
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
        self.textures.insert(
            id,
            Texture {
                group,
                coverage: bytes_per_pixel == 1,
            },
        );
    }

    /// Honours the texture work Dear ImGui asked for after the frame it belongs to.
    ///
    /// Since 1.92 the font atlas is the library's to grow rather than the host's to build: a
    /// frame that draws a size or a character no earlier frame did arrives with an upload
    /// attached. The requests carry their own identifiers, in a half of the identifier space
    /// that cannot collide with the editor's own images, so there is nothing to reconcile - an
    /// upload replaces whatever was there and a destroy drops it.
    pub fn apply_texture_requests(&mut self, requests: &[bite_imgui::TextureRequest]) {
        for request in requests {
            match &request.action {
                bite_imgui::TextureAction::Upload {
                    width,
                    height,
                    coverage,
                    pixels,
                } => {
                    if *coverage {
                        self.coverage_texture(request.id, *width, *height, pixels);
                    } else {
                        self.texture(request.id, *width, *height, pixels);
                    }
                }
                bite_imgui::TextureAction::Destroy => self.free_texture(request.id),
            }
        }
    }

    /// Releases a texture, which the filmstrip does when a branch's thumbnails are replaced.
    pub fn free_texture(&mut self, id: u64) {
        self.textures.remove(&id);
    }

    /// Releases every texture whose identifier the predicate accepts.
    pub fn free_textures(&mut self, mut keep: impl FnMut(u64) -> bool) {
        self.textures.retain(|id, _| keep(*id));
    }

    pub fn has_texture(&self, id: u64) -> bool {
        self.textures.contains_key(&id)
    }
    pub fn render(
        &mut self,
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
        let mut buffers: Vec<(wgpu::Buffer, wgpu::Buffer)> = Vec::with_capacity(data.len());
        for list in data {
            self.vertex_scratch.clear();
            self.index_scratch.clear();
            for vertex in &list.vertices {
                self.vertex_scratch.extend(float_bytes(&[
                    vertex.pos[0],
                    vertex.pos[1],
                    vertex.uv[0],
                    vertex.uv[1],
                ]));
                self.vertex_scratch.extend(vertex.color.to_ne_bytes());
            }
            for index in &list.indices {
                self.index_scratch.extend(index.to_ne_bytes());
            }
            if self.vertex_scratch.is_empty() || self.index_scratch.is_empty() {
                continue;
            }
            buffers.push((
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: &self.vertex_scratch,
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: &self.index_scratch,
                        usage: wgpu::BufferUsages::INDEX,
                    }),
            ));
        }
        let drawable: Vec<_> = data
            .iter()
            .filter(|list| !list.vertices.is_empty() && !list.indices.is_empty())
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
                        // The shell gap color, so resizing never flashes an unpainted edge.
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(crate::theme::GAP_COLOR.0[0]),
                            g: f64::from(crate::theme::GAP_COLOR.0[1]),
                            b: f64::from(crate::theme::GAP_COLOR.0[2]),
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_bind_group(0, &self.screen, &[]);
            // Remembered so that a run of text, or of images, costs one pipeline switch.
            let mut bound: Option<bool> = None;
            for (list, (vertices, indices)) in drawable.iter().zip(&buffers) {
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
                    if bound != Some(texture.coverage) {
                        pass.set_pipeline(if texture.coverage {
                            &self.coverage_pipeline
                        } else {
                            &self.pipeline
                        });
                        bound = Some(texture.coverage);
                    }
                    pass.set_bind_group(1, &texture.group, &[]);
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
