use macroquad::prelude::*;

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        clear_background(WHITE);

        draw_text("HNSW Demo GUI", 20.0, 40.0, 30.0, BLACK);

        draw_text(
            "This is a placeholder for the HNSW demo GUI.",
            20.0,
            80.0,
            20.0,
            DARKGRAY,
        );

        // Update the screen
        next_frame().await;
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Ghost Engine".to_owned(),
        window_width: 800,
        window_height: 600,
        window_resizable: false,
        ..Default::default()
    }
}
