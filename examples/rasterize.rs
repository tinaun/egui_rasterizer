use egui::Context;
use egui_rasterizer::rasterize;

fn example_ui(ctx: &Context) {
    egui::Window::new("Hello world!").show(&ctx, |ui| {
        ui.label("Hello software rendering!");
        if ui.button("Click me").clicked() {
            // take some action here
        }

        ui.separator();
        ui.hyperlink("https://example.org");

        #[derive(PartialEq)]
        enum Enum {
            First,
            Second,
            Third,
        }
        let mut my_enum = Enum::First;
        ui.horizontal(|ui| {
            ui.radio_value(&mut my_enum, Enum::First, "First");
            ui.radio_value(&mut my_enum, Enum::Second, "Second");
            ui.radio_value(&mut my_enum, Enum::Third, "Third");
        });
    });
}

fn main() {
    let time = std::time::Instant::now();
    let pixmap = rasterize((1024, 768), |ctx| {
        ctx.set_visuals(egui::Visuals::light());
        example_ui(ctx)
    });
    let elapsed = time.elapsed();
    println!("{:?} elapsed", elapsed);

    pixmap.save_png("tmp/test.png").unwrap();
}
