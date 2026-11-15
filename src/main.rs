//! Voyager: a space travel demo for Rune (github.com/anomalyco/rune). Pick a planet, engage the
//! drive, and the window jumps to warp.
//!
//!     cargo run --release
//!     cargo run --release -- --scene jump --snap warp.png --at 4      # headless still
//!     cargo run --release -- --list-scenes

mod gfx;
mod hud;
mod sound;
mod voyage;
mod worlds;

use std::sync::atomic::{AtomicBool, Ordering};

use rune::{App, AppSpec, Key, KeyEvent, Tx};

use sound::Sounds;
use voyage::State;
use worlds::WORLDS;

/// The state a window starts with: the default, plus sound. A headless run stays silent, and
/// `VOYAGER_MUTE=1` starts a window muted.
fn init() -> State {
    let muted = std::env::var_os("VOYAGER_MUTE").is_some();
    let sound = if rune::tooling::headless_run().is_some() { None } else { Sounds::load(muted) };
    State { sound, muted, ..State::default() }
}

/// Every frame: the clock moves the trip on, and the instruments follow it.
fn poll(s: &mut State, t: f64, _: &mut Tx) -> bool {
    s.tick(t);
    // Once the clips have had time to open, say so if one didn't (or always, on request).
    static CHECKED: AtomicBool = AtomicBool::new(false);
    if t > 2.5 && !CHECKED.swap(true, Ordering::Relaxed) && let Some(sound) = &s.sound {
        let report = sound.report();
        if report.contains("Failed") || std::env::var_os("VOYAGER_SOUND_LOG").is_some() {
            eprintln!("voyager: sound: {report}");
        }
    }
    true
}

/// Up and Down pick a planet (1-8 too), Return or Space engages, A toggles the autopilot and M
/// the sound.
fn on_key(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    let n = WORLDS.len();
    match &ev.key {
        Key::Up => s.select((s.selected + n - 1) % n),
        Key::Down => s.select((s.selected + 1) % n),
        Key::Enter => s.engage(),
        Key::Text(typed) => match typed.as_str() {
            " " => s.engage(),
            "a" | "A" => s.toggle_autopilot(),
            "m" | "M" => s.toggle_mute(),
            other => match other.parse::<usize>() {
                Ok(i) if (1..=n).contains(&i) => s.select(i - 1),
                _ => return false,
            },
        },
        _ => return false,
    }
    true
}

/// Parks the ship at planet `i` with the next one out selected.
fn park(s: &mut State, i: usize) {
    s.here = i;
    s.selected = (i + 1) % WORLDS.len();
}

/// The Dock's icon on macOS and the taskbar's on Windows, when there is no bundle to give one.
fn show_icon() {
    if let Err(why) = rune::set_app_icon(include_bytes!("../assets/icon.png").as_slice()) {
        eprintln!("voyager: {why}");
    }
}

fn main() {
    // Windows reads the icon when it makes a window, so it has to be there before `run`.
    show_icon();
    App::new(AppSpec::<State> {
        min_size: Some((1080.0, 680.0)),
        // Starting points for headless runs: each planet (Earth is the default), and a trip
        // under way from t = 0 (spooling until 1.6 s, in the tunnel until 6.2 s, settled by 8.6 s).
        scenes: &[
            ("mercury", |s| park(s, 0)),
            ("venus", |s| park(s, 1)),
            ("mars", |s| park(s, 3)),
            ("jupiter", |s| park(s, 4)),
            ("saturn", |s| park(s, 5)),
            ("uranus", |s| park(s, 6)),
            ("neptune", |s| park(s, 7)),
            ("jump", |s| {
                s.selected = 5;
                s.engage();
            }),
            ("autopilot", |s| s.toggle_autopilot()),
            // Saturn alone, for `scripts/make-icon.sh`.
            ("icon", |s| {
                s.here = 5;
                s.poster = true;
            }),
        ],
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("Voyager", (1200.0, 750.0), init, hud::view)
    })
    .compositor(gfx::Gfx::default())
    // And again once the window is up: an icon set before launch reaches AppKit, but the Dock
    // of a bare binary goes on showing the generic one.
    .on_ready(|_| show_icon())
    .run();
}
