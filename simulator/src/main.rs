use minifb::{Key, Window, WindowOptions};

use girlvoice_ui_core::{
    DISPLAY_SIZE,
};

const SCALE: usize = 2;

fn main() {
    let window_size = DISPLAY_SIZE * SCALE;
    

    let mut buffer: Vec<u32> = vec![0; window_size * window_size];

    let mut window = Window::new(
        "Test - ESC to exit",
        window_size,
        window_size,
        WindowOptions::default(),
    )
    .unwrap_or_else(|e| {
        panic!("{}", e);
    });

    // Limit to max ~60 fps update rate
    window.set_target_fps(30);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        for i in buffer.iter_mut() {
            *i = 0; // write something more funny here!
        }

        // We unwrap here as we want this code to exit if it fails. Real applications may want to handle this in a different way
        window
            .update_with_buffer(&buffer, window_size, window_size)
            .unwrap();
    }
}