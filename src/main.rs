use anyhow::{Context, Result};
use glam::{vec3, Mat4};
use pollster::FutureExt;
use wgpu::util::DeviceExt;
use winit;

fn main() -> Result<()> {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new();

    let window = winit::window::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_inner_size(winit::dpi::LogicalSize::<u32> {
            width: 640,
            height: 360,
        })
        .build(&event_loop)
        .context("Failed to build window")?;

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);

    let surface = unsafe { instance.create_surface(&window) };

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .block_on()
        .context("No adapter found")?;

    let surface_format = surface
        .get_preferred_format(&adapter)
        .context("There is no preferred format")?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
            },
            None,
        )
        .block_on()
        .context("No device found")?;

    configure_surface(&device, &surface, surface_format, window.inner_size())?;

    let vertices = [
        vec3(-0.1f32, -0.1, 0.),
        vec3(0., 0.1, 0.),
        vec3(0.1, -0.1, 0.),
    ];
    let num_vertices = vertices.len() as u32;

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::bytes_of(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let instances = [vec3(0f32, 0., 0.), vec3(-0.5, 0., 0.), vec3(0.5, 0., 0.)];
    let num_instances = instances.len() as u32;

    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::bytes_of(&instances),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let proj_matrix = Mat4::orthographic_lh(-1f32, 1., -1., 1., 0., 1.);

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::bytes_of(&proj_matrix),
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
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of_val(&proj_matrix) as u64),
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
                array_stride: std::mem::size_of_val(&vertices[0]) as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            },
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of_val(&instances[0]) as wgpu::BufferAddress,
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
                targets: &[surface_format.into()],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    };

    event_loop.run(move |e, _, control_flow| {
        use winit::{
            event::{Event, WindowEvent},
            event_loop::ControlFlow,
        };

        *control_flow = ControlFlow::Poll;

        match e {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => {
                    configure_surface(&device, &surface, surface_format, size).unwrap();
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    configure_surface(&device, &surface, surface_format, *new_inner_size).unwrap();
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(..) => {
                let frame_buffer = surface
                    .get_current_texture()
                    .expect("Failed to get next surface texture");

                let frame_buffer_view = frame_buffer
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

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
                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.set_pipeline(&render_pipeline);
                    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
                    render_pass.draw(0..num_vertices, 0..num_instances);
                }

                queue.submit(Some(encoder.finish()));

                frame_buffer.present();
            }
            _ => (),
        }
    });
}

fn configure_surface(
    device: &wgpu::Device,
    surface: &wgpu::Surface,
    surface_format: wgpu::TextureFormat,
    size: winit::dpi::PhysicalSize<u32>,
) -> Result<()> {
    surface.configure(
        device,
        &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        },
    );

    Ok(())
}
