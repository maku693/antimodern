use std::{
    sync::{Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use anyhow::{Context, Result};
use tokio::task;
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod renderer;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Hello, world!")
        .with_inner_size(LogicalSize::<u32> {
            width: 640,
            height: 360,
        })
        .build(&event_loop)
        .context("Failed to build window")?;

    let renderer = Arc::new(RwLock::new(renderer::Renderer::new(&window).await?));

    {
        let renderer = renderer.clone();
        task::spawn_blocking(move || loop {
            renderer.read().unwrap().poll_device();
            sleep(Duration::from_millis(100));
        });
    }

    tokio::task::block_in_place(move || {
        event_loop.run(move |e, _, control_flow| {
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
