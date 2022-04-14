use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use pollster::FutureExt;
use winit;

mod renderer;

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

    let context = renderer::GPUContext::new(&window).block_on()?;
    let renderer = renderer::Renderer::new(&context)?;

    let context = Arc::new(RwLock::new(context));
    let renderer = Arc::new(renderer);

    {
        let context = context.clone();
        thread::spawn(move || loop {
            context.read().unwrap().device().poll(wgpu::Maintain::Poll);
            thread::sleep(Duration::from_millis(100));
        });
    }

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
                    context.write().unwrap().resize_surface(size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    context.write().unwrap().resize_surface(*new_inner_size);
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(..) => {
                renderer
                    .render(&context.read().unwrap())
                    .block_on()
                    .unwrap();
            }
            _ => (),
        }
    });
}
