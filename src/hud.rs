//! The HUD: every panel, readout and control, as plain Rune elements laid out over the space
//! scene. Effects are empty `effect` slots the compositor paints into (`gfx.rs`).

use std::f32::consts::TAU;

use rune::tok::motion;
use rune::{Color, Cursor, Cx, El, Family, LineCap, Shape, Spring, TextStyle, Transition, canvas, col, effect, el, row, rule, spacer, text};
use rune_motion::LoopSpec;

use crate::gfx::{ELECTRIC, GLOW, PULSE, SHOCK, SPACE, SPARKLES};
use crate::voyage::{Drive, Phase, State, grouped};
use crate::worlds::WORLDS;

const INK: u32 = 0xe9f1ff;
const EDGE: f32 = 20.0;
const CARD_W: f32 = 264.0;
const CARD_TOP: f32 = 76.0;
const ROW_H: f32 = 44.0;

/// An sRGB hex colour.
fn c(hex: u32) -> Color {
    let ch = |shift: u32| rune::color::srgb_decode(((hex >> shift) & 0xff) as f32 / 255.0);
    Color { r: ch(16), g: ch(8), b: ch(0), a: 1.0 }
}

fn ca(hex: u32, a: f32) -> Color {
    c(hex).alpha(a)
}

/// A small spaced-out label.
fn caption(s: impl Into<String>) -> El {
    text(s).font(TextStyle::new(10.0, 600.0).tracking(1.5)).color(ca(INK, 0.5))
}

/// The style of instrument figures.
fn figures(size: f32) -> TextStyle {
    TextStyle::new(size, 500.0).family(Family::SystemMono)
}

/// An effect slot the compositor fills: `mark` picks the effect, `data` are its numbers.
fn fx(mark: u32, data: [f32; 4]) -> El {
    effect(mark).effect_data(data)
}

/// Frosted glass: whatever is behind the element, blurred, under a dark tint and a hairline.
fn glass(e: El) -> El {
    e.fill(ca(0x060a14, 0.5)).backdrop_blur(20.0).border(1.0, ca(0xffffff, 0.09)).shadow(14.0, 44.0, ca(0x000000, 0.4))
}

fn dot(hex: u32, size: f32) -> El {
    el().size(size, size).radius(size / 2.0).fill(c(hex)).shadow(0.0, size, ca(hex, 0.9))
}

/// Points along a circle from angle `a0` to `a1`.
fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, steps: usize) -> Vec<[f32; 2]> {
    (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            [cx + a.cos() * r, cy + a.sin() * r]
        })
        .collect()
}

/// A world with something in orbit.
fn logo(accent: u32) -> Shape {
    let orbit: Vec<[f32; 2]> = (0..=32)
        .map(|i| {
            let a = i as f32 * TAU / 32.0;
            let (x, y) = (a.cos() * 10.0, a.sin() * 3.6);
            [11.0 + x * 0.906 + y * 0.423, 11.0 - x * 0.423 + y * 0.906]
        })
        .collect();
    Shape::new().fill(arc(11.0, 11.0, 4.6, 0.0, TAU, 20), c(accent)).stroke(orbit, 1.2, ca(INK, 0.8)).fill(arc(19.6, 6.6, 1.7, 0.0, TAU, 10), c(INK))
}

fn top_bar(s: &State, accent: u32) -> El {
    let w = s.world();
    let clock = format!("T+ {:02}:{:04.1}", (s.now / 60.0) as u32, s.now % 60.0);
    let busy = !s.in_orbit();
    let lamp = dot(if busy { 0xffd166 } else { accent }, 7.0).looping(LoopSpec::opacity(if busy { 0.45 } else { 1.8 }, 0.0, 0.3, 1.0));
    let place = if matches!(s.phase, Phase::Warp { .. }) { WORLDS[s.bound].name } else { w.name };
    let status = row().key("status").h(28.0).px(12.0).gap(8.0).center_y().radius(14.0).fill(ca(0xffffff, 0.06)).border(1.0, ca(0xffffff, 0.1))
        .child(lamp)
        .child(text(s.status()).font(TextStyle::new(10.5, 600.0).tracking(1.4)).color(c(INK)))
        .child(text(place.to_uppercase()).font(TextStyle::new(10.5, 600.0).tracking(1.4)).color(ca(INK, 0.5)));
    let gauge = |label: &str, value: String, width: f32| col().w(width).gap(3.0).child(caption(label)).child(text(value).font(figures(12.5)).color(ca(INK, 0.9)));
    row().key("top").w_full().h(36.0).center_y().gap(12.0)
        .child(canvas(logo(accent)).size(22.0, 22.0))
        .child(text("VOYAGER").font(TextStyle::new(13.0, 600.0).tracking(4.5)).color(c(INK)))
        .child(text("DEEP SPACE NAVIGATION").font(TextStyle::new(10.0, 500.0).tracking(1.6)).color(ca(INK, 0.4)))
        .child(spacer())
        .child(status)
        .child(gauge("MISSION CLOCK", clock, 104.0).ml(8.0))
        .child(gauge("FLOWN", format!("{} AU", grouped(s.flown as f32, 1)), 96.0))
}

/// The chart: every world, how far it is from here, and a highlight that springs to the pick.
fn nav(s: &State) -> El {
    let live = s.in_orbit();
    let picked = &WORLDS[s.selected];
    let mut list = col().w_full().child(
        el().key("nav-pick").absolute(0.0, s.selected as f32 * ROW_H).w_full().h(ROW_H).radius(12.0)
            .fill(ca(0xffffff, 0.07)).border(1.0, ca(picked.accent, 0.5)).motion(Spring::new(0.38, 0.22)),
    );
    for (i, w) in WORLDS.iter().enumerate() {
        let dim = if live || i == s.selected { 1.0 } else { 0.5 };
        let far = WORLDS[s.here].distance_to(w);
        let side = if i == s.here {
            row().h(18.0).px(7.0).center().radius(9.0).fill(ca(w.accent, 0.16)).child(text("HERE").font(TextStyle::new(9.0, 600.0).tracking(1.2)).color(c(w.accent)))
        } else {
            text(format!("{far:.2} AU")).font(figures(11.5)).color(ca(INK, 0.6 * dim))
        };
        list = list.child(
            row().key(format!("world-{i}")).role("button").label(w.name).w_full().h(ROW_H).px(12.0).gap(12.0).center_y().radius(12.0)
                .hover(ca(0xffffff, 0.04)).cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| s.select(i))
                .child(dot(w.accent, 9.0))
                .child(
                    col().gap(1.0)
                        .child(text(w.name).font(TextStyle::new(14.0, 600.0)).color(ca(INK, dim)))
                        .child(text(w.class).font(TextStyle::new(11.0, 400.0)).color(ca(INK, 0.45 * dim))),
                )
                .child(spacer())
                .child(side),
        );
    }
    glass(col()).key("nav").w(280.0).padding(14.0).gap(12.0).radius(18.0)
        .child(
            row().px(6.0).pt(4.0).center_y()
                .child(caption("DESTINATIONS"))
                .child(spacer())
                .child(text(format!("{} CHARTED", WORLDS.len())).font(figures(10.0)).color(ca(INK, 0.35))),
        )
        .child(list)
        .child(rule(1.0, ca(0xffffff, 0.07)))
        .child(text(format!("\u{2191}\u{2193} select   Enter engage   A autopilot   M {}", if s.muted { "unmute" } else { "mute" })).font(TextStyle::new(10.5, 400.0)).color(ca(INK, 0.38)).px(6.0).pb(2.0))
}

/// What the ship is looking at.
fn card(s: &State) -> El {
    let w = s.world();
    let heading = match s.phase {
        Phase::Spool { .. } => "DEPARTING",
        Phase::Arrive { .. } => "ON APPROACH",
        _ => "IN ORBIT",
    };
    let mut facts = col().w_full().gap(8.0);
    for (name, value) in w.facts {
        facts = facts.child(row().w_full().center_y().child(caption(name)).child(spacer()).child(text(value).font(figures(12.0)).color(ca(INK, 0.9))));
    }
    glass(col()).key(format!("card-{}", s.here)).w(CARD_W).padding(18.0).gap(10.0).radius(18.0)
        .transition(Transition::blur_replace())
        .child(row().gap(8.0).center_y().child(dot(w.accent, 7.0)).child(caption(heading)))
        .child(text(w.name).font(TextStyle::new(26.0, 600.0).tracking(-0.4)).color(c(INK)))
        .child(text(format!("{} \u{00b7} {:.2} AU from the Sun", w.class, w.orbit_au)).font(TextStyle::new(12.0, 400.0)).color(ca(INK, 0.55)))
        .child(rule(1.0, ca(0xffffff, 0.08)))
        .child(facts)
}

/// The targeting ring around the world, and the leader out to its card.
fn reticle(s: &State, w: f32, h: f32, accent: u32) -> Shape {
    let pose = s.pose();
    let (cx, cy) = (pose.at[0] * w, pose.at[1] * h);
    let r = ring_radius(s, h) * 1.2;
    let turn = s.now as f32 * 0.22;
    let mut shape = Shape::new();
    for q in 0..4 {
        let a = turn + q as f32 * TAU / 4.0 + 0.34;
        shape = shape.stroke(arc(cx, cy, r, a, a + 0.9, 18), 1.5, ca(accent, 0.9));
    }
    for q in 0..3 {
        let a = -turn * 0.6 + q as f32 * TAU / 3.0;
        shape = shape.stroke(arc(cx, cy, r + 9.0, a, a + 1.5, 24), 1.0, ca(INK, 0.22));
    }
    for i in 0..72 {
        let a = i as f32 * TAU / 72.0;
        let (near, far) = (r + 17.0, r + if i % 6 == 0 { 25.0 } else { 20.0 });
        shape = shape.line([cx + a.cos() * near, cy + a.sin() * near], [cx + a.cos() * far, cy + a.sin() * far], 1.0, LineCap::Butt, ca(INK, if i % 6 == 0 { 0.5 } else { 0.2 }));
    }
    // From the ring's upper right across to the card's edge, turning a corner only when the
    // window is tall enough to leave the card above the ring.
    let a = -0.9f32;
    let from = [cx + a.cos() * (r + 30.0), cy + a.sin() * (r + 30.0)];
    let to = [w - EDGE - CARD_W, from[1].clamp(CARD_TOP + 24.0, CARD_TOP + 150.0)];
    shape.stroke(vec![from, [from[0], to[1]], to], 1.0, ca(accent, 0.55)).fill(arc(from[0], from[1], 2.5, 0.0, TAU, 10), c(accent))
}

/// The planet's radius in points, which the reticle and the radar rings go around.
fn ring_radius(s: &State, h: f32) -> f32 {
    s.pose().radius * h
}

/// The drive's hum: two beating sines, louder with the charge.
fn scope(t: f32, level: f32, w: f32, h: f32, accent: u32) -> Shape {
    let mid = h / 2.0;
    let amp = (0.14 + 0.86 * level) * (mid - 1.0);
    let rate = 2.0 + 9.0 * level;
    let line: Vec<[f32; 2]> = (0..=72)
        .map(|i| {
            let x = i as f32 / 72.0;
            let envelope = (x * std::f32::consts::PI).sin();
            let y = (x * 21.0 + t * rate).sin() * 0.6 + (x * 47.0 - t * rate * 1.7).sin() * 0.4;
            [x * w, mid + y * amp * envelope]
        })
        .collect();
    Shape::new().line([0.0, mid], [w, mid], 0.5, LineCap::Butt, ca(INK, 0.18)).stroke(line, 1.25, c(accent))
}

fn readout(label: &str, value: String, unit: &str, width: f32) -> El {
    col().w(width).gap(5.0)
        .child(caption(label))
        .child(
            // The figure changes width every frame in flight; its unit follows at once.
            row().gap(7.0).items_end().instant_changes()
                .child(text(value).font(figures(28.0)).color(c(INK)))
                .child(text(unit).font(TextStyle::new(12.0, 500.0)).color(ca(INK, 0.5)).pb(5.0)),
        )
}

/// The route as a rail the ship's marker runs along.
fn route(s: &State, accent: u32) -> El {
    let (from, to) = s.route();
    let done = s.progress();
    let name = |i: usize, a: f32| text(WORLDS[i].name.to_uppercase()).font(TextStyle::new(10.5, 600.0).tracking(1.4)).color(ca(INK, a));
    let rail = el().grow(1.0).h(12.0)
        .child(el().absolute(0.0, 5.0).w_full().h(2.0).radius(1.0).fill(ca(0xffffff, 0.12)))
        .child(el().absolute(0.0, 5.0).w_frac(done).h(2.0).radius(1.0).fill(c(accent)))
        .child(row().absolute(0.0, 0.0).w_frac(done).h(12.0).justify_end().center_y().child(dot(accent, 8.0).mr(-4.0)));
    row().key("route").w_full().center_y().gap(14.0).instant_changes()
        .child(name(from, 0.8))
        .child(rail)
        .child(name(to, if from == to { 0.35 } else { 0.8 }))
        .child(text(format!("{:.2} AU", s.trip_length())).font(figures(11.0)).color(ca(INK, 0.5)))
}

fn autopilot(s: &State, accent: u32) -> El {
    let on = s.autopilot;
    let knob = el().key("autopilot-knob").size(18.0, 18.0).radius(9.0).fill(c(0xffffff)).absolute(if on { 18.0 } else { 2.0 }, 2.0);
    let switch = el().key("autopilot-track").size(38.0, 22.0).radius(11.0).fill(if on { c(accent) } else { ca(0xffffff, 0.16) }).motion(motion::SNAPPY).child(knob);
    row().key("autopilot").role("switch").label("Autopilot").checked(on).gap(10.0).center_y().cursor(Cursor::Pointer)
        .on_press(|s: &mut State| s.toggle_autopilot())
        .child(caption("AUTOPILOT"))
        .child(switch)
}

/// The button the whole window is about. Its glow and lightning are effect slots under and
/// over its face, on its own capsule.
fn engage(s: &State, drive: &Drive, accent: u32) -> El {
    let ready = s.can_engage();
    let flying = !s.in_orbit();
    let label = match s.phase {
        Phase::Orbit { .. } if ready => "ENGAGE",
        Phase::Orbit { .. } => "ON STATION",
        Phase::Spool { .. } => "SPOOLING",
        Phase::Warp { .. } => "IN TRANSIT",
        Phase::Arrive { .. } => "ARRIVING",
    };
    let glow = if ready {
        [2.0, accent as f32, 1.0, 20.0]
    } else if flying {
        [1.0, accent as f32, 0.5 + 0.9 * drive.charge, 18.0]
    } else {
        [0.0, accent as f32, 0.0, 18.0]
    };
    let sparks = match s.phase {
        Phase::Orbit { .. } if ready => [0.8, 1.1, 1.0, accent as f32],
        Phase::Spool { .. } => [0.7 + 0.9 * drive.charge, 2.0 + 7.0 * drive.charge, 0.0, accent as f32],
        _ => [0.0, 0.0, 1.0, accent as f32],
    };
    let face = row().key("engage-face").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radius(27.0).center();
    let face = if ready {
        face.gradient(c(accent).mix(Color::WHITE, 0.3), c(accent).mix(Color::BLACK, 0.25))
    } else if flying {
        face.fill(ca(0x060a14, 0.7)).border(1.0, ca(accent, 0.55))
    } else {
        face.fill(ca(0xffffff, 0.05)).border(1.0, ca(0xffffff, 0.1))
    };
    let ink = if ready { c(0x050b16) } else if flying { c(accent) } else { ca(INK, 0.4) };
    let whole = |mark: u32, data: [f32; 4], bleed: f32| fx(mark, data).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radius(27.0).effect_bleed(bleed);
    el().key("engage").role("button").label("Engage warp drive").disabled(!ready).size(190.0, 54.0)
        .cursor(if ready { Cursor::Pointer } else { Cursor::Default }).press_scale(if ready { 0.96 } else { 1.0 })
        .on_press(|s: &mut State| s.engage())
        .child(whole(GLOW, glow, 70.0).key("engage-glow"))
        .child(face.child(text(label).font(TextStyle::new(13.0, 600.0).tracking(2.8)).color(ink)))
        .child(whole(ELECTRIC, sparks, 40.0).key("engage-sparks"))
}

/// The console along the bottom: the route, the instruments and the drive.
fn console(s: &State, drive: &Drive, accent: u32) -> El {
    let (speed, unit) = s.velocity();
    let level = drive.charge.max(drive.warp);
    let state = text(s.status()).font(TextStyle::new(9.5, 600.0).tracking(1.2)).color(c(if s.in_orbit() { INK } else { accent }));
    let core = col().w(170.0).gap(7.0)
        .child(row().w_full().center_y().child(caption("DRIVE CORE")).child(spacer()).child(state))
        .child(
            el().w_full().h(5.0).radius(2.5).fill(ca(0xffffff, 0.1)).instant_changes()
                .child(el().w_frac(drive.charge.max(0.02)).h(5.0).radius(2.5).fill(c(accent))),
        )
        .child(canvas(scope(s.now as f32, level, 170.0, 22.0, accent)).key("scope").size(170.0, 22.0));
    let post = || el().w(1.0).h(44.0).fill(ca(0xffffff, 0.08));
    glass(col()).key("console").w_full().px(22.0).py(16.0).gap(14.0).radius(22.0)
        .child(route(s, accent))
        .child(
            row().w_full().center_y().gap(22.0)
                .child(readout("VELOCITY", speed, unit, 215.0))
                .child(post())
                .child(readout("DISTANCE TO TARGET", format!("{:.2}", s.remaining()), "AU", 185.0))
                .child(post())
                .child(core)
                .child(spacer())
                .child(autopilot(s, accent))
                .child(engage(s, drive, accent)),
        )
}

/// Drops in when the ship settles into orbit.
fn banner(s: &State) -> El {
    let w = s.world();
    glass(row()).key(format!("banner-{}", s.here)).h(44.0).px(22.0).gap(11.0).center_y().radius(22.0)
        .transition(Transition::offset(0.0, -18.0).combined(Transition::blur(4.0)))
        .child(dot(w.accent, 7.0))
        .child(text("ORBIT ESTABLISHED").font(TextStyle::new(11.0, 600.0).tracking(1.8)).color(c(INK)))
        .child(text(w.name.to_uppercase()).font(TextStyle::new(11.0, 600.0).tracking(1.8)).color(c(w.accent)))
}

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let [w, h] = cx.layout_size("root").unwrap_or([1200.0, 750.0]);
    let world = s.world();
    let accent = world.accent;
    let drive = s.drive();
    let pose = s.pose();
    let parked = s.in_orbit();
    let all = |e: El| e.pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0));

    let mut root = col().key("root").w_full().h_full().fill(c(0x02030a)).child(all(fx(SPACE, [0.0; 4])).key("space"));
    if s.poster {
        return root;
    }

    // Radar rings off the world while the ship is parked, and the reticle around it. The rings
    // fade through their own strength: a slot that never fades as an element stays out of an
    // offscreen layer, and at strength 0 it draws nothing.
    let rings = match s.phase {
        Phase::Orbit { since } => ((s.now - since) / 0.9).clamp(0.0, 1.0),
        Phase::Spool { t0 } => 1.0 - ((s.now - t0) / 0.3).clamp(0.0, 1.0),
        _ => 0.0,
    } as f32;
    let r = ring_radius(s, h) * 1.2 + 26.0;
    root = root.child(
        fx(PULSE, [accent as f32, 60.0, 3.6, 0.32 * rings]).key("ping").absolute(pose.at[0] * w - r, pose.at[1] * h - r).size(2.0 * r, 2.0 * r).radius(r).effect_bleed(64.0)
            .instant_changes(),
    );
    root = root.child(all(canvas(reticle(s, w, h, accent))).key("reticle").inert().opacity(if parked { 1.0 } else { 0.0 }));

    let hud = col().key("hud").w_full().h_full().padding(EDGE).gap(16.0)
        .child(top_bar(s, accent))
        .child(row().w_full().grow(1.0).min_h(0.0).items_start().child(nav(s)))
        .child(console(s, &drive, accent));
    root = root.child(hud);

    // Keyed, as every child of the root is: the radar slot comes and goes ahead of them, and an
    // unkeyed sibling would be matched by position and remounted.
    let mut aside = col().key("aside").pin(None, Some(CARD_TOP), Some(EDGE), None);
    if !matches!(s.phase, Phase::Warp { .. }) {
        aside = aside.child(card(s));
    }
    // The banner slides in through a layer of its own; its sparkles sit over it and pop in
    // whole, so they draw straight into the window.
    let mut news = el().key("news");
    if s.banner() {
        let burst = [s.arrived.unwrap_or(s.now) as f32, 18.0, 9.0, accent as f32];
        news = news.child(banner(s)).child(all(fx(SPARKLES, burst)).key("sparkles").radius(22.0).effect_bleed(70.0).transition(Transition::none()));
    }
    let notice = row().key("notice").pin(Some(0.0), Some(86.0), Some(0.0), None).center().inert().child(news);

    // Last, so it bends everything above: a shockwave from the middle of the window at each
    // jump and each drop out of warp. Idle, it draws nothing.
    let shock = all(fx(SHOCK, [w * 0.5, h * 0.5, s.shock as f32, 22.0])).key("shock").effect_backdrop(true).inert();
    root.child(aside).child(notice).child(shock)
}

#[cfg(test)]
mod tests {
    use rune::{Env, Runtime};

    use super::*;
    use crate::voyage::START;

    fn runtime() -> Runtime<State> {
        let mut rt = Runtime::new(State::default(), view, (1200.0, 750.0), Env { system_dark: true, scale: 2.0 });
        rt.poll = Some(|s, t, _| {
            s.tick(t);
            true
        });
        rt.frame(0.0);
        rt
    }

    fn click(rt: &mut Runtime<State>, key: &str) {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key}"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
    }

    #[test]
    fn picking_a_planet_and_pressing_engage_starts_the_trip() {
        let mut rt = runtime();
        click(&mut rt, "world-5");
        assert_eq!(rt.state.selected, 5);
        click(&mut rt, "engage");
        assert!(matches!(rt.state.phase, Phase::Spool { .. }));
        assert_eq!(rt.state.route(), (START, 5));
    }

    #[test]
    fn the_list_is_locked_in_flight_and_the_card_leaves_in_the_tunnel() {
        let mut rt = runtime();
        click(&mut rt, "engage");
        click(&mut rt, "world-7");
        assert_eq!(rt.state.selected, START + 1, "a press in flight changes nothing");
        assert!(rt.find("card-2").is_some());
        for _ in 0..180 {
            rt.frame(1.0 / 60.0);
        }
        assert!(matches!(rt.state.phase, Phase::Warp { .. }));
        assert!(rt.find("card-2").is_none());
    }

    #[test]
    fn the_autopilot_switch_flies_to_the_next_planet_by_itself() {
        let mut rt = runtime();
        click(&mut rt, "autopilot");
        assert!(rt.state.autopilot);
        for _ in 0..(12 * 60) {
            rt.frame(1.0 / 60.0);
        }
        assert_eq!(rt.state.here, START + 1);
        assert!(rt.state.flown > 0.5);
    }

    #[test]
    fn the_icon_scene_draws_the_planet_alone() {
        let state = State { poster: true, ..State::default() };
        let mut rt = Runtime::new(state, view, (1024.0, 1024.0), Env { system_dark: true, scale: 1.0 });
        rt.frame(0.0);
        assert!(rt.find("space").is_some());
        assert!(rt.find("hud").is_none());
    }
}
