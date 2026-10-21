// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use GrAMF::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = winit::event_loop::EventLoop::builder().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut application = App::new(std::env::args().nth(1).map(Into::into));
    event_loop.run_app(&mut application)?;
    Ok(())
}
