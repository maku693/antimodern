use std::mem::size_of_val;
use std::time::Instant;

use anyhow::{Context, Ok, Result};
use bytemuck::{bytes_of, from_bytes};
use futures::future::FutureExt;
use glam::{vec3, Mat3, Mat4, Vec3};
use wgpu::util::DeviceExt;

pub struct GPUContext {
    surface: wgpu::Surface,
    surface_configuration: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GPUContext {
    pub async fn new(window: &winit::window::Window) -> Result<GPUContext> {
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);

        let surface = unsafe { instance.create_surface(window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("No adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("No device found")?;

        let surface_configuration = {
            let surface_format = surface
                .get_preferred_format(&adapter)
                .context("There is no preferred format")?;

            let inner_size = window.inner_size();

            wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: inner_size.width,
                height: inner_size.height,
                present_mode: wgpu::PresentMode::Fifo,
            }
        };
        surface.configure(&device, &surface_configuration);

        Ok(GPUContext {
            surface,
            surface_configuration,
            device,
            queue,
        })
    }

    pub fn surface(&self) -> &wgpu::Surface {
        &self.surface
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_configuration.format
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn resize_surface(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface_configuration.width = size.width;
        self.surface_configuration.height = size.height;
        self.surface
            .configure(&self.device, &self.surface_configuration)
    }
}

const NUM_MAX_INFLIGHT_BUFFERS: usize = 3;

pub struct Renderer {
    frame: futures::lock::Mutex<usize>,

    vertex_buffer: wgpu::Buffer,
    num_vertices: u32,
    instance_buffer: [wgpu::Buffer; NUM_MAX_INFLIGHT_BUFFERS],
    num_instances: u32,
    uniform_buffer: wgpu::Buffer,

    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    pub fn new(context: &GPUContext) -> Result<Renderer> {
        let device = context.device();

        let frame = futures::lock::Mutex::new(3);

        let vertices = [
            vec3(-0.1f32, -0.1, 0.),
            vec3(0., 0.1, 0.),
            vec3(0.1, -0.1, 0.),
        ];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes_of(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let num_vertices = vertices.len() as u32;

        let instances = [vec3(0f32, 0., 0.), vec3(-0.5, 0., 0.), vec3(0.5, 0., 0.)];
        let instance_buffer_desc = wgpu::BufferDescriptor {
            label: None,
            size: size_of_val(&instances) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::MAP_READ
                | wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: false,
        };
        let instance_buffer = [
            device.create_buffer(&instance_buffer_desc),
            device.create_buffer(&instance_buffer_desc),
            device.create_buffer(&instance_buffer_desc),
        ];

        let num_instances = instances.len() as u32;

        let proj_matrix = Mat4::orthographic_lh(-1f32, 1., -1., 1., 0., 1.);
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes_of(&proj_matrix),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size_of_val(&proj_matrix) as u64),
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let render_pipeline = {
            let vertex_buffer_layouts = [
                wgpu::VertexBufferLayout {
                    array_stride: size_of_val(&vertices[0]) as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                },
                wgpu::VertexBufferLayout {
                    array_stride: size_of_val(&instances[0]) as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 1,
                    }],
                },
            ];

            let shader_module = device.create_shader_module(&wgpu::include_wgsl!("main.wgsl"));

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: "vs_main",
                    buffers: &vertex_buffer_layouts,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader_module,
                    entry_point: "fs_main",
                    targets: &[context.surface_format().into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };

        Ok(Renderer {
            frame,
            vertex_buffer,
            num_vertices,
            instance_buffer,
            num_instances,
            uniform_buffer,
            bind_group,
            render_pipeline,
        })
    }

    pub async fn render(&mut self, context: &GPUContext) -> Result<()> {
        let mut frame = *self.frame.lock().await;
        frame = (frame + 1) % NUM_MAX_INFLIGHT_BUFFERS;
        *self.frame.get_mut() = frame;

        let now = Instant::now();
        log::info!("frame {}: begin", frame);

        let instance_buffer = &self.instance_buffer[frame];

        let instance_buffer_slice = instance_buffer.slice(..);

        instance_buffer_slice.map_async(wgpu::MapMode::Read).await?;
        let mut vertices =
            from_bytes::<[Vec3; 3]>(&instance_buffer_slice.get_mapped_range()).clone();
        for v in vertices.iter_mut() {
            *v = Mat3::from_rotation_y(0.1) * *v;
        }
        instance_buffer.unmap();

        instance_buffer_slice
            .map_async(wgpu::MapMode::Write)
            .await?;
        instance_buffer_slice
            .get_mapped_range_mut()
            .copy_from_slice(bytes_of(&vertices));
        instance_buffer.unmap();

        let frame_buffer = context
            .surface()
            .get_current_texture()
            .expect("Failed to get next surface texture");

        let frame_buffer_view = frame_buffer
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = context
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &frame_buffer_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
            render_pass.draw(0..self.num_vertices, 0..self.num_instances);
        }

        context.queue().submit(Some(encoder.finish()));

        frame_buffer.present();

        log::info!(
            "frame {} present: elapsed: {}ms",
            frame,
            now.elapsed().as_millis()
        );

        context
            .queue()
            .on_submitted_work_done()
            .then(|_| async move {
                log::info!(
                    "frame {} submitted work done: elapsed: {}ms",
                    frame,
                    now.elapsed().as_millis()
                );
            })
            .await;

        Ok(())
    }
}
