//! Synthesizes every sound the app plays, from sines and noise, as WAV files:
//!
//!     cargo run --release --example make_sounds
//!     scripts/make-sounds.sh        # the same, then packs the two long clips as AAC
//!
//! `trip` is the whole flight as one clip, so its parts stay in step with the picture: the drive
//! spooling up, the jump, the tunnel, the drop out of it and a chime as the ship settles. The
//! timings below are `voyage.rs`'s.

use std::f32::consts::{PI, TAU};
use std::fs;
use std::io::Write;
use std::path::Path;

const RATE: f32 = 44_100.0;
const SPOOL: f32 = 1.6;
const WARP: f32 = 4.6;
const ARRIVE: f32 = 2.4;

/// White noise in -1..1, the same every run.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// A one-pole low-pass whose cutoff can move every sample.
#[derive(Default)]
struct LowPass(f32);

impl LowPass {
    fn run(&mut self, x: f32, cutoff: f32) -> f32 {
        self.0 += (1.0 - (-TAU * cutoff / RATE).exp()) * (x - self.0);
        self.0
    }
}

/// A state-variable band-pass whose centre can move every sample.
#[derive(Default)]
struct BandPass {
    low: f32,
    band: f32,
}

impl BandPass {
    fn run(&mut self, x: f32, centre: f32, q: f32) -> f32 {
        let f = 2.0 * (PI * centre.min(7000.0) / RATE).sin();
        self.low += f * self.band;
        let high = x - self.low - self.band / q;
        self.band += f * high;
        self.band
    }
}

/// A stereo clip. `wet` is what goes to the reverb.
struct Clip {
    dry: [Vec<f32>; 2],
    wet: [Vec<f32>; 2],
}

impl Clip {
    fn new(seconds: f32) -> Clip {
        let n = (seconds * RATE) as usize;
        Clip { dry: [vec![0.0; n], vec![0.0; n]], wet: [vec![0.0; n], vec![0.0; n]] }
    }

    /// Adds a layer from `start` for `seconds`: `voice` gets the time since `start` and returns
    /// the left and right samples, of which `send` also goes to the reverb.
    fn layer(&mut self, start: f32, seconds: f32, send: f32, mut voice: impl FnMut(f32) -> [f32; 2]) {
        let first = (start * RATE) as usize;
        let count = (seconds * RATE) as usize;
        for i in 0..count {
            let Some(at) = first.checked_add(i).filter(|at| *at < self.dry[0].len()) else { break };
            for (ch, v) in voice(i as f32 / RATE).into_iter().enumerate() {
                self.dry[ch][at] += v;
                self.wet[ch][at] += v * send;
            }
        }
    }

    /// Mixes in the reverb, takes out anything below hearing, rounds off the peaks, sets the
    /// level and fades the very end.
    fn finish(mut self, peak: f32, tail: f32) -> [Vec<f32>; 2] {
        for ch in 0..2 {
            let wet = reverb(&self.wet[ch], ch);
            let mut floor = LowPass::default();
            for (d, w) in self.dry[ch].iter_mut().zip(wet) {
                let v = *d + w;
                *d = (v - floor.run(v, 22.0)).tanh();
            }
        }
        let top = self.dry.iter().flatten().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
        let n = self.dry[0].len();
        let fade = (tail * RATE) as usize;
        for ch in 0..2 {
            for (i, v) in self.dry[ch].iter_mut().enumerate() {
                let out = if fade > 0 && i + fade >= n { (n - i) as f32 / fade as f32 } else { 1.0 };
                *v *= peak / top * out;
            }
        }
        self.dry
    }
}

/// Four damped combs into two all-passes, a little longer on the right so the tail is wide.
fn reverb(input: &[f32], channel: usize) -> Vec<f32> {
    let spread = channel * 23;
    let mut out = vec![0.0; input.len()];
    for delay in [1557, 1617, 1491, 1422] {
        let mut line = vec![0.0f32; delay + spread];
        let (mut at, mut damp) = (0, 0.0f32);
        for (o, x) in out.iter_mut().zip(input) {
            let y = line[at];
            damp = y * 0.75 + damp * 0.25;
            line[at] = x + damp * 0.86;
            at = (at + 1) % line.len();
            *o += y * 0.25;
        }
    }
    for delay in [225, 556] {
        let mut line = vec![0.0f32; delay + spread];
        let mut at = 0;
        for o in out.iter_mut() {
            let held = line[at];
            let y = held - 0.5 * *o;
            line[at] = *o + 0.5 * held;
            at = (at + 1) % line.len();
            *o = y;
        }
    }
    out
}

/// Equal-power left and right gains for `pan` in -1..1.
fn pan(pan: f32) -> [f32; 2] {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * PI / 4.0;
    [a.cos(), a.sin()]
}

/// A phase that advances by a frequency that may change every sample.
#[derive(Default)]
struct Phase(f32);

impl Phase {
    fn step(&mut self, hz: f32) -> f32 {
        self.0 = (self.0 + hz / RATE).fract();
        self.0 * TAU
    }
}

/// A buzz: `count` harmonics falling off as 1/h.
fn buzz(phase: f32, count: usize) -> f32 {
    (1..=count).map(|h| (phase * h as f32).sin() / h as f32).sum()
}

/// The drive charging: a whine that climbs three and a half octaves over a bass note climbing
/// after it, fluttering faster as it goes, with air rushing up behind.
fn spool(clip: &mut Clip, start: f32) {
    let (mut tone, mut fifth, mut flutter) = ([Phase::default(), Phase::default()], [Phase::default(), Phase::default()], Phase::default());
    let (mut bass, mut dull) = ([Phase::default(), Phase::default()], [LowPass::default(), LowPass::default()]);
    let (mut hiss, mut bands) = ([Noise(11), Noise(29)], [BandPass::default(), BandPass::default()]);
    clip.layer(start, SPOOL + 0.04, 0.25, |t| {
        let x = (t / SPOOL).min(1.0);
        let hz = 60.0 * 2f32.powf(x * 3.6);
        let cut = ((SPOOL + 0.04 - t) / 0.04).clamp(0.0, 1.0);
        let beat = 1.0 - 0.38 * (0.5 + 0.5 * flutter.step(5.0 + 23.0 * x * x).sin());
        std::array::from_fn(|ch| {
            let detune = 1.0 + 0.006 * ch as f32;
            let whine = buzz(tone[ch].step(hz * detune), 6) + 0.5 * buzz(fifth[ch].step(hz * 1.5 * detune), 3);
            let air = bands[ch].run(hiss[ch].next(), 300.0 * 2f32.powf(x * 4.5), 2.5);
            // The charge building underneath: a low note climbing an octave and a half.
            let swell = dull[ch].run(buzz(bass[ch].step((40.0 + 70.0 * x * x) * detune), 8), 420.0) * 0.34 * (0.25 + 0.75 * x) * beat;
            (whine * 0.24 * (0.12 + 0.88 * x.powf(1.4)) * beat + swell + air * 0.3 * x.powf(2.5)) * cut
        })
    });
}

/// The press that starts it: a relay closing, then a short tone up.
fn press(clip: &mut Clip, start: f32) {
    let (mut knock, mut tone) = (Phase::default(), Phase::default());
    clip.layer(start, 0.35, 0.3, |t| {
        let attack = 1.0 - (-t * 900.0).exp();
        let thud = (knock.step(85.0 + 130.0 * (-t * 26.0).exp()).sin() * 1.8).tanh() * (-t * 13.0).exp() * 0.6;
        let blip = tone.step(660.0 + 900.0 * (t / 0.12).min(1.0)).sin() * (-t * 14.0).exp() * 0.18;
        [(thud + blip) * attack; 2]
    });
}

/// A horn blast under an impact: a low fifth thick with harmonics, in fast and out slowly, its
/// top closing as it goes. The harmonics carry the note on speakers too small for its root.
fn blast(clip: &mut Clip, start: f32, seconds: f32, level: f32) {
    let notes = [55.0f32, 82.5, 110.0];
    let mut voices: Vec<[Phase; 2]> = notes.iter().map(|_| [Phase::default(), Phase::default()]).collect();
    let mut tops = [LowPass::default(), LowPass::default()];
    clip.layer(start, seconds, 0.4, |t| {
        let swell = (t / 0.03).min(1.0) * (-t * 3.4 / seconds).exp() * ((seconds - t) / 0.2).clamp(0.0, 1.0);
        let open = 260.0 + 1500.0 * (-t * 3.0).exp();
        std::array::from_fn(|ch| {
            let detune = 1.0 + 0.004 * (if ch == 0 { -1.0 } else { 1.0 });
            let chord: f32 = notes.iter().zip(voices.iter_mut()).map(|(hz, p)| buzz(p[ch].step(hz * detune), 10)).sum();
            tops[ch].run(chord, open) * swell * level
        })
    });
}

/// A jump or a drop out of warp: a boom that falls away under a burst of noise closing down.
fn impact(clip: &mut Clip, start: f32, depth: f32, seed: u32) {
    let mut boom = Phase::default();
    let (mut burst, mut closing) = ([Noise(seed), Noise(seed + 17)], [LowPass::default(), LowPass::default()]);
    clip.layer(start, 2.4, 0.45, |t| {
        let attack = 1.0 - (-t * 400.0).exp();
        let thump = (boom.step(40.0 * depth + 130.0 * depth * (-t * 7.0).exp()).sin() * 2.4).tanh() * (-t * 2.3).exp() * 0.85;
        std::array::from_fn(|ch| {
            let rush = closing[ch].run(burst[ch].next(), 9000.0 * (-t * 5.0).exp() + 140.0) * (-t * 2.8).exp() * 0.6;
            (thump + rush) * attack
        })
    });
}

/// The crack of the jump: a chirp down from the top of the range.
fn zap(clip: &mut Clip, start: f32) {
    let mut chirp = Phase::default();
    clip.layer(start, 0.6, 0.6, |t| {
        let v = chirp.step(250.0 + 3800.0 * (-t * 14.0).exp()).sin() * (0.6 + 0.4 * (TAU * 90.0 * t).sin()) * (-t * 9.0).exp() * 0.3;
        [v, v]
    });
}

/// The tunnel: a rumble, the engine throbbing on a low note, a chord over it that climbs three
/// semitones through the trip, a shimmer on top, wind, and stars streaking past from one ear to
/// the other.
fn tunnel(clip: &mut Clip, start: f32) {
    let mut brown = [0.0f32; 2];
    let (mut dust, mut deep, mut sub) = ([Noise(3), Noise(5)], [LowPass::default(), LowPass::default()], [LowPass::default(), LowPass::default()]);
    let (mut gust, mut wind) = ([Noise(7), Noise(13)], [BandPass::default(), BandPass::default()]);
    let chord = [1.0f32, 1.5, 2.0, 2.5, 3.0, 4.0];
    let mut partials: Vec<[Phase; 2]> = chord.iter().map(|_| [Phase::default(), Phase::default()]).collect();
    let shimmer = [(16.0f32, 3.1f32), (24.0, 4.3), (32.0, 5.7)];
    let mut glints: Vec<Phase> = shimmer.iter().map(|_| Phase::default()).collect();
    let (mut engine, mut octave, mut muffle) = ([Phase::default(), Phase::default()], [Phase::default(), Phase::default()], [LowPass::default(), LowPass::default()]);
    clip.layer(start, WARP + 0.12, 0.3, |t| {
        let x = t / WARP;
        let level = (t / 0.25).min(1.0) * ((WARP + 0.12 - t) / 0.12).clamp(0.0, 1.0);
        let base = 55.0 * 2f32.powf(x * 3.0 / 12.0);
        let glint: f32 = shimmer.iter().zip(glints.iter_mut()).map(|((ratio, rate), p)| p.step(base * ratio).sin() * (0.5 + 0.5 * (TAU * rate * t).sin()) * 0.03).sum();
        std::array::from_fn(|ch| {
            brown[ch] = (brown[ch] + dust[ch].next() * 0.02).clamp(-1.0, 1.0) * 0.999;
            let low = deep[ch].run(brown[ch], 140.0);
            let rumble = (low - sub[ch].run(low, 30.0)) * 2.0 * (1.0 + 0.25 * (TAU * 0.7 * t + ch as f32).sin());
            let pad: f32 = chord.iter().zip(partials.iter_mut()).enumerate().map(|(i, (ratio, p))| {
                let detune = 1.0 + 0.0025 * (if ch == 0 { -1.0 } else { 1.0 }) * (1.0 + i as f32 * 0.3);
                p[ch].step(base * ratio * detune).sin() / (i as f32 + 1.5)
            }).sum();
            let centre = 1200.0 + 900.0 * (TAU * 0.23 * t).sin() + 500.0 * (TAU * 0.61 * t + 1.0 + ch as f32).sin();
            let air = wind[ch].run(gust[ch].next(), centre, 1.2) * 0.1;
            // The engine: the chord's root as a buzz with its octave, throbbing, the top rolled off.
            let lean = 1.0 + 0.003 * (if ch == 0 { -1.0 } else { 1.0 });
            let drone = muffle[ch].run(buzz(engine[ch].step(base * lean), 9) + 0.6 * buzz(octave[ch].step(base * 2.0 / lean), 6), 480.0);
            let throb = 0.7 + 0.3 * (TAU * 5.5 * t + ch as f32 * 0.6).sin();
            (rumble * 0.5 + drone * 0.2 * throb + pad * 0.17 + glint * 1.3 + air * 1.2) * level
        })
    });
    // The streaks. Each is a short hiss falling in pitch as it crosses.
    let mut dice = Noise(101);
    let mut at = start + 0.2;
    while at < start + WARP - 0.3 {
        let side = if dice.next() > 0.0 { 1.0 } else { -1.0 };
        let (pitch, long) = (2600.0 + 1400.0 * dice.next(), 0.3 + 0.08 * dice.next());
        let (mut hiss, mut band) = (Noise(1000 + (at * 977.0) as u32), BandPass::default());
        clip.layer(at, long, 0.2, |t| {
            let u = t / long;
            let v = band.run(hiss.next(), pitch * (1.0 - 0.6 * u), 3.0) * (PI * u).sin().powi(2) * 0.2;
            pan(side * (2.0 * u - 1.0) * 0.9).map(|g| g * v)
        });
        at += 0.33 + 0.16 * (dice.next() + 1.0);
    }
}

/// Air pulled in ahead of the drop out of warp.
fn intake(clip: &mut Clip, start: f32, seconds: f32) {
    let (mut hiss, mut bands) = ([Noise(41), Noise(43)], [BandPass::default(), BandPass::default()]);
    clip.layer(start, seconds, 0.2, |t| {
        let u = t / seconds;
        std::array::from_fn(|ch| bands[ch].run(hiss[ch].next(), 500.0 * 2f32.powf(u * 3.6), 2.0) * u.powi(3) * 0.32)
    });
}

/// The drive winding down, and a low fifth under the approach.
fn wind_down(clip: &mut Clip, start: f32) {
    let (mut whine, mut flutter) = ([Phase::default(), Phase::default()], Phase::default());
    clip.layer(start, 2.6, 0.3, |t| {
        let beat = 1.0 - 0.3 * (0.5 + 0.5 * flutter.step(4.0 + 18.0 * (-t * 1.6).exp()).sin());
        let hz = 45.0 + 520.0 * (-t * 2.2).exp();
        std::array::from_fn(|ch| buzz(whine[ch].step(hz * (1.0 + 0.006 * ch as f32)), 4) * (-t * 1.9).exp() * 0.16 * beat)
    });
    let mut bed = [Phase::default(), Phase::default(), Phase::default(), Phase::default()];
    clip.layer(start, 4.6, 0.4, |t| {
        let level = (t / 0.4).min(1.0) * (-t * 0.75).exp() * 0.17;
        std::array::from_fn(|ch| (bed[ch * 2].step(55.0 + 0.2 * ch as f32).sin() + 0.7 * buzz(bed[ch * 2 + 1].step(82.5 - 0.15 * ch as f32), 3)) * level)
    });
}

/// One struck bell: a tone and two partials off the harmonic series that die sooner.
fn bell(clip: &mut Clip, start: f32, hz: f32, side: f32, level: f32) {
    let gains = pan(side);
    clip.layer(start, 2.4, 0.9, |t| {
        let attack = 1.0 - (-t * 900.0).exp();
        let v = (TAU * hz * t).sin() * (-t * 3.2).exp() + 0.25 * (TAU * hz * 2.76 * t).sin() * (-t * 7.0).exp() + 0.1 * (TAU * hz * 5.4 * t).sin() * (-t * 12.0).exp();
        gains.map(|g| g * v * level * attack)
    });
}

/// The note the chime lands on: its root two and three octaves down, swelling in under the bells.
fn landing(clip: &mut Clip, start: f32) {
    let (mut root, mut low) = ([Phase::default(), Phase::default()], [Phase::default(), Phase::default()]);
    clip.layer(start, 2.7, 0.5, |t| {
        let swell = (t / 0.12).min(1.0) * (-t * 1.5).exp() * ((2.7 - t) / 0.3).clamp(0.0, 1.0);
        std::array::from_fn(|ch| (buzz(root[ch].step(110.0 + 0.3 * ch as f32), 4) * 0.5 + low[ch].step(55.0).sin()) * swell * 0.3)
    });
}

fn trip() -> [Vec<f32>; 2] {
    let (jump, drop, settle) = (SPOOL, SPOOL + WARP, SPOOL + WARP + ARRIVE);
    let mut clip = Clip::new(settle + 2.8);
    press(&mut clip, 0.0);
    spool(&mut clip, 0.0);
    impact(&mut clip, jump, 1.0, 61);
    blast(&mut clip, jump, 1.5, 0.2);
    zap(&mut clip, jump);
    tunnel(&mut clip, jump);
    intake(&mut clip, drop - 0.55, 0.55);
    impact(&mut clip, drop, 0.85, 83);
    blast(&mut clip, drop, 2.2, 0.24);
    wind_down(&mut clip, drop);
    landing(&mut clip, settle);
    // A major, climbing: the ship is in orbit.
    for (i, hz) in [880.0, 1108.73, 1318.51, 1760.0].into_iter().enumerate() {
        bell(&mut clip, settle + 0.11 * i as f32, hz, if i % 2 == 0 { -0.5 } else { 0.5 }, 0.26);
    }
    // Short of full scale: the AAC encoder overshoots the peaks a little.
    clip.finish(0.8, 0.4)
}

/// The room tone in orbit: a low fifth and air that swell and fall back to nothing over the
/// clip, so it can loop however long the gap between its plays.
fn ambient() -> [Vec<f32>; 2] {
    let seconds = 12.0;
    let mut clip = Clip::new(seconds);
    // Every partial fits a whole number of cycles in the clip.
    let partials = [(55.0f32, 1.0f32), (82.5, 0.85), (110.0, 0.8), (165.0, 0.4), (220.0, 0.2)];
    let (mut dust, mut soft) = ([Noise(71), Noise(73)], [LowPass::default(), LowPass::default()]);
    clip.layer(0.0, seconds, 0.5, |t| {
        let breath = (0.5 - 0.5 * (TAU * t / seconds).cos()).powf(0.8);
        std::array::from_fn(|ch| {
            let beat = (1.0 + ch as f32) / seconds;
            let tone: f32 = partials.iter().enumerate().map(|(i, (hz, gain))| {
                let slow = 0.75 + 0.25 * (TAU * (i as f32 + 1.0) * t / seconds + ch as f32).sin();
                ((TAU * hz * t).sin() + (TAU * (hz + beat * (i as f32 + 1.0)) * t).sin()) * gain * slow
            }).sum();
            ((tone * 0.22).tanh() * 0.5 + soft[ch].run(dust[ch].next(), 500.0) * 0.1) * breath
        })
    });
    clip.finish(0.6, 0.0)
}

/// A short run of plucked notes for the interface, `(delay, hz)` each, over a low knock.
fn blips(notes: &[(f32, f32)], seconds: f32) -> [Vec<f32>; 2] {
    let mut clip = Clip::new(seconds);
    clip.layer(0.0, seconds, 0.2, |t| {
        let v = (TAU * (120.0 + 80.0 * (-t * 40.0).exp()) * t).sin() * (-t * 34.0).exp() * (1.0 - (-t * 1500.0).exp()) * 0.9;
        [v, v]
    });
    for &(delay, hz) in notes {
        clip.layer(delay, seconds - delay, 0.25, |t| {
            let attack = 1.0 - (-t * 1500.0).exp();
            let v = ((TAU * hz * t).sin() + 0.35 * (TAU * hz * 1.5 * t).sin()) * (-t * 38.0).exp() * attack;
            [v, v]
        });
    }
    clip.finish(0.55, 0.02)
}

/// 16-bit stereo PCM.
fn write(path: &Path, [left, right]: [Vec<f32>; 2]) -> std::io::Result<()> {
    let bytes = (left.len() * 4) as u32;
    let mut out = Vec::with_capacity(44 + bytes as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + bytes).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&(RATE as u32).to_le_bytes());
    out.extend_from_slice(&(RATE as u32 * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&bytes.to_le_bytes());
    for (l, r) in left.iter().zip(&right) {
        for v in [l, r] {
            out.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
    }
    fs::File::create(path)?.write_all(&out)
}

fn main() -> std::io::Result<()> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "assets/sounds".into());
    let dir = Path::new(&dir);
    fs::create_dir_all(dir)?;
    write(&dir.join("trip.wav"), trip())?;
    write(&dir.join("ambient.wav"), ambient())?;
    write(&dir.join("select.wav"), blips(&[(0.0, 1320.0)], 0.16))?;
    write(&dir.join("on.wav"), blips(&[(0.0, 880.0), (0.07, 1320.0)], 0.26))?;
    write(&dir.join("off.wav"), blips(&[(0.0, 1320.0), (0.07, 880.0)], 0.26))?;
    println!("wrote trip, ambient, select, on and off to {}", dir.display());
    Ok(())
}
