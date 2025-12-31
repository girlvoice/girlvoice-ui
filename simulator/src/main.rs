mod dsp;

use std::sync::{Arc, Mutex};
use std::time::Instant; // for shader time, would be replaced by timer on MCU

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use minifb::{Key, Window, WindowOptions, Scale};

use dsp::VocoderDSP;

use girlvoice_ui_core::{
    Visualizer, Color, ColorPalette, palette, DISPLAY_SIZE
};

const SCALE: usize = 2;

// shared between DSP and main UI thread
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
    
    // simulator vocoder DSP
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

    // from fft example
    let audio_callback = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        let mut analyzer = analyzer_audio.lock().unwrap();
        let mut shared = shared_audio.lock().unwrap();
        
        let mut peak = 0.0f32;
        for frame in data.chunks(channels) {
            let sample = if channels > 1 {
                frame.iter().sum::<f32>() / channels as f32
            } else {
                frame[0]
            };
            peak = peak.max(sample.abs());
            analyzer.process(sample);
        }
        
        shared.energies.copy_from_slice(analyzer.energies());
        shared.peak_level = shared.peak_level * 0.9 + peak * 0.1;
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            audio_callback,
            |err| eprintln!("Audio error: {}", err),
            None
        ).unwrap(),
        cpal::SampleFormat::I16 => {
            let analyzer_audio = Arc::clone(&analyzer);
            let shared_audio = Arc::clone(&shared);
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut analyzer = analyzer_audio.lock().unwrap();
                    let mut shared = shared_audio.lock().unwrap();
                    let mut peak = 0.0f32;
                    for frame in data.chunks(channels) {
                        let sample = if channels > 1 {
                            frame.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / channels as f32
                        } else {
                            frame[0] as f32 / 32768.0
                        };
                        peak = peak.max(sample.abs());
                        analyzer.process(sample);
                    }
                    shared.energies.copy_from_slice(analyzer.energies());
                    shared.peak_level = shared.peak_level * 0.9 + peak * 0.1; // moving avg
                },
                |err| eprintln!("Audio error: {}", err),
                None
            ).unwrap()
        },
        format => panic!("Unsupported sample format: {:?}", format)
    };

    stream.play().expect("Audio stream failed");
    println!("Audio stream started\n");

    
    let mut window = Window::new(
        "Girlvoice Visualizer - ESC to exit",
        window_size,
        window_size,
        WindowOptions { scale: Scale::X1, ..Default::default() }
    )
    .unwrap_or_else(|e| {
        panic!("{}", e);
    });

    window.set_target_fps(30);

    let mut visualizer = Visualizer::new(num_channels);
    let mut framebuffer = vec![0u32; DISPLAY_SIZE * DISPLAY_SIZE];

    let mut last_frame = Instant::now();


    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32();
        last_frame = now;
       
        let energies = {
            let shared = shared.lock().unwrap();
            shared.energies.clone()
        };

        // run main shader
        visualizer.update(dt, &energies);

        // fade buffer for trails
        let fade = 0.7;
        for pixel in framebuffer.iter_mut() {
            let r = ((*pixel >> 16) & 0xFF) as f32 * fade;
            let g = ((*pixel >> 8) & 0xFF) as f32 * fade;
            let b = (*pixel & 0xFF) as f32 * fade;
            *pixel = 0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        }

        let vis_brightness = 1.0;
        visualizer.render(|x, y, color| {
            if x < DISPLAY_SIZE && y < DISPLAY_SIZE {
                let idx = y * DISPLAY_SIZE + x;
                let dimmed = color.scale(vis_brightness);
                let existing = framebuffer[idx];
                let er = ((existing >> 16) & 0xFF) as u32;
                let eg = ((existing >> 8) & 0xFF) as u32;
                let eb = (existing & 0xFF) as u32;
                let nr = (er + dimmed.r as u32).min(255);
                let ng = (eg + dimmed.g as u32).min(255);
                let nb = (eb + dimmed.b as u32).min(255);
                framebuffer[idx] = 0xFF000000 | (nr << 16) | (ng << 8) | nb;
            }
        });

        draw_level_meters(&mut framebuffer, &energies);

        // scale up screen
        let scaled_framebuffer: Vec<u32> = if SCALE > 1 {
            let mut scaled = vec![0u32; window_size * window_size];
            for y in 0..DISPLAY_SIZE {
                for x in 0..DISPLAY_SIZE {
                    let color = framebuffer[y * DISPLAY_SIZE + x];
                    for sy in 0..SCALE {
                        for sx in 0..SCALE {
                            scaled[(y * SCALE + sy) * window_size + (x * SCALE + sx)] = color;
                        }
                    }
                }
            }
            scaled
        } else {
            framebuffer.clone()
        };

        window
            .update_with_buffer(&scaled_framebuffer, window_size, window_size)
            .unwrap();
    }
}


fn draw_level_meters(framebuffer: &mut [u32], energies: &[f32]) {
    let meter_width = 4;
    let meter_height = 40;
    let spacing = 2;
    let (start_x, start_y) = (5, 5);
    
    for (i, &energy) in energies.iter().enumerate() {
        let x = start_x + (i % 16) * (meter_width + spacing);
        let y = start_y;
        
        for dy in 0..meter_height {
            for dx in 0..meter_width {
                let (px, py) = (x + dx, y + dy);
                if px < DISPLAY_SIZE && py < DISPLAY_SIZE {
                    framebuffer[py * DISPLAY_SIZE + px] = 0xFF202020;
                }
            }
        }
        
        let level_height = (energy * meter_height as f32) as usize;
        let color = palette::rainbow(i as f32 / energies.len() as f32);
        for dy in 0..level_height {
            for dx in 0..meter_width {
                let (px, py) = (x + dx, y + meter_height - 1 - dy);
                if px < DISPLAY_SIZE && py < DISPLAY_SIZE {
                    framebuffer[py * DISPLAY_SIZE + px] = color.to_argb32();
                }
            }
        }
    }
}