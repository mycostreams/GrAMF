// mod app;
// mod render;

use GrAMF::App;
// use winit::event_loop::{ControlFlow, EventLoop};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = winit::event_loop::EventLoop::builder().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut application = App::default();
    event_loop.run_app(&mut application)?;
    Ok(())
}
