use tephrite_rs::ui::text_bake::{CpuTextBaker, TextStyle};

fn main() {
    let mut baker = CpuTextBaker::new();

    let style = TextStyle::default();

    let image = baker.bake_rgba8("This is a test\nHello.", style).unwrap();

    let image = image.try_into_dynamic().unwrap();

    image.save("/tmp/texture.png").unwrap();
}
