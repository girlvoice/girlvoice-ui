mod dsp;

use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use minifb::{Key, Window, WindowOptions};

use dsp::VocoderDSP;

use girlvoice_ui_core::{
    DISPLAY_SIZE,
};

const SCALE: usize = 2;

// shared between dsp and main UI thread
struct SharedState {
    energies: Vec<f32>,
    peak_level: f32,
}

impl SharedState {
    fn new(num_channels: usize) -> Self {
        Self {
            energies: vec![0.0; num_channels],
            peak_level: 0.0,
        }
    }
}


fn main() {
    println!("### Girlvoice Vocoder UI Simulator");
    println!();

    // simulator UI
    let window_size = DISPLAY_SIZE * SCALE;
    
    let num_channels = 12;
    let start_freq = 100.0;
    let end_freq = 3000.0;

    // audio init
    let host = cpal::default_host();
    let device = host.default_input_device().expect("No input device available");
    println!("Using input device: {}", device.name().unwrap());

    let config = device.default_input_config().expect("No input config available");
    println!("Audio config: {:?}", config);

    let sample_rate = config.sample_rate() as f32;
    let channels = config.channels() as usize;

    let shared = Arc::new(Mutex::new(SharedState::new(num_channels)));
    let shared_audio = Arc::clone(&shared);

    let analyzer = Arc::new(Mutex::new(VocoderDSP::new(
        num_channels, start_freq, end_freq, sample_rate,
    )));
    let analyzer_audio = Arc::clone(&analyzer);


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