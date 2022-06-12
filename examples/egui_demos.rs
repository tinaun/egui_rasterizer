use egui::Context;
use egui_rasterizer::TinySkiaBackend;

fn run_software(mut ui: impl FnMut(&Context) + 'static) {
    use egui_winit::winit::{
        self,
        event_loop::{ControlFlow, EventLoop},
    };
    use softbuffer::GraphicsContext;

    let ev_loop = EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Test Render")
        .build(&ev_loop)
        .unwrap();

    let mut gc = unsafe { GraphicsContext::new(window) }.unwrap();
    let mut rasterizer = TinySkiaBackend::new();
    let mut state = egui_winit::State::new(&ev_loop);
    state.set_pixels_per_point(2.0);

    ev_loop.run(move |ev, _, control_flow| {
        use winit::event::{Event, WindowEvent};

        *control_flow = ControlFlow::Wait;

        match ev {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent { event, .. } => {
                //println!("{:?}", event);
                if !state.on_event(&rasterizer.context(), &event) {}
                gc.window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                let input = state.take_egui_input(&gc.window());
                //println!("{:?}", input);
                let (_output, needs_repaint, pixmap) = rasterizer.output_to_pixmap(input, &mut ui);
                let w = pixmap.width() as u16;
                let h = pixmap.height() as u16;
                let data = pixmap.data().as_ptr() as *const u32;
                let new_data = unsafe { std::slice::from_raw_parts(data, pixmap.data().len() / 4) };

                gc.set_buffer(new_data, w, h);

                if needs_repaint {
                    gc.window().request_redraw();
                }
            }
            _ => {}
        }
    })
}

fn main() {
    let mut demos = egui_demo_lib::DemoWindows::default();
    run_software(move |ctx| demos.ui(ctx));
}