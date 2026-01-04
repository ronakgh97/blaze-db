use macroquad::prelude::*;

#[allow(dead_code)]
struct MainState {}

fn _window_conf() -> Conf {
    Conf {
        window_title: String::from("NSW DEMO"),
        window_width: 1200,
        window_height: 800,
        window_resizable: false,
        sample_count: 256,
        ..Default::default()
    }
}

#[allow(unused)]
#[macroquad::main(_window_conf)]
async fn main() {
    let state = MainState {};

    loop {
        clear_background(BLACK);

        render_main(&state).await;

        // Update the screen
        next_frame().await;
    }
}

#[allow(unused)]
async fn render_main(state: &MainState) {
    let screen_width = screen_width();
    let screen_height = screen_height();
}
