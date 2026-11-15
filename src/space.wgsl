// Deep space behind the HUD, drawn in one pass over a single triangle: a nebula and drifting
// stars, the Sun, one lit planet (rocky, or banded with a storm and rings), and the warp tunnel
// the stars stretch into. Colours are authored display-referred (as a shadertoy is) and decoded
// at the end when the target wants linear light.

struct U {
    view: vec4f,    // window w, h in device px; time in s; device px per point
    rect: vec4f,    // the slot in window px: x, y, w, h
    drive: vec4f,   // warp 0..1, flash, shake, travel (streak cells)
    body: vec4f,    // centre x, y in window px; radius px; kind (0 rocky, 1 banded)
    surf: vec4f,    // rocky: sea level, clouds, city lights, ice caps
                    // banded: axis roll, axis tip, storm (+ red, - dark), rings (+ broad, - narrow)
    c0: vec4f,      // low colour (seas, dark bands); a = surface seed
    c1: vec4f,      // mid colour; a = spin phase
    c2: vec4f,      // high colour; a = body visibility
    atmo: vec4f,    // atmosphere colour; a = strength
    neb_a: vec4f,   // nebula colour; a = seed
    neb_b: vec4f,   // nebula colour; a = strength
    sun: vec4f,     // position in view units; size (0 = none)
    sun_col: vec4f, // sun colour; a = 1 for sRGB-encoded output, 0 for linear
    more: vec4f,    // craters (rocky); lens flare ghosts 0..1; haze (banded)
}

@group(0) @binding(0) var<uniform> u: U;

struct VOut {
    @builtin(position) pos: vec4f,
    @location(0) px: vec2f,
}

// The viewport is the whole window (shifted by the pass origin inside a layer), so this is the
// fragment's window position whatever target the slot draws into.
@vertex
fn vs(@builtin(vertex_index) i: u32) -> VOut {
    let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VOut;
    o.pos = vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
    o.px = vec2f(uv.x, 1.0 - uv.y) * u.view.xy;
    return o;
}

const TAU: f32 = 6.28318530718;

// ---------------------------------------------------------------- noise

fn hash12(p: vec2f) -> f32 {
    var p3 = fract(vec3f(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash22(p: vec2f) -> vec2f {
    var p3 = fract(vec3f(p.x, p.y, p.x) * vec3f(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

fn hash13(p: vec3f) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash33(p: vec3f) -> vec3f {
    var p3 = fract(p * vec3f(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

fn noise2(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash12(i), hash12(i + vec2f(1.0, 0.0)), w.x),
        mix(hash12(i + vec2f(0.0, 1.0)), hash12(i + vec2f(1.0, 1.0)), w.x),
        w.y,
    );
}

fn noise3(p: vec3f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    let a = mix(hash13(i), hash13(i + vec3f(1.0, 0.0, 0.0)), w.x);
    let b = mix(hash13(i + vec3f(0.0, 1.0, 0.0)), hash13(i + vec3f(1.0, 1.0, 0.0)), w.x);
    let c = mix(hash13(i + vec3f(0.0, 0.0, 1.0)), hash13(i + vec3f(1.0, 0.0, 1.0)), w.x);
    let d = mix(hash13(i + vec3f(0.0, 1.0, 1.0)), hash13(i + vec3f(1.0, 1.0, 1.0)), w.x);
    return mix(mix(a, b, w.y), mix(c, d, w.y), w.z);
}

// Both fbms return about 0..1 whatever the octave count.
fn fbm2(p0: vec2f) -> f32 {
    var p = p0;
    var amp = 0.5;
    var sum = 0.0;
    for (var i = 0; i < 5; i++) {
        sum += amp * noise2(p);
        p = mat2x2f(1.6, 1.2, -1.2, 1.6) * p + vec2f(3.1, 1.7);
        amp *= 0.5;
    }
    return sum / 0.96875;
}

fn fbm3(p0: vec3f, octaves: i32) -> f32 {
    var p = p0;
    var amp = 0.5;
    var sum = 0.0;
    var total = 0.0;
    for (var i = 0; i < octaves; i++) {
        sum += amp * noise3(p);
        total += amp;
        p = p * 2.03 + vec3f(11.3, 7.1, 5.7);
        amp *= 0.5;
    }
    return sum / total;
}

// ---------------------------------------------------------------- sky

// One depth of stars: a jittered point per cell, a few of them bright. `cell_px` is a cell's
// size in device px, so a star keeps its size in pixels whatever the density.
fn star_layer(uv: vec2f, seed: f32, cell_px: f32, t: f32) -> vec3f {
    let g = floor(uv);
    let f = fract(uv) - 0.5;
    let h = hash22(g + seed);
    let m = hash12(g * 1.37 + seed + 3.1);
    if (m < 0.3) {
        return vec3f(0.0);
    }
    let d = length(f - (h - 0.5) * 0.6);
    let mag = pow(m, 11.0);
    let r = (0.5 + 1.9 * mag) * u.view.w / cell_px;
    let core = 0.14 * r * r / (d * d + 0.14 * r * r);
    let fade = 1.0 - smoothstep(0.22, 0.48, d);
    let twinkle = 0.82 + 0.18 * sin(t * (1.5 + 4.0 * h.x) + h.y * 40.0);
    let tint = mix(vec3f(0.74, 0.84, 1.0), vec3f(1.0, 0.87, 0.72), smoothstep(0.35, 0.9, h.y));
    return tint * core * fade * twinkle * (0.2 + 1.3 * mag);
}

fn nebula(p: vec2f, t: f32) -> vec3f {
    let seed = u.neb_a.a;
    let np = p * 1.5 + vec2f(seed * 7.3 + t * 0.004, seed * 3.1);
    let bend = vec2f(noise2(np * 1.3 + 3.7), noise2(np * 1.3 - 5.2)) - 0.5;
    let n1 = fbm2(np + bend * 1.6);
    let n2 = fbm2(np * 0.7 - bend * 1.2 + 11.0);
    let cloud = smoothstep(0.38, 0.86, n1);
    let dust = smoothstep(0.36, 0.82, n2);
    let glow = u.neb_a.rgb * cloud * cloud * 0.6 + u.neb_b.rgb * dust * (1.0 - 0.6 * cloud) * 0.34;
    return glow * u.neb_b.a;
}

fn sun_glare(d: vec2f, size: f32, tint: vec3f) -> vec3f {
    let r = length(d);
    let disc = 1.0 - smoothstep(size * 0.55, size, r);
    let glow = 0.95 * exp(-r / (size * 1.3)) + 0.3 * exp(-r / (size * 5.0)) + 0.07 * exp(-r / (size * 17.0));
    let spikes = (exp(-abs(d.y) / (size * 0.16)) + exp(-abs(d.x) / (size * 0.16))) * exp(-r / (size * 4.5)) * 0.5;
    let streak = exp(-abs(d.y) / (size * 0.4)) * exp(-abs(d.x) / (size * 22.0)) * 0.3;
    return vec3f(1.0) * disc * 1.6 + tint * (glow + spikes + streak);
}

// A soft disc, a little brighter toward its rim: one lens-flare ghost.
fn ghost(d: vec2f, radius: f32) -> f32 {
    let r = length(d) / radius;
    return (1.0 - smoothstep(0.55, 1.0, r)) * (0.4 + 0.6 * smoothstep(0.0, 0.8, r));
}

// ---------------------------------------------------------------- warp

// Stars in polar, log-radius cells. As `travel` grows each one runs outward, faster the further
// it gets, and `warp` draws its tail out behind it. Every lane around the axis has its own pace
// and phase, so the stars never line up in rings, and they thin out toward the axis, where the
// cells crowd together.
fn streak_layer(p: vec2f, n_ang: f32, k: f32, travel: f32, seed: f32, warp: f32) -> f32 {
    let r = length(p);
    let uu = (atan2(p.y, p.x) / TAU + 0.5) * n_ang;
    let gu = floor(uu);
    let lane = gu - n_ang * floor(gu / n_ang);
    let pace = hash22(vec2f(lane, seed));
    let vv = log(max(r, 1e-4)) * k - travel * (0.65 + 0.7 * pace.x) + pace.y * 17.0;
    let cell = vec2f(lane, floor(vv));
    let m = hash12(cell * 1.7 + seed + 9.2);
    let keep = mix(0.94, 0.5, smoothstep(0.03, 0.55, r));
    let there = smoothstep(keep, keep + 0.05, m);
    if (there <= 0.0) {
        return 0.0;
    }
    let h = hash22(cell + seed);
    let fu = fract(uu) - 0.5 - (h.x - 0.5) * 0.7;
    let fv = fract(vv);
    let arc_px = TAU * r * u.rect.w / n_ang;
    let wu = min((0.8 + 1.6 * h.x) * u.view.w / max(arc_px, 1.0), 0.3);
    let across = exp(-fu * fu / (wu * wu));
    let head = 0.55 + 0.4 * h.y;
    let len = mix(0.03, 0.5, warp);
    let along = smoothstep(head - len, head, fv) * (1.0 - smoothstep(head, head + 0.015, fv));
    return across * along * there * (0.4 + 1.5 * r) * smoothstep(0.03, 0.2, r) * (0.6 + 0.8 * h.y);
}

// Plasma strands running down the tunnel walls; seamless around the axis.
fn tunnel(p: vec2f, travel: f32) -> vec3f {
    let r = length(p) + 1e-4;
    let dir = p / r;
    let z = log(r) * 1.5 - travel * 0.3;
    let n = fbm3(vec3f(dir * 1.7, z), 4);
    let n2 = fbm3(vec3f(dir * 3.3 + 4.0, z * 1.6 + 9.0), 3);
    let strands = smoothstep(0.46, 0.8, n);
    let blue = mix(vec3f(0.04, 0.18, 0.85), vec3f(0.2, 0.85, 1.0), n2);
    let violet = vec3f(0.55, 0.2, 0.95) * smoothstep(0.52, 0.85, n2);
    return (blue * strands + violet * 0.45) * smoothstep(0.04, 0.8, r);
}

// ---------------------------------------------------------------- worlds

// A world's own frame from the unit normal seen on screen: x and z around the equator, y along
// the spin axis. `roll` turns the axis on screen, `tip` (0..1) leans its north pole at the viewer.
fn world_frame(n: vec3f, roll: f32, tip: f32, spin: f32) -> vec3f {
    let ux = vec2f(cos(roll), sin(roll));
    let wx = vec2f(-ux.y, ux.x);
    let kz = sqrt(1.0 - tip * tip);
    let s = vec3f(dot(n, vec3f(ux, 0.0)), dot(n, vec3f(-wx * kz, tip)), dot(n, vec3f(wx * tip, kz)));
    let c = cos(spin);
    let sn = sin(spin);
    return vec3f(c * s.x + sn * s.z, s.y, -sn * s.x + c * s.z);
}

// Light scattered just outside the limb, brighter toward the sun.
fn halo(d: vec2f, r: f32, light: vec3f, falloff: f32) -> vec3f {
    let toward = dot(d / max(r, 1e-4), normalize(light.xy + vec2f(1e-5, 0.0)));
    let side = smoothstep(-0.35, 0.9, toward);
    return u.atmo.rgb * u.atmo.a * exp(-(r - 1.0) * falloff) * (0.1 + 0.8 * side);
}

// One size of crater: at most one per cell of a grid through the globe, a dark floor inside a
// bright rim. Where the cell's point lies off the surface the crater comes out smaller, or not
// at all.
fn crater_layer(s: vec3f, scale: f32, seed: f32) -> f32 {
    let p = s * scale + seed;
    let cell = floor(p);
    let centre = 0.3 + 0.4 * hash33(cell + seed);
    let radius = 0.12 + 0.16 * hash13(cell * 1.3 + seed + 5.0);
    let d = length(fract(p) - centre) / radius;
    let pit = 1.0 - smoothstep(0.55, 0.95, d);
    let lip = d - 1.0;
    return exp(-lip * lip * 30.0) * 0.45 - pit * 0.4;
}

// The height of a rocky world's surface at `s` on the unit globe, 0..1: continents bent out of
// shape by a slow warp, with finer and finer relief on top.
fn terrain(s: vec3f, seed: f32, octaves: i32) -> f32 {
    let bend = vec3f(noise3(s * 2.3 + seed + 11.0), noise3(s * 2.3 + seed + 37.0), noise3(s * 2.3 + seed + 71.0)) - 0.5;
    return fbm3((s + bend * 0.2) * 1.9 + seed, octaves);
}

// Cloud cover at `s`, 0..1: fronts of weather torn into wisps.
fn weather(s: vec3f, seed: f32, t: f32, octaves: i32) -> f32 {
    let drift = vec3f(t * 0.012, 0.0, seed + 5.0);
    let swirl = vec3f(noise3(s * 3.7 + drift), noise3(s * 3.7 + drift + 19.0), noise3(s * 3.7 + drift + 43.0)) - 0.5;
    let cover = fbm3((s + swirl * 0.24) * 3.1 + drift, octaves);
    // Small gaps eat into the edges of the fronts.
    return smoothstep(0.525, 0.66, cover - 0.1 * (fbm3(s * 15.0 + drift * 2.0, 3) - 0.5));
}

// Seas, land, ice, clouds, craters and a night side. Returns premultiplied colour and coverage;
// outside the disc the colour is the atmosphere's glow, to add.
fn rocky(d: vec2f, light: vec3f, t: f32, px_r: f32) -> vec4f {
    let rr = dot(d, d);
    let r = sqrt(rr);
    if (rr >= 1.0) {
        return vec4f(halo(d, r, light, 13.0), 0.0);
    }
    let nz = sqrt(1.0 - rr);
    let n = vec3f(d, nz);
    let seed = u.c0.a;
    let spin = u.c1.a + t * 0.022;
    let s = world_frame(n, 0.41, 0.22, spin);
    // The Sun's direction in the planet's own frame, to shade relief and drop cloud shadows.
    let sunward = world_frame(light, 0.41, 0.22, spin);
    // As many octaves as the globe has pixels for, the finest about four pixels across: the
    // closer the planet, the more there is to see.
    let lod = i32(clamp(round(1.0 + log2(px_r / 6.5) / 1.02), 3.0, 9.0));
    let hgt = terrain(s, seed, lod);
    let detail = fbm3(s * 8.3 + seed * 2.0 + 17.0, max(lod - 2, 2));
    let h = hgt * 0.84 + detail * 0.16;
    let sea = u.surf.x;
    let wet = smoothstep(0.3, 0.6, u.surf.y);
    let land = smoothstep(sea - 0.005, sea + 0.005, h);

    // Seas: dark in the deeps, paler over the shelves of a world with weather.
    var water = mix(u.c0.rgb * 0.38, u.c0.rgb, smoothstep(sea - 0.22, sea, h));
    water = mix(water, u.c0.rgb + vec3f(0.05, 0.22, 0.2), smoothstep(sea - 0.035, sea, h) * 0.5 * wet);
    // Land: lowlands up to highlands, dry country in patches away from the poles, and snow on
    // the peaks where there is weather to bring it.
    var ground = mix(u.c1.rgb, u.c2.rgb, smoothstep(sea, sea + 0.26, h + (detail - 0.5) * 0.2));
    let dry = smoothstep(0.46, 0.64, noise3(s * 2.9 + seed + 53.0) + (detail - 0.5) * 0.3) * (1.0 - abs(s.y));
    ground = mix(ground, u.c2.rgb * vec3f(1.08, 0.98, 0.8), dry * 0.6);
    ground *= 0.8 + 0.4 * detail;
    ground = mix(ground, vec3f(0.9, 0.93, 0.96), smoothstep(sea + 0.19, sea + 0.26, h) * wet * 0.85);
    var albedo = mix(water, ground, land);
    let pocked = u.more.x;
    if (pocked > 0.0) {
        let dents = crater_layer(s, 4.3, seed) + 0.8 * crater_layer(s, 8.9, seed + 3.0) + 0.6 * crater_layer(s, 18.7, seed + 7.0);
        albedo *= max(1.0 + pocked * dents, 0.2);
    }
    let ice = smoothstep(0.7, 0.86, abs(s.y) + (detail - 0.5) * 0.3) * u.surf.w;
    albedo = mix(albedo, vec3f(0.92, 0.96, 1.0), ice);
    // Relief: ground that climbs toward the Sun faces away from it.
    let ahead = terrain(normalize(s + sunward * 0.012), seed, lod);
    albedo *= clamp(1.0 + (hgt - ahead) * 15.0 * land * (1.0 - ice), 0.55, 1.45);

    var clouds = 0.0;
    if (u.surf.y > 0.0) {
        let puffs = max(lod - 1, 3);
        clouds = weather(s, seed, t, puffs) * u.surf.y;
        // Clouds nearer the Sun throw their shadows on the ground, and on the clouds behind.
        let upwind = weather(normalize(s + sunward * 0.022), seed, t, puffs) * u.surf.y;
        albedo *= 1.0 - 0.42 * upwind * (1.0 - clouds);
        let top = mix(vec3f(0.66, 0.71, 0.8), vec3f(1.0), clamp(0.62 + (clouds - upwind) * 1.4, 0.0, 1.0));
        albedo = mix(albedo, top, clouds * 0.94);
    }

    let ndl = dot(n, light);
    let lit = smoothstep(-0.1, 0.75, ndl);
    var c = albedo * (0.012 + 1.05 * lit);
    // Only a world wet enough for weather has seas that shine: a hot spot in a wider sheen.
    let facing = max(dot(n, normalize(light + vec3f(0.0, 0.0, 1.0))), 0.0);
    c += vec3f(1.0, 0.95, 0.85) * (pow(facing, 90.0) * 0.9 + pow(facing, 14.0) * 0.1) * (1.0 - land) * (1.0 - clouds) * lit * wet;

    let lights = u.surf.z;
    if (lights > 0.0) {
        // Cities: specks of light in clusters, thickest near the coasts, lit on the night side.
        let settled = smoothstep(0.5, 0.72, fbm3(s * 6.5 + seed + 31.0, 3)) * (0.35 + 0.65 * (1.0 - smoothstep(sea, sea + 0.1, h)));
        let specks = smoothstep(0.45, 0.8, noise3(s * 52.0 + seed)) * (0.5 + noise3(s * 23.0 + seed + 9.0));
        let night = 1.0 - smoothstep(-0.12, 0.2, ndl);
        c += vec3f(1.0, 0.76, 0.42) * settled * specks * land * (1.0 - ice) * (1.0 - clouds) * night * lights;
    }

    let air = u.atmo.rgb * u.atmo.a;
    c += air * pow(1.0 - nz, 2.6) * (0.12 + 1.1 * lit) + air * 0.06 * lit;
    // The line between day and night runs warm where there is air to redden the light.
    c += vec3f(1.0, 0.42, 0.18) * exp(-ndl * ndl * 70.0) * u.atmo.a * wet * 0.07;
    let edge = 1.0 - smoothstep(1.0 - 1.5 / px_r, 1.0, r);
    return vec4f(c * edge, edge);
}

// How much ring there is `rho` planet radii from the axis: broad sheets with a gap in them, or a
// faint sheet inside one narrow ring.
fn ring_density(rho: f32, narrow: bool) -> f32 {
    if (narrow) {
        // A faint sheet of inner rings, then the one ring bright enough to pick out.
        let sheet = smoothstep(1.62, 1.66, rho) * (1.0 - smoothstep(1.88, 1.92, rho)) * (0.07 + 0.1 * noise2(vec2f(rho * 70.0, 2.0)));
        let d = (rho - 2.0) / 0.014;
        return min(sheet + exp(-d * d) * 0.8, 1.0);
    }
    var v = 0.5 + 0.5 * noise2(vec2f(rho * 27.0, 0.5));
    v *= 0.62 + 0.38 * noise2(vec2f(rho * 97.0, 7.5));
    let gap = 1.0 - 0.9 * (1.0 - smoothstep(0.0, 0.035, abs(rho - 1.98)));
    let edges = smoothstep(1.3, 1.36, rho) * (1.0 - smoothstep(2.36, 2.45, rho));
    return v * gap * edges * (0.55 + 0.45 * smoothstep(1.3, 1.7, rho));
}

// A globe of cloud belts around its axis, with a storm in them and rings in its equatorial
// plane. The rings' far half goes behind the globe and the near half in front, and each shadows
// the other.
fn banded(d: vec2f, light: vec3f, t: f32, px_r: f32) -> vec4f {
    let rr = dot(d, d);
    let r = sqrt(rr);
    let roll = u.surf.x;
    let tip = u.surf.y;
    let storm = u.surf.z;
    let rings = abs(u.surf.w);
    let narrow = u.surf.w < 0.0;
    let ux = vec2f(cos(roll), sin(roll));
    let wx = vec2f(-ux.y, ux.x);
    let kz = sqrt(1.0 - tip * tip);
    let pole = vec3f(-wx * kz, tip);
    var shade = vec4f(0.0);
    if (rr < 1.0) {
        let nz = sqrt(1.0 - rr);
        let n = vec3f(d, nz);
        let seed = u.c0.a;
        let s = world_frame(n, roll, tip, u.c1.a + t * 0.05);
        let turb = fbm3(vec3f(s.x * 1.7, s.y * 5.5, s.z * 1.7) + seed, 4) - 0.5;
        let b1 = sin(s.y * 13.0 + turb * 3.4);
        let b2 = sin(s.y * 29.0 + turb * 6.5 + 1.3);
        var albedo = mix(u.c0.rgb, u.c1.rgb, b1 * 0.5 + 0.5);
        albedo = mix(albedo, u.c2.rgb, smoothstep(0.15, 1.0, b2) * 0.55);
        if (storm != 0.0) {
            // An oval in the southern belts that turns with the planet.
            let eye = normalize(vec3f(0.62, -0.38, 0.69));
            let east = normalize(cross(vec3f(0.0, 1.0, 0.0), eye));
            let north = cross(eye, east);
            let off = s - eye;
            let near = step(0.0, dot(s, eye));
            let e = length(vec2f(dot(off, east) / 0.3, dot(off, north) / 0.15)) + turb * 0.25;
            let oval = (1.0 - smoothstep(0.75, 1.0, e)) * near;
            if (storm > 0.0) {
                albedo = mix(albedo, vec3f(0.74, 0.27, 0.15) + 0.12 * (1.0 - e), oval * storm * 0.9);
            } else {
                albedo = mix(albedo, albedo * 0.3, oval * -storm * 0.85);
                let lift = dot(off, north) / 0.15 - 1.5;
                let cirrus = exp(-lift * lift * 6.0) * (1.0 - smoothstep(0.7, 1.5, abs(dot(off, east)) / 0.3)) * near;
                albedo = mix(albedo, vec3f(0.92, 0.96, 1.0), cirrus * -storm * 0.75);
            }
        }
        let haze = u.more.z;
        if (haze > 0.0) {
            // Deep haze hides the belts, and a pale hood sits over the pole.
            albedo = mix(albedo, mix(u.c0.rgb, u.c1.rgb, 0.5), haze);
            albedo = mix(albedo, vec3f(0.94, 0.99, 0.98), smoothstep(0.2, 0.95, s.y) * haze * 0.42);
        }
        let ndl = dot(n, light);
        var lit = smoothstep(-0.1, 0.75, ndl);
        if (rings > 0.0 && !narrow) {
            // Where the ray to the sun crosses the ring plane.
            let slope = dot(light, pole);
            let reach = -dot(n, pole) / select(slope, 1e-3, abs(slope) < 1e-3);
            if (reach > 0.0) {
                lit *= 1.0 - 0.7 * ring_density(length(n + light * reach), false) * rings;
            }
        }
        var c = albedo * (0.012 + 1.05 * lit) * mix(0.6, 1.0, pow(nz, 0.5));
        let air = u.atmo.rgb * u.atmo.a;
        c += air * pow(1.0 - nz, 2.6) * (0.1 + lit);
        let edge = 1.0 - smoothstep(1.0 - 1.5 / px_r, 1.0, r);
        shade = vec4f(c * edge, edge);
    } else {
        shade = vec4f(halo(d, r, light, 17.0) * 0.6, 0.0);
    }
    if (rings > 0.0) {
        let a = dot(d, ux);
        let bk = dot(d, wx);
        let b = bk / tip;
        let rho = sqrt(a * a + b * b);
        if (rho > 1.3 && rho < 2.45 && (b > 0.0 || rr >= 1.0)) {
            let at = vec3f(a * ux + bk * wx, b * kz);
            let along = dot(at, light);
            let behind = along * along - (dot(at, at) - 1.0);
            let shadow = smoothstep(0.0, 0.1, behind) * step(along, 0.0);
            let alpha = ring_density(rho, narrow) * rings;
            let grain = mix(u.c1.rgb, u.c2.rgb, 0.35 + 0.4 * noise2(vec2f(rho * 13.0, 3.0)));
            let c = select(grain, vec3f(0.7, 0.8, 0.84), narrow) * (1.0 - 0.94 * shadow) * 0.95;
            shade = vec4f(shade.rgb * (1.0 - alpha) + c * alpha, shade.a + (1.0 - shade.a) * alpha);
        }
    }
    return shade;
}

// ---------------------------------------------------------------- frame

@fragment
fn fs(in: VOut) -> @location(0) vec4f {
    let res = u.rect.zw;
    let t = u.view.z;
    let dpr = u.view.w;
    let warp = u.drive.x;
    let travel = u.drive.w;

    let jolt = vec2f(noise2(vec2f(t * 23.0, 1.7)), noise2(vec2f(4.2, t * 27.0))) - 0.5;
    let shake = jolt * u.drive.z * 7.0 * dpr;
    let q = in.px - u.rect.xy + shake;
    let p = (q - 0.5 * res) / res.y;

    let kind = u.body.w;
    let seen = u.c2.a;
    let bc = (u.body.xy - u.rect.xy + shake - 0.5 * res) / res.y;
    let br = u.body.z / res.y;
    let has_body = br > 0.0005 && seen > 0.001;

    var col = vec3f(0.0);

    // The sky at rest. As the drive spools it starts to flow outward, then gives way to the tunnel.
    let rest = 1.0 - smoothstep(0.15, 0.85, warp);
    if (rest > 0.001) {
        let sky = p * (1.0 - 0.08 * smoothstep(0.0, 0.3, warp));
        var back = nebula(sky, t);
        let drift = vec2f(t * 0.0026, t * 0.0009);
        back += star_layer((sky + drift) * 26.0, 11.0, res.y / 26.0, t);
        back += star_layer((sky + drift * 0.6) * 47.0 + 13.7, 37.0, res.y / 47.0, t) * 0.8;
        back += star_layer((sky + drift * 0.35) * 83.0 + 41.3, 71.0, res.y / 83.0, t) * 0.6;
        if (u.sun.z > 0.0) {
            back += sun_glare(p - u.sun.xy, u.sun.z, u.sun_col.rgb);
            // Flare ghosts across the middle of the lens, fainter the smaller the Sun is.
            let flare = clamp(u.sun.z / 0.02, 0.0, 1.0) * u.more.y;
            back += u.sun_col.rgb * ghost(p + u.sun.xy * 0.42, 0.045) * 0.022 * flare;
            back += vec3f(0.4, 0.7, 1.0) * ghost(p + u.sun.xy * 0.95, 0.02) * 0.035 * flare;
        }
        col = back * rest;
    }

    if (has_body) {
        let d = (p - bc) / br;
        let px_r = u.body.z;
        var world = vec4f(0.0);
        if (kind < 0.5) {
            world = rocky(d, normalize(vec3f(u.sun.xy - bc, 0.5)), t, px_r);
        } else {
            world = banded(d, normalize(vec3f(u.sun.xy - bc, 0.5)), t, px_r);
        }
        col = col * (1.0 - world.a * seen) + world.rgb * seen;
    }

    if (warp > 0.001) {
        // Three depths of streaks, each sampled a little apart per channel so they fringe.
        let split = 0.016 * warp;
        var s = vec3f(0.0);
        s += vec3f(
            streak_layer(p * (1.0 + split), 56.0, 1.9, travel, 5.0, warp),
            streak_layer(p, 56.0, 1.9, travel, 5.0, warp),
            streak_layer(p * (1.0 - split), 56.0, 1.9, travel, 5.0, warp),
        );
        s += 0.8 * vec3f(
            streak_layer(p * (1.0 + split), 97.0, 2.6, travel * 0.72, 23.0, warp),
            streak_layer(p, 97.0, 2.6, travel * 0.72, 23.0, warp),
            streak_layer(p * (1.0 - split), 97.0, 2.6, travel * 0.72, 23.0, warp),
        );
        s += 0.6 * vec3f(
            streak_layer(p * (1.0 + split), 151.0, 3.3, travel * 0.5, 47.0, warp),
            streak_layer(p, 151.0, 3.3, travel * 0.5, 47.0, warp),
            streak_layer(p * (1.0 - split), 151.0, 3.3, travel * 0.5, 47.0, warp),
        );
        col += s * vec3f(0.82, 0.92, 1.0) * smoothstep(0.0, 0.25, warp) * 1.7;
        col += tunnel(p, travel) * warp * warp * 0.75;
        let rc = dot(p, p);
        col += vec3f(0.7, 0.88, 1.0) * (0.016 / (rc + 0.016)) * warp * 0.95;
    }

    col += vec3f(0.8, 0.92, 1.0) * u.drive.y * (0.3 + 0.7 * exp(-length(p) * 2.2));

    let corner = length((q / res - 0.5) * vec2f(1.0, 0.92)) * 1.3;
    col *= 1.0 - 0.5 * clamp(pow(corner, 2.6), 0.0, 1.0);

    col = 1.0 - exp(-col * 1.4);
    col += (hash12(in.px + fract(t) * 91.7) - 0.5) * 1.5 / 255.0;
    col = max(col, vec3f(0.0));
    if (u.sun_col.a < 0.5) {
        col = pow(col, vec3f(2.2));
    }
    return vec4f(col, 1.0);
}
