// Rust implementation of the girlvoice Amaranth DSP pipeline:
// - bandpass IIR filters for each frequency band
// - envelope followers to extract the amplitudes

use std::f32::consts::PI;

// same mel scale as girlvoice-gateware
fn mel(freq: f32) -> f32 {
    1127.0 * (1.0 + freq / 700.0).ln()
}

fn mel_to_freq(m: f32) -> f32 {
    700.0 * ((m / 1127.0).exp() - 1.0)
}

// second-order IIR butterworth bandpass filter (girlvoice/dsp/bandpass_iir.py)
pub struct BandpassIIR {
    // filter coefficients
    b: [f32; 3], // numerator (feedforward)
    a: [f32; 3], // denominator (feedback)
    
    // state
    x: [f32; 3], // input delay line
    y: [f32; 2]  // output delay line
}


impl BandpassIIR {
    pub fn new(low_freq: f32, high_freq: f32, sample_rate: f32, order: u32) -> Self { // order is the filter order (1 = 2nd order, 2 = 4th order)
        let nyq = sample_rate / 2.0;
        let low = low_freq / nyq;
        let high = high_freq / nyq;
        
        // bilinear transform
        let bw = high - low;
        let center = (low * high).sqrt();
        
        // prewrap
        let omega = (PI * center).tan();
        let bw_omega = (PI * bw).tan();
        
        let q = omega / bw_omega;
        let omega_sq = omega * omega;
        
        let norm = 1.0 + omega / q + omega_sq;
        
        let b0 = (omega / q) / norm;
        let b1 = 0.0;
        let b2 = -(omega / q) / norm;
        
        let a1 = 2.0 * (omega_sq - 1.0) / norm;
        let a2 = (1.0 - omega / q + omega_sq) / norm;
        
        Self {
            b: [b0, b1, b2],
            a: [1.0, a1, a2],
            x: [0.0; 3],
            y: [0.0; 2]
        }
    }

    // process a sample
    pub fn process(&mut self, input: f32) -> f32 {
        // shift input delay line
        self.x[2] = self.x[1];
        self.x[1] = self.x[0];
        self.x[0] = input;

        let output = self.b[0] * self.x[0] 
                   + self.b[1] * self.x[1] 
                   + self.b[2] * self.x[2]
                   - self.a[1] * self.y[0] 
                   - self.a[2] * self.y[1];

        // shift output delay line
        self.y[1] = self.y[0];
        self.y[0] = output;

        output
    }

    pub fn reset(&mut self) {
        self.x = [0.0; 3];
        self.y = [0.0; 2];
    }
}


// envelope follower using exponential smoothing (girlvoice/dsp/envelope.py)
pub struct EnvelopeFollower {
    value: f32,
    attack: f32,
    release: f32,
    attack_comp: f32,
    release_comp: f32
}

impl EnvelopeFollower {
    pub fn new(sample_rate: f32, attack_ms: f32, release_ms: f32) -> Self {  // attack_ms and release_ms are half-life times
        let attack_samples = sample_rate * attack_ms / 1000.0;
        let release_samples = sample_rate * release_ms / 1000.0;
        
        let attack = (-1.0 / attack_samples).exp();
        let release = (-1.0 / release_samples).exp();
        
        Self {
            value: 0.0,
            attack,
            release,
            attack_comp: 1.0 - attack,
            release_comp: 1.0 - release
        }
    }

    // process a sample
    pub fn process(&mut self, input: f32) -> f32 {
        let abs_input = input.abs();
        
        let (coeff, comp) = if abs_input > self.value {
            (self.attack, self.attack_comp)
        } else {
            (self.release, self.release_comp)
        };
        
        self.value = self.value * coeff + abs_input * comp;
        self.value
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn reset(&mut self) {
        self.value = 0.0;
    }
}

pub struct VocoderChannel {
    pub bandpass: BandpassIIR,
    pub envelope: EnvelopeFollower,
    pub center_freq: f32,
    pub low_freq: f32,
    pub high_freq: f32
}

impl VocoderChannel {
    pub fn new(low_freq: f32, high_freq: f32, sample_rate: f32) -> Self {
        let center_freq = (low_freq + high_freq) / 2.0;
        
        Self {
            bandpass: BandpassIIR::new(low_freq, high_freq, sample_rate, 1),
            envelope: EnvelopeFollower::new(sample_rate, 1.0, 25.0),
            center_freq,
            low_freq,
            high_freq
        }
    }

    // process a sample
    pub fn process(&mut self, input: f32) -> f32 {
        let filtered = self.bandpass.process(input);
        self.envelope.process(filtered)
    }
}


// multi-channel vocoder (mel-spaced frequency bands)
pub struct VocoderDSP {
    channels: Vec<VocoderChannel>,
    sample_rate: f32,
    peak_values: Vec<f32>,
    energies: Vec<f32> // smoothed output energies (0-1)
}

impl VocoderDSP {
    // vocoder DSP
    // - num_channels: number of frequency bands (8-16 for girlvoice)
    // - start_freq: lowest frequency band center (Hz)
    // - end_freq: highest frequency band center (Hz)
    // - sample_rate: audio sample rate (Hz)

    pub fn new(num_channels: usize, start_freq: f32, end_freq: f32, sample_rate: f32) -> Self {
        let start_mel = mel(start_freq);
        let end_mel = mel(end_freq);
        
        // calculate channel frequencies on mel scale
        let channel_mels: Vec<f32> = (0..num_channels)
            .map(|i| start_mel + (end_mel - start_mel) * (i as f32) / ((num_channels - 1) as f32))
            .collect();
        
        let channel_freqs: Vec<f32> = channel_mels.iter().map(|&m| mel_to_freq(m)).collect();
        
        // bandwidth parameter (from Stanford ECE Vocoder github)
        let bandwidth_param = 0.035;
        
        let channels: Vec<VocoderChannel> = channel_freqs
            .iter()
            .map(|&freq| {
                let low = freq * (1.0 - bandwidth_param);
                let high = freq * (1.0 + bandwidth_param);
                VocoderChannel::new(low, high, sample_rate)
            })
            .collect();

        println!("Using {} vocoder channels:", num_channels);
        for (i, ch) in channels.iter().enumerate() {
            println!("  Channel {}: {:.1} Hz ({:.1} - {:.1})", 
                     i, ch.center_freq, ch.low_freq, ch.high_freq);
        }

        Self {
            peak_values: vec![1.0; num_channels],
            energies: vec![0.0; num_channels],
            channels,
            sample_rate
        }
    }

    // process a sample. returns a slice of normalized energies (0-1) for each channel
    pub fn process(&mut self, sample: f32) -> &[f32] {
        for (i, channel) in self.channels.iter_mut().enumerate() {
            let envelope = channel.process(sample);
            
            if envelope > self.peak_values[i] {
                self.peak_values[i] = envelope;
            } else {
                // slow decay
                self.peak_values[i] *= 0.9999;
                self.peak_values[i] = self.peak_values[i].max(0.001);
            }
            
            self.energies[i] = (envelope / self.peak_values[i]).clamp(0.0, 1.0);
        }
        
        &self.energies
    }

    // process a buffer of samples and return energies
    pub fn process_buffer(&mut self, samples: &[f32]) -> &[f32] {
        for &sample in samples {
            self.process(sample);
        }
        &self.energies
    }

    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    pub fn energies(&self) -> &[f32] {
        &self.energies
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }
}