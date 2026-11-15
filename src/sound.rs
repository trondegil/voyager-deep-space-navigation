//! Sound. Five clips, synthesized by `examples/make_sounds.rs`, ride along inside the binary and
//! play through Rune's `Player`. A player opens a file, so they are written out to the temp
//! folder once at launch.

use rune::Player;

/// Something that happened and should be heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    /// The selection moved.
    Select,
    SwitchOn,
    SwitchOff,
    /// The drive started charging: the whole trip plays from here.
    Engage,
    /// The ship settled into orbit.
    Arrive,
}

const CLIPS: [(&str, &[u8]); 5] = [
    ("select.wav", include_bytes!("../assets/sounds/select.wav")),
    ("on.wav", include_bytes!("../assets/sounds/on.wav")),
    ("off.wav", include_bytes!("../assets/sounds/off.wav")),
    ("trip.m4a", include_bytes!("../assets/sounds/trip.m4a")),
    ("ambient.m4a", include_bytes!("../assets/sounds/ambient.m4a")),
];

pub struct Sounds {
    select: Player,
    on: Player,
    off: Player,
    /// Spool, jump, tunnel, drop-out and arrival chime as one clip, in step with `voyage.rs`.
    trip: Player,
    /// The room tone in orbit, looping.
    ambient: Player,
}

impl Sounds {
    /// Writes the clips out and opens them. `None` when the temp folder can't be written, and
    /// the app then runs silent.
    pub fn load(muted: bool) -> Option<Sounds> {
        let dir = std::env::temp_dir().join("voyager-sounds");
        std::fs::create_dir_all(&dir).ok()?;
        for (name, bytes) in CLIPS {
            let path = dir.join(name);
            if std::fs::read(&path).ok().as_deref() != Some(bytes) {
                std::fs::write(&path, bytes).ok()?;
            }
        }
        let open = |name: &str, volume: f32| {
            let clip = Player::audio(dir.join(name));
            clip.set_volume(volume);
            clip.preload();
            clip
        };
        let sounds = Sounds { select: open("select.wav", 0.45), on: open("on.wav", 0.5), off: open("off.wav", 0.5), trip: open("trip.m4a", 0.9), ambient: open("ambient.m4a", 0.55) };
        sounds.set_muted(muted);
        sounds.ambient.set_looping(true);
        sounds.ambient.play();
        Some(sounds)
    }

    pub fn cue(&self, cue: Cue) {
        match cue {
            Cue::Select => again(&self.select),
            Cue::SwitchOn => again(&self.on),
            Cue::SwitchOff => again(&self.off),
            Cue::Engage => {
                self.ambient.pause();
                again(&self.trip);
            }
            // The trip clip is still ringing its chime; the room tone swells back in under it.
            Cue::Arrive => again(&self.ambient),
        }
    }

    pub fn set_muted(&self, muted: bool) {
        for clip in [&self.select, &self.on, &self.off, &self.trip, &self.ambient] {
            clip.set_muted(muted);
        }
    }

    /// What each clip is doing, for a log line.
    pub fn report(&self) -> String {
        [("select", &self.select), ("on", &self.on), ("off", &self.off), ("trip", &self.trip), ("ambient", &self.ambient)]
            .map(|(name, clip)| format!("{name} {:?}", clip.state()))
            .join(", ")
    }
}

/// Plays a clip from its start, whether or not it was still playing.
fn again(clip: &Player) {
    clip.seek(0.0);
    clip.play();
}
