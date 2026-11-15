//! The chart: the eight planets. Their figures are the textbook ones, rounded. Their looks are
//! drawn from noise, not from maps, and their sizes on screen are not to scale.

/// A globe with seas, land, ice and weather.
pub const ROCKY: f32 = 0.0;
/// A globe of cloud bands, with or without rings.
pub const BANDED: f32 = 1.0;

/// The Sun's colour.
pub const SUNLIGHT: u32 = 0xfff1d0;

pub struct World {
    pub name: &'static str,
    pub class: &'static str,
    /// Mean distance from the Sun in astronomical units.
    pub orbit_au: f32,
    /// Speed of a low parking orbit.
    pub orbit_kms: f32,
    pub facts: [(&'static str, &'static str); 3],
    /// The HUD's colour while the ship is here.
    pub accent: u32,

    /// `ROCKY` or `BANDED`.
    pub kind: f32,
    /// Radius on screen, in window heights.
    pub radius: f32,
    /// Low, mid and high colours: seas, lowlands and highlands, or a giant's bands.
    pub colors: [u32; 3],
    pub atmosphere: (u32, f32),
    /// Rocky: the share under the sea, cloud cover, city lights at night and ice caps. Banded:
    /// the axis's roll on screen, how far its pole leans at the viewer (0..1), a storm (above 0
    /// a red oval, below 0 a dark one under white cirrus) and rings (above 0 broad, below 0 one
    /// narrow ring).
    pub surface: [f32; 4],
    /// How cratered a rocky world is.
    pub craters: f32,
    /// How far haze smooths a banded world's belts away.
    pub haze: f32,
    pub seed: f32,
    /// Which face it shows at the start, in radians of spin.
    pub spin: f32,
    /// Two colours for the sky behind it, and their strength.
    pub nebula: (u32, u32, f32),
    /// Where the Sun hangs, in window heights from the centre (x right, y down), and its radius.
    pub sun: ([f32; 2], f32),
}

impl World {
    /// Astronomical units to `other` when the two line up on the same side of the Sun.
    pub fn distance_to(&self, other: &World) -> f32 {
        (self.orbit_au - other.orbit_au).abs()
    }
}

pub const WORLDS: [World; 8] = [
    World {
        name: "Mercury",
        class: "Rocky planet",
        orbit_au: 0.39,
        orbit_kms: 3.0,
        facts: [("GRAVITY", "0.38 g"), ("MEAN TEMP", "440 K"), ("YEAR", "88 d")],
        accent: 0xd9c8a9,
        kind: ROCKY,
        radius: 0.118,
        colors: [0x4a4642, 0x8a8178, 0xc4b8a8],
        atmosphere: (0xffe0b0, 0.0),
        surface: [0.3, 0.0, 0.0, 0.0],
        craters: 1.0,
        haze: 0.0,
        seed: 13.0,
        spin: 0.8,
        nebula: (0x7a4a1a, 0x4a2a4a, 0.5),
        sun: ([-0.22, -0.21], 0.05),
    },
    World {
        name: "Venus",
        class: "Rocky planet",
        orbit_au: 0.72,
        orbit_kms: 7.3,
        facts: [("GRAVITY", "0.90 g"), ("MEAN TEMP", "737 K"), ("YEAR", "225 d")],
        accent: 0xffd27a,
        kind: BANDED,
        radius: 0.162,
        colors: [0xe6c888, 0xf4e6b8, 0xdcb672],
        atmosphere: (0xffe2a0, 1.0),
        surface: [0.12, 0.1, 0.0, 0.0],
        craters: 0.0,
        haze: 0.0,
        seed: 27.0,
        spin: 2.0,
        nebula: (0x8a5a1a, 0x6a2a3a, 0.5),
        sun: ([-0.16, -0.3], 0.036),
    },
    World {
        name: "Earth",
        class: "Rocky planet",
        orbit_au: 1.0,
        orbit_kms: 7.66,
        facts: [("GRAVITY", "1.00 g"), ("MEAN TEMP", "288 K"), ("YEAR", "365.3 d")],
        accent: 0x6cc5ff,
        kind: ROCKY,
        radius: 0.172,
        colors: [0x1c4f9e, 0x3d7a3a, 0xa8926a],
        atmosphere: (0x5aa8ff, 1.0),
        surface: [0.535, 0.9, 1.1, 1.0],
        craters: 0.0,
        haze: 0.0,
        seed: 3.0,
        spin: 1.11,
        nebula: (0x1b3a8a, 0x5a2a8a, 0.6),
        sun: ([-0.2, -0.335], 0.024),
    },
    World {
        name: "Mars",
        class: "Rocky planet",
        orbit_au: 1.52,
        orbit_kms: 3.56,
        facts: [("GRAVITY", "0.38 g"), ("MEAN TEMP", "210 K"), ("YEAR", "687 d")],
        accent: 0xff8f66,
        kind: ROCKY,
        radius: 0.138,
        colors: [0x55231a, 0xa64b2a, 0xe2a274],
        atmosphere: (0xff9a6c, 0.45),
        surface: [0.4, 0.1, 0.0, 0.8],
        craters: 0.35,
        haze: 0.0,
        seed: 21.0,
        spin: 1.49,
        nebula: (0x7a1f3a, 0x3a1f6a, 0.55),
        sun: ([-0.13, -0.3], 0.017),
    },
    World {
        name: "Jupiter",
        class: "Gas giant",
        orbit_au: 5.2,
        orbit_kms: 42.1,
        facts: [("GRAVITY", "2.53 g"), ("CLOUD TOPS", "165 K"), ("YEAR", "11.9 y")],
        accent: 0xffb98a,
        kind: BANDED,
        radius: 0.196,
        colors: [0xb07a4e, 0xf0dcc0, 0x8a4a2e],
        atmosphere: (0xffd9b0, 0.6),
        surface: [-0.06, 0.06, 1.0, 0.0],
        craters: 0.0,
        haze: 0.0,
        seed: 5.0,
        spin: 0.45,
        nebula: (0x5a3a1a, 0x1f4a5a, 0.5),
        sun: ([-0.25, -0.32], 0.009),
    },
    World {
        name: "Saturn",
        class: "Gas giant",
        orbit_au: 9.58,
        orbit_kms: 25.1,
        facts: [("GRAVITY", "1.06 g"), ("CLOUD TOPS", "134 K"), ("YEAR", "29.5 y")],
        accent: 0xffd98f,
        kind: BANDED,
        radius: 0.132,
        colors: [0xc49a62, 0xf2dfb4, 0x8a5a3a],
        atmosphere: (0xffd9a0, 0.7),
        surface: [-0.36, 0.3, 0.0, 1.0],
        craters: 0.0,
        haze: 0.0,
        seed: 8.0,
        spin: 2.96,
        nebula: (0x6a4a1a, 0x4a2a7a, 0.55),
        sun: ([-0.24, -0.3], 0.007),
    },
    World {
        name: "Uranus",
        class: "Ice giant",
        orbit_au: 19.19,
        orbit_kms: 15.1,
        facts: [("GRAVITY", "0.89 g"), ("CLOUD TOPS", "76 K"), ("YEAR", "84.0 y")],
        accent: 0x8ef0ea,
        kind: BANDED,
        radius: 0.165,
        colors: [0xaee4ea, 0xbdeef0, 0xa2dce4],
        atmosphere: (0xa8f4ff, 0.9),
        surface: [-1.3, 0.3, 0.0, -0.8],
        craters: 0.0,
        haze: 0.9,
        seed: 33.0,
        spin: 1.0,
        nebula: (0x0f6a6a, 0x1f3a7a, 0.55),
        sun: ([-0.12, -0.34], 0.0055),
    },
    World {
        name: "Neptune",
        class: "Ice giant",
        orbit_au: 30.07,
        orbit_kms: 16.6,
        facts: [("GRAVITY", "1.14 g"), ("CLOUD TOPS", "72 K"), ("YEAR", "164.8 y")],
        accent: 0x7a9cff,
        kind: BANDED,
        radius: 0.16,
        colors: [0x2a52d6, 0x4a7cf0, 0x1c38a6],
        atmosphere: (0x6a9cff, 1.0),
        surface: [-0.42, 0.18, -1.0, 0.0],
        craters: 0.0,
        haze: 0.0,
        seed: 44.0,
        spin: 0.5,
        nebula: (0x1a2a8a, 0x4a1f7a, 0.6),
        sun: ([-0.21, -0.33], 0.0045),
    },
];
