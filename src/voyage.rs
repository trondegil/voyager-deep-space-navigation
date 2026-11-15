//! The trip: where the ship is, the drive's phases, and every number the view and the compositor
//! read from them. Each one is a pure function of the state and its clock, so a headless run at
//! a given time always draws the same frame.

use crate::sound::{Cue, Sounds};
use crate::worlds::{WORLDS, World};

/// Seconds the drive charges before the jump.
pub const SPOOL: f64 = 1.6;
/// Seconds in the tunnel.
pub const WARP: f64 = 4.6;
/// Seconds from dropping out of warp to a settled orbit.
pub const ARRIVE: f64 = 2.4;
/// Seconds the autopilot waits in orbit before the next jump.
const LAYOVER: f64 = 5.0;
/// Seconds the arrival banner stays up.
const BANNER: f64 = 3.6;

/// Where the planet rests in the window, as fractions of its width and height.
pub const HOME: [f32; 2] = [0.635, 0.455];

/// Seconds light takes to cross one astronomical unit.
const LIGHT_SECONDS_PER_AU: f64 = 499.005;

/// Earth, where the ship starts.
pub const START: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Parked around `State::here` since this time.
    Orbit { since: f64 },
    /// The drive is charging.
    Spool { t0: f64 },
    /// In the tunnel.
    Warp { t0: f64 },
    /// Out of the tunnel at `State::here`, closing on it.
    Arrive { t0: f64 },
}

pub struct State {
    /// The clock (seconds), as of the last `poll`.
    pub now: f64,
    pub phase: Phase,
    /// The planet the ship orbits, or the one it left while it is in the tunnel.
    pub here: usize,
    /// Where the current (or last) trip started.
    pub origin: usize,
    /// Where the current (or last) trip ends.
    pub bound: usize,
    /// The row picked in the destination list.
    pub selected: usize,
    pub autopilot: bool,
    /// When the last jump or drop-out sent a shockwave through the window.
    pub shock: f64,
    /// When the ship last settled into orbit after a trip.
    pub arrived: Option<f64>,
    /// Astronomical units flown so far.
    pub flown: f64,
    /// The clips to play, in a window. Headless runs and tests have none.
    pub sound: Option<Sounds>,
    pub muted: bool,
    /// Draw only the planet, centred, for the app icon.
    pub poster: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            now: 0.0,
            phase: Phase::Orbit { since: 0.0 },
            here: START,
            origin: START,
            bound: START,
            selected: START + 1,
            autopilot: false,
            shock: -10.0,
            arrived: None,
            flown: 0.0,
            sound: None,
            muted: false,
            poster: false,
        }
    }
}

/// What the drive is doing to the picture.
#[derive(Clone, Copy, Debug, Default)]
pub struct Drive {
    /// 0 at rest, 1 in the tunnel.
    pub warp: f32,
    /// A white-out, 0..1.
    pub flash: f32,
    /// Camera shake, 0..1.
    pub shake: f32,
    /// How far the streaks have run, in their own cells.
    pub travel: f32,
    /// The capacitor, 0..1: it fills while spooling and drains in the tunnel.
    pub charge: f32,
}

/// The planet as drawn: its centre in fractions of the window, its radius in fractions of the
/// window's height, and how much of it is there.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub at: [f32; 2],
    pub radius: f32,
    pub seen: f32,
}

fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Warp at `tau` seconds into a trip: a crawl while spooling, a kick to full at the jump, a
/// hard stop at the far end.
fn warp_at(tau: f64) -> f64 {
    if tau <= 0.0 {
        0.0
    } else if tau < SPOOL {
        0.14 * (tau / SPOOL).powi(2)
    } else if tau < SPOOL + WARP {
        let x = ((tau - SPOOL) / 0.5).min(1.0);
        0.14 + 0.86 * (1.0 - (1.0 - x).powi(3))
    } else {
        (-(tau - SPOOL - WARP) * 7.0).exp()
    }
}

impl State {
    pub fn world(&self) -> &'static World {
        &WORLDS[self.here]
    }

    pub fn in_orbit(&self) -> bool {
        matches!(self.phase, Phase::Orbit { .. })
    }

    /// Seconds since the drive started charging, in flight.
    pub fn trip_time(&self) -> Option<f64> {
        match self.phase {
            Phase::Orbit { .. } => None,
            Phase::Spool { t0 } => Some(self.now - t0),
            Phase::Warp { t0 } => Some(SPOOL + self.now - t0),
            Phase::Arrive { t0 } => Some(SPOOL + WARP + self.now - t0),
        }
    }

    fn cue(&self, cue: Cue) {
        if let Some(sound) = &self.sound {
            sound.cue(cue);
        }
    }

    /// Whether the selected planet is somewhere to go.
    pub fn can_engage(&self) -> bool {
        self.in_orbit() && self.selected != self.here
    }

    pub fn select(&mut self, i: usize) {
        if self.in_orbit() && i < WORLDS.len() && i != self.selected {
            self.selected = i;
            self.cue(Cue::Select);
        }
    }

    pub fn engage(&mut self) {
        if self.can_engage() {
            self.depart(self.now);
        }
    }

    /// Starts the trip to the selected planet at `t0`.
    fn depart(&mut self, t0: f64) {
        self.origin = self.here;
        self.bound = self.selected;
        self.arrived = None;
        self.phase = Phase::Spool { t0 };
        self.cue(Cue::Engage);
    }

    /// Switching the autopilot on in orbit starts its countdown short, so the tour begins soon.
    pub fn toggle_autopilot(&mut self) {
        self.autopilot = !self.autopilot;
        if self.autopilot && self.in_orbit() {
            self.phase = Phase::Orbit { since: self.now - (LAYOVER - 1.5) };
        }
        self.cue(if self.autopilot { Cue::SwitchOn } else { Cue::SwitchOff });
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        if let Some(sound) = &self.sound {
            sound.set_muted(self.muted);
        }
        self.cue(Cue::Select);
    }

    /// Advances the clock, and the phase through every boundary it passed.
    pub fn tick(&mut self, now: f64) {
        self.now = now;
        loop {
            match self.phase {
                Phase::Spool { t0 } if now >= t0 + SPOOL => {
                    self.shock = t0 + SPOOL;
                    self.phase = Phase::Warp { t0: t0 + SPOOL };
                }
                Phase::Warp { t0 } if now >= t0 + WARP => {
                    self.flown += f64::from(self.trip_length());
                    self.here = self.bound;
                    self.shock = t0 + WARP;
                    self.phase = Phase::Arrive { t0: t0 + WARP };
                }
                Phase::Arrive { t0 } if now >= t0 + ARRIVE => {
                    self.arrived = Some(t0 + ARRIVE);
                    // Line up the next planet, so one more press flies on.
                    self.selected = (self.here + 1) % WORLDS.len();
                    self.phase = Phase::Orbit { since: t0 + ARRIVE };
                    self.cue(Cue::Arrive);
                }
                Phase::Orbit { since } if self.autopilot && now >= since + LAYOVER => {
                    self.selected = (self.here + 1) % WORLDS.len();
                    self.depart(since + LAYOVER);
                }
                _ => break,
            }
        }
    }

    /// The two ends of the route on show: the trip under way, or in orbit the one selected.
    pub fn route(&self) -> (usize, usize) {
        if self.in_orbit() { (self.here, self.selected) } else { (self.origin, self.bound) }
    }

    /// Astronomical units between the route's two ends.
    pub fn trip_length(&self) -> f32 {
        let (a, b) = self.route();
        WORLDS[a].distance_to(&WORLDS[b])
    }

    pub fn drive(&self) -> Drive {
        let Some(tau) = self.trip_time() else { return Drive::default() };
        let warp = warp_at(tau);
        let (flash, shake, charge) = match self.phase {
            Phase::Orbit { .. } => (0.0, 0.0, 0.0),
            Phase::Spool { .. } => {
                let x = tau / SPOOL;
                (0.2 * x.powi(5), 0.75 * x * x, smooth(x))
            }
            Phase::Warp { .. } => {
                let x = tau - SPOOL;
                (0.95 * (-x * 4.2).exp(), 0.3 + 0.7 * (-x * 2.5).exp(), 1.0 - 0.75 * smooth(x / WARP))
            }
            Phase::Arrive { .. } => {
                let x = tau - SPOOL - WARP;
                (0.9 * (-x * 4.5).exp(), 0.65 * (-x * 3.0).exp(), 0.25 * (-x * 1.5).exp())
            }
        };
        // The streaks' speed follows warp, so their distance is its integral over the trip.
        let step: f64 = 1.0 / 120.0;
        let mut travel = 0.0;
        let mut x: f64 = 0.0;
        while x < tau {
            let dt = step.min(tau - x);
            travel += (0.25 + 9.0 * warp_at(x + dt * 0.5).powf(1.5)) * dt;
            x += dt;
        }
        Drive { warp: warp as f32, flash: flash as f32, shake: shake as f32, travel: travel as f32, charge: charge as f32 }
    }

    /// Share of the route behind the ship, 0..1.
    pub fn progress(&self) -> f32 {
        match self.phase {
            Phase::Orbit { .. } | Phase::Spool { .. } => 0.0,
            Phase::Warp { t0 } => smooth((self.now - t0) / WARP) as f32,
            Phase::Arrive { .. } => 1.0,
        }
    }

    /// Astronomical units still to go.
    pub fn remaining(&self) -> f32 {
        match self.phase {
            Phase::Arrive { .. } => 0.0,
            _ => self.trip_length() * (1.0 - self.progress()),
        }
    }

    /// Speed as text and its unit: kilometres a second in orbit, multiples of light in flight.
    pub fn velocity(&self) -> (String, &'static str) {
        match self.phase {
            Phase::Orbit { .. } => (grouped(self.world().orbit_kms, 2), "km/s"),
            Phase::Spool { t0 } => (format!("{:.3}", 0.92 * smooth((self.now - t0) / SPOOL).powi(2)), "c"),
            Phase::Warp { t0 } => {
                let x = ((self.now - t0) / WARP).clamp(0.0, 1.0);
                // The derivative of `progress`, in astronomical units a second, against light's.
                let c = (f64::from(self.trip_length()) * 6.0 * x * (1.0 - x) / WARP * LIGHT_SECONDS_PER_AU).max(1.0);
                (grouped(c as f32, if c < 100.0 { 1 } else { 0 }), "c")
            }
            Phase::Arrive { t0 } => {
                let c = 0.92 * (-(self.now - t0) * 3.4).exp();
                let orbit = f64::from(self.world().orbit_kms);
                if c > 0.01 { (format!("{c:.3}"), "c") } else { (grouped((c * 299_792.458).max(orbit) as f32, 2), "km/s") }
            }
        }
    }

    /// One word for what the drive is doing.
    pub fn status(&self) -> &'static str {
        match self.phase {
            Phase::Orbit { .. } => "IN ORBIT",
            Phase::Spool { .. } => "SPOOLING",
            Phase::Warp { .. } => "FTL TRANSIT",
            Phase::Arrive { .. } => "ARRIVING",
        }
    }

    /// The planet's place on screen: at rest in orbit, swept past as the ship leaves, and
    /// growing out of the middle of the window as it closes on the next one.
    pub fn pose(&self) -> Pose {
        let radius = self.world().radius;
        if self.poster {
            return Pose { at: [0.5, 0.53], radius: 0.17, seen: 1.0 };
        }
        let Some(tau) = self.trip_time() else { return Pose { at: HOME, radius, seen: 1.0 } };
        let leaving = SPOOL + 0.4;
        if tau < leaving {
            let k = ((tau / leaving) as f32).powi(3);
            let at = [HOME[0] + (HOME[0] - 0.5) * 4.0 * k, HOME[1] + (HOME[1] - 0.5) * 4.0 * k];
            Pose { at, radius: radius * (1.0 + 3.2 * k), seen: 1.0 - smooth((tau - SPOOL) / 0.35) as f32 }
        } else if tau < SPOOL + WARP {
            Pose { at: HOME, radius: 0.0, seen: 0.0 }
        } else {
            let x = (((tau - SPOOL - WARP) / ARRIVE) as f32).clamp(0.0, 1.0);
            let e = 1.0 - (1.0 - x).powi(4);
            let at = [0.5 + (HOME[0] - 0.5) * e, 0.5 + (HOME[1] - 0.5) * e];
            Pose { at, radius: radius * (0.02 + 0.98 * e), seen: smooth(f64::from(x) / 0.1) as f32 }
        }
    }

    /// Whether the arrival banner is up.
    pub fn banner(&self) -> bool {
        self.arrived.is_some_and(|t| self.in_orbit() && self.now < t + BANNER)
    }
}

/// `1234.5` as `1,234.50`.
pub fn grouped(v: f32, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    let (whole, frac) = s.split_once('.').map_or((s.as_str(), ""), |(w, f)| (w, f));
    let mut out = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trip_runs_through_every_phase_and_lands_on_the_selected_planet() {
        let mut s = State::default();
        s.tick(1.0);
        s.select(5);
        s.engage();
        assert_eq!(s.phase, Phase::Spool { t0: 1.0 });
        s.tick(1.0 + SPOOL + 0.1);
        assert!(matches!(s.phase, Phase::Warp { .. }));
        assert_eq!(s.here, START, "still counted from the planet it left");
        assert!(s.drive().warp > 0.5);
        s.tick(1.0 + SPOOL + WARP + 0.1);
        assert!(matches!(s.phase, Phase::Arrive { .. }));
        assert_eq!(s.here, 5);
        // One big step crosses the rest.
        s.tick(60.0);
        assert!(s.in_orbit());
        assert_eq!(s.drive().warp, 0.0);
        assert!((s.flown - 8.58).abs() < 0.01, "Earth to Saturn is 8.58 AU, flew {}", s.flown);
        assert_eq!(s.selected, 6, "the next planet out is lined up");
    }

    #[test]
    fn engaging_needs_somewhere_else_to_go() {
        let mut s = State::default();
        s.select(START);
        s.engage();
        assert!(s.in_orbit());
    }

    #[test]
    fn the_planet_rests_at_home_and_is_gone_in_the_tunnel() {
        let mut s = State::default();
        assert_eq!(s.pose().at, HOME);
        s.engage();
        s.tick(SPOOL + WARP * 0.5);
        assert_eq!(s.pose().seen, 0.0);
        s.tick(SPOOL + WARP + ARRIVE - 0.001);
        assert!((s.pose().radius - s.world().radius).abs() < 0.001);
    }

    #[test]
    fn warp_speed_is_counted_in_multiples_of_light() {
        let mut s = State::default();
        s.select(7);
        s.engage();
        s.tick(SPOOL + WARP * 0.5);
        // 29.07 AU in 4.6 s peaks at 1.5 times the mean: 9.48 AU/s, where light does 0.002.
        assert_eq!(s.velocity(), ("4,730".into(), "c"));
    }

    #[test]
    fn numbers_group_in_thousands() {
        assert_eq!(grouped(7198.416, 2), "7,198.42");
        assert_eq!(grouped(4.24, 2), "4.24");
        assert_eq!(grouped(123456.0, 0), "123,456");
    }
}
