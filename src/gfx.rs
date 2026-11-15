//! The compositor. Rune lays out and draws the HUD; at each `El::effect` slot this paints what
//! the slot's mark asks for, in painter order: the space scene from `space.wgsl`, or one
//! rune-fx layer (glow, lightning, radar rings, sparkles, a shockwave that bends everything
//! already drawn under it).

use std::collections::HashMap;

use rune::compose::{Compositor, DeviceKey, Frame};
use rune::render::{EffectPhase, EffectSite, KIND_EFFECT};
use rune_fx::*;

use crate::voyage::State;
use crate::worlds::{SUNLIGHT, World};

/// The scene behind everything.
pub const SPACE: u32 = 1;
/// Data: style (0 soft, 1 neon, 2 aura), colour, intensity, radius.
pub const GLOW: u32 = 2;
/// Data: intensity, amplitude, calm (1) or arcing (0), colour.
pub const ELECTRIC: u32 = 3;
/// Data: centre x, y in points, start time, strength.
pub const SHOCK: u32 = 4;
/// Data: colour, reach in points, period, intensity.
pub const PULSE: u32 = 5;
/// Data: burst time, count, size, colour.
pub const SPARKLES: u32 = 6;

/// `space.wgsl`'s uniform block, field for field.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Scene {
    view: [f32; 4],
    rect: [f32; 4],
    drive: [f32; 4],
    body: [f32; 4],
    surf: [f32; 4],
    c0: [f32; 4],
    c1: [f32; 4],
    c2: [f32; 4],
    atmo: [f32; 4],
    neb_a: [f32; 4],
    neb_b: [f32; 4],
    sun: [f32; 4],
    sun_col: [f32; 4],
    more: [f32; 4],
}

/// A hex colour as the shader wants it: display-referred, with `a` in the spare channel.
fn tone(hex: u32, a: f32) -> [f32; 4] {
    [((hex >> 16) & 0xff) as f32 / 255.0, ((hex >> 8) & 0xff) as f32 / 255.0, (hex & 0xff) as f32 / 255.0, a]
}

/// The scene for this state, in a slot at `rect` (window device px) drawn at `scale` px a point.
fn scene(s: &State, rect: [f32; 4], scale: f32, window: (u32, u32)) -> Scene {
    let w: &World = s.world();
    let drive = s.drive();
    let pose = s.pose();
    // The icon gets a deeper sky and a bigger Sun than the planet's own.
    let (nebula, (sun_at, sun_size)) = if s.poster { ((0x2438b8, 0x8a2aa0, 1.25), ([-0.27, -0.29], 0.022)) } else { (w.nebula, w.sun) };
    Scene {
        view: [window.0 as f32, window.1 as f32, s.now as f32, scale],
        rect,
        drive: [drive.warp, drive.flash, drive.shake, drive.travel],
        body: [rect[0] + pose.at[0] * rect[2], rect[1] + pose.at[1] * rect[3], pose.radius * rect[3], w.kind],
        surf: w.surface,
        c0: tone(w.colors[0], w.seed),
        c1: tone(w.colors[1], w.spin),
        c2: tone(w.colors[2], pose.seen),
        atmo: tone(w.atmosphere.0, w.atmosphere.1),
        neb_a: tone(nebula.0, w.seed),
        neb_b: tone(nebula.1, nebula.2),
        sun: [sun_at[0], sun_at[1], sun_size, 0.0],
        sun_col: tone(SUNLIGHT, 1.0),
        more: [w.craters, if s.poster { 0.0 } else { 1.0 }, w.haze, 0.0],
    }
}

/// The space shader's pipeline, one per target format it has been asked to draw into.
struct Space {
    module: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
}

impl Space {
    fn new(device: &wgpu::Device) -> Space {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("space"), source: wgpu::ShaderSource::Wgsl(include_str!("space.wgsl").into()) });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("space"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("space"), bind_group_layouts: &[Some(&bgl)], immediate_size: 0 });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("space"),
            size: std::mem::size_of::<Scene>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("space"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }],
        });
        Space { module, layout, uniforms, bind, pipelines: HashMap::new() }
    }

    fn pipeline(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        let (module, layout) = (&self.module, &self.layout);
        self.pipelines.entry(format).or_insert_with(|| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("space"),
                layout: Some(layout),
                vertex: wgpu::VertexState { module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        })
    }

    /// Paints the scene into the slot. The hook has the scissor on the slot already; the
    /// viewport is put over the whole window so the shader sees window coordinates even inside
    /// a layer, and Rune resets it afterwards.
    fn draw(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, pass: &mut wgpu::RenderPass<'_>, site: &EffectSite<'_>, mut scene: Scene, window: (u32, u32)) {
        scene.sun_col[3] = if site.srgb_blend { 1.0 } else { 0.0 };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&scene));
        pass.set_viewport(-site.origin[0], -site.origin[1], window.0 as f32, window.1 as f32, 0.0, 1.0);
        pass.set_pipeline(self.pipeline(device, site.format));
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// A colour that rode in a slot's data as its 24-bit hex.
fn hue(v: f32) -> Rgb {
    rgb(v.max(0.0) as u32)
}

/// The rune-fx layer a slot's mark and data describe, on the slot's own shape.
fn layer(mark: u32, d: [f32; 4], shape: Shape, t: f32, reach: f32) -> Option<FxLayer> {
    let fx = match mark {
        GLOW => {
            let style = match d[0] as u32 {
                1 => GlowStyle::Neon,
                2 => GlowStyle::Aura,
                _ => GlowStyle::Soft,
            };
            Fx::glow(GlowParams { style, color: hue(d[1]), third: rgb(0x9b7bff), intensity: d[2], radius: d[3], aura_speed: 0.3, ..Default::default() })
        }
        ELECTRIC => {
            let base = if d[2] > 0.5 { ElectricParams::charged() } else { ElectricParams::default() };
            Fx::electric(ElectricParams { intensity: d[0], amplitude: d[1], color: hue(d[3]), ..base })
        }
        SHOCK => Fx::distort(DistortParams::Shockwave { at: [d[0], d[1]], reach, width: 110.0, amount: d[3], chroma: 0.7, t0: d[2], duration: 1.25 }),
        PULSE => Fx::pulse(PulseParams { style: PulseStyle::Radar, color: hue(d[0]), reach: d[1], period: d[2], intensity: d[3], count: 3, width: 1.2, ..Default::default() }),
        SPARKLES => Fx::sparkles(SparkleParams { burst: Some(d[0]), count: d[1] as u32, size: d[2], color: hue(d[3]), margin: 30.0, ..Default::default() }),
        _ => return None,
    };
    Some(fx.on(shape).at(t))
}

#[derive(Default)]
pub struct Gfx {
    /// The device `space` and `fx` were made on.
    device: Option<DeviceKey>,
    space: Option<Space>,
    fx: Option<FxRenderer>,
}

impl Compositor<State> for Gfx {
    /// Space never holds still.
    fn animating(&self) -> bool {
        true
    }

    fn render(&mut self, f: &mut Frame<'_, State>) {
        if self.device != Some(f.device_key()) {
            self.device = Some(f.device_key());
            self.space = None;
            self.fx = None;
        }
        let (device, queue, size) = (f.device, f.queue, f.size);
        let now = f.rt.state.now as f32;
        let reach = f.rt.viewport.0.hypot(f.rt.viewport.1) * 0.8;

        // Every slot in the painter stream: where it is, and the layer it asks for.
        let mut picture = None;
        let mut slots: Vec<u32> = Vec::new();
        let mut layers: Vec<FxLayer> = Vec::new();
        let mut placements: Vec<FxSlot> = Vec::new();
        for (n, i) in f.rt.instances.iter().enumerate() {
            if i.params[1] != KIND_EFFECT {
                continue;
            }
            let mark = f.rt.marks.get(n).copied().unwrap_or(0);
            let k = i.params[3];
            if mark == SPACE {
                picture = Some(scene(&f.rt.state, i.rect, k, size));
                continue;
            }
            let [x, y, w, h] = i.rect.map(|v| v / k);
            let r = i.radii.iter().fold(0.0f32, |a, v| a.max(v.abs())) / k;
            let Some(l) = layer(mark, i.uv, Shape::rounded(x, y, w, h, r), now, reach) else { continue };
            slots.push(n as u32);
            layers.push(l);
            placements.push(FxSlot::default().scale(k).clip(i.clip, i.fade));
        }

        if self.fx.is_none() {
            // Preparing a layer builds its effect's pipeline. On the first frame prepare one of
            // each kind, live (no slot draws them), so the first jump doesn't stall on a compile.
            let probe = Shape::rounded(0.0, 0.0, 64.0, 64.0, 8.0);
            let kinds = [(SHOCK, [32.0, 32.0, now - 0.2, 8.0]), (SPARKLES, [now - 0.1, 6.0, 8.0, 0.0]), (ELECTRIC, [1.0, 4.0, 0.0, 0.0]), (PULSE, [0.0, 30.0, 2.0, 1.0]), (GLOW, [1.0, 0.0, 1.0, 14.0])];
            layers.extend(kinds.into_iter().filter_map(|(mark, data)| layer(mark, data, probe, now, reach)));
        }
        let space = self.space.get_or_insert_with(|| Space::new(device));
        let fx = self.fx.get_or_insert_with(|| FxRenderer::new(device, f.format, FxOptions { dither: true }));
        fx.set_srgb_blend(f.srgb_blend());
        fx.add_format(device, wgpu::TextureFormat::Rgba16Float);
        fx.prepare_with_slots(device, queue, &FxFrame { viewport: [size.0, size.1], scale: 1.0, layers: &layers, backdrop: None, sources: &[] }, &placements);

        let clear = f.rt.clear_color();
        f.ui.prepare(device, queue, &mut f.rt.text.atlas, &f.rt.instances, size);
        f.ui.draw_with_backdrop(f.encoder, f.target, clear, &mut |phase, site: &EffectSite<'_>| {
            if site.mark == SPACE {
                if let (EffectPhase::Draw(pass), Some(picture)) = (phase, picture) {
                    space.draw(device, queue, pass, site, picture, size);
                }
                return;
            }
            let Some(k) = slots.iter().position(|n| *n == site.index) else { return };
            match phase {
                EffectPhase::Backdrop(encoder) => {
                    if let Some(backdrop) = site.backdrop {
                        fx.blur_backdrop(device, queue, encoder, backdrop, k);
                    }
                }
                EffectPhase::Draw(pass) => fx.draw_layer_in(pass, site.format, site.origin, k),
            }
        });
    }
}
