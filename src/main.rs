use std::sync::{Arc, RwLock};

use anyhow::{Context, Result};

mod renderer;

#[tokio::main]
async fn main() -> Result<()> {
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

    let renderer = Arc::new(RwLock::new(renderer::Renderer::new(&window).await?));

    {
        let renderer = renderer.clone();
        tokio::task::spawn_blocking(move || loop {
            renderer.read().unwrap().poll_device();
            std::thread::sleep(std::time::Duration::from_millis(100));
        });
    }

    tokio::task::block_in_place(move || {
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
                        renderer.write().unwrap().resize_surface(size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        renderer.write().unwrap().resize_surface(*new_inner_size);
                    }
                    _ => (),
                },
                Event::MainEventsCleared => {
                    window.request_redraw();
                }
                Event::RedrawRequested(..) => {
                    renderer.read().unwrap().render();
                }
                _ => (),
            }
        });
    })
}
