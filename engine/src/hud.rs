//! Skate 3's HUD: the retail trick display (`trickdisplay`, an APT movie run by `skate3_sim::hud_runtime`) as the
//! contract's [`HudFrame`]. Read from the player's prepared HUD (`private/hud`: the movie as JSON, its shapes' and
//! fonts' textures as raw RGBA), advanced with the scoring each tick, and drawn (`apt_scene::draw`) into one part:
//! the trick display's corner of the 1280x720 stage ([`AREA`]), shown on a panel by the player's chosen controller.
//!
//! Its draws are Flash-style: a texture times a colour transform ([`Shading::Transform`]), over what's under it.
use engine_core::glam::Vec2;
use engine_core::hud::{HudBlend, HudDraw, HudFrame, HudPart, HudVertex, Placement, Rgba8, Shading, TextureId};
use skate3_sim::{apt_scene, hud_runtime};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The movie's stage, its units.
pub const STAGE: [f32; 2] = [1280.0, 720.0];
/// The part of the stage shown (x, y from its top left, width, height): the trick display's corner, bottom left (its
/// tricks, the multiplier, the score and the line's, with room above for a long line's tricks and to the right for
/// long names).
pub const AREA: [f32; 4] = [40.0, 360.0, 640.0, 360.0];
/// How many pixels a stage unit is drawn at (the part's resolution).
pub const PIXELS_PER_UNIT: f32 = 1.5;
/// The panel's width, real metres.
pub const WIDTH_M: f32 = 0.2;
/// Where the panel's bottom left is from the grip, real metres (right, up): clear of the controller's head (the trick
/// display is at the bottom of its picture).
pub const OFFSET: Vec2 = Vec2::new(-0.055, 0.08);

/// Where the player's prepared HUD is: `SKATE_SCORING_HUD_ROOT`, or `private/hud` in the game's assets.
pub fn root(asset_root: &Path) -> PathBuf {
    std::env::var_os("SKATE_SCORING_HUD_ROOT").map(PathBuf::from).unwrap_or_else(|| asset_root.join("private/hud"))
}

/// The trick display: its movie running, its shapes, and its textures (by id: their index).
pub struct Hud {
    runtime: hud_runtime::Runtime,
    shapes: apt_scene::Shapes,
    /// Each texture's file, its id its index.
    ids: BTreeMap<String, TextureId>,
    /// Their pixels, until fetched ([`Hud::texture`]).
    textures: Vec<Option<Rgba8>>,
    /// It stopped (an error advancing or drawing it): it shows nothing from then on.
    failed: bool,
    /// Advanced since it was last drawn (or not drawn yet).
    changed: bool,
    /// The multiplier's circle (its clip, `multiplier_mc`): how it is when shown (its first frame, `all`: each depth's
    /// colour), and whether it is held so ([`Hud::set_circle_always`]).
    circle: Option<Circle>,
    circle_always: bool,
}

/// The multiplier's circle: its clip's character, and each depth's colour when it is shown.
struct Circle {
    character: i32,
    shown: BTreeMap<i32, [u8; 8]>,
}

/// The clip placed as `multiplier_mc`, and its first frame's colours.
fn circle(movie: &skate3_sim::apt_movie::Movie) -> Option<Circle> {
    let character = movie
        .characters
        .values()
        .flat_map(|c| c.frames.iter().flat_map(|f| f.controls.iter()))
        .find(|c| c.name.as_deref() == Some(CIRCLE))
        .and_then(|c| c.character_id)?;
    let first = movie.characters.get(&character)?.frames.first()?;
    let shown = first.controls.iter().filter_map(|c| Some((c.depth, c.color_transform?))).collect();
    Some(Circle { character, shown })
}

/// The multiplier's clip, its circle and timer: shown while a line runs, faded out (`clear`, `clearlong`) once it ends.
pub const CIRCLE: &str = "multiplier_mc";

impl Hud {
    /// Reads the HUD from `root` (all but the GPU's part, so it can be done off the main thread: it takes seconds),
    /// its movie started with the scoring's `input`.
    pub fn load(root: &Path, input: hud_runtime::Input) -> Result<Hud, String> {
        let source: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("runtime/trickdisplay.json")).map_err(|e| format!("{}: {e}", root.display()))?,
        )
        .map_err(|e| e.to_string())?;
        let runtime = hud_runtime::Runtime::load(&source, input)?;
        let shapes: apt_scene::Shapes = serde_json::from_value(source["shapes"].clone()).map_err(|e| e.to_string())?;
        let mut files = BTreeMap::new();
        for shape in shapes.values().flatten() {
            files.insert(shape.texture.rgba.clone(), [shape.texture.width, shape.texture.height]);
        }
        for font in runtime.bindings.movie.text_assets.fonts.values() {
            files.insert(font.texture.clone(), font.size);
        }
        let mut ids = BTreeMap::new();
        let mut textures = Vec::new();
        for (path, size) in files {
            let pixels = std::fs::read(root.join(&path)).map_err(|e| format!("HUD {path}: {e}"))?;
            if pixels.len() != size[0] as usize * size[1] as usize * 4 {
                return Err(format!("Invalid HUD texture size {path}"));
            }
            ids.insert(path, textures.len() as TextureId);
            textures.push(Some(Rgba8 { width: size[0], height: size[1], pixels }));
        }
        let circle = circle(&runtime.bindings.movie);
        Ok(Hud { runtime, shapes, ids, textures, failed: false, changed: true, circle, circle_always: false })
    }

    /// The multiplier's circle always shown (`true`), or fading in and out with the line as retail has it. Its
    /// animations still run (the timer, the multiplier's values); only its fades are held at full.
    pub fn set_circle_always(&mut self, always: bool) {
        if self.circle_always != always {
            self.circle_always = always;
            self.changed = true;
            self.hold_circle();
        }
    }

    /// The movie drawn. Retail hides its whole screen once a line has ended and the display is idle; with the circle
    /// always shown, the circle is drawn then on its own (the screen shown and its other parts hidden for the draw, and
    /// put back after).
    fn draws(&mut self) -> Result<Vec<apt_scene::Draw>, String> {
        use skate3_sim::apt_vm::Value;
        let root = self.runtime.bindings.movie.root;
        let alone = match &self.circle {
            Some(circle) if self.circle_always && !self.runtime.vm.get(root, "_visible").truth() => self.circle_top(circle.character),
            _ => None,
        };
        let Some(top) = alone else {
            return apt_scene::draw(&self.runtime.bindings.movie, &self.runtime.vm, &self.shapes);
        };
        let others: Vec<(usize, bool)> = self.runtime.bindings.movie.instances[&root]
            .children
            .values()
            .filter(|&&c| c != top)
            .map(|&c| (c, self.runtime.vm.get(c, "_visible").truth()))
            .collect();
        let vm = &mut self.runtime.vm;
        vm.set(root, "_visible", Value::Bool(true))?;
        for (c, _) in &others {
            vm.set(*c, "_visible", Value::Bool(false))?;
        }
        let draws = apt_scene::draw(&self.runtime.bindings.movie, &self.runtime.vm, &self.shapes);
        let vm = &mut self.runtime.vm;
        for (c, visible) in others {
            vm.set(c, "_visible", Value::Bool(visible))?;
        }
        vm.set(root, "_visible", Value::Bool(false))?;
        draws
    }

    /// The root's child the circle is under (or is).
    fn circle_top(&self, character: i32) -> Option<usize> {
        let movie = &self.runtime.bindings.movie;
        let mut at = movie.instances.iter().find(|(_, i)| i.character == character).map(|(id, _)| *id)?;
        loop {
            let parent = movie.instances.iter().find(|(_, i)| i.children.values().any(|c| *c == at)).map(|(p, _)| *p)?;
            if parent == movie.root {
                return Some(at);
            }
            at = parent;
        }
    }

    /// How opaque the circle is drawn (its draws' alpha, composed through every clip above it: 0 when it isn't drawn).
    #[cfg(test)]
    pub(crate) fn circle_alpha(&mut self) -> f32 {
        use std::collections::BTreeSet;
        let Some(circle) = &self.circle else { return 0.0 };
        let movie = &self.runtime.bindings.movie;
        // (its clip and every character placed under it, and the textures only their shapes use)
        let mut under = BTreeSet::new();
        let mut todo = vec![circle.character];
        while let Some(id) = todo.pop() {
            if !under.insert(id) {
                continue;
            }
            for control in movie.characters.get(&id).into_iter().flat_map(|c| c.frames.iter().flat_map(|f| f.controls.iter())) {
                todo.extend(control.character_id.filter(|&c| c >= 0));
            }
        }
        let textures = |inside: bool| -> BTreeSet<String> {
            self.shapes
                .iter()
                .filter(|(id, _)| under.contains(*id) == inside)
                .flat_map(|(_, shapes)| shapes.iter().map(|s| s.texture.rgba.clone()))
                .collect()
        };
        let others = textures(false);
        let own: BTreeSet<String> = textures(true).difference(&others).cloned().collect();
        assert!(!own.is_empty(), "the circle has textures of its own");
        let draws = self.draws().expect("drawn");
        draws.iter().filter(|d| own.contains(&d.texture)).map(|d| d.multiply[3]).fold(0.0, f32::max)
    }

    /// The circle's parts at their shown colours, where its timeline has put them (each frame sets them again).
    fn hold_circle(&mut self) {
        let (Some(circle), true) = (&self.circle, self.circle_always) else { return };
        let movie = &mut self.runtime.bindings.movie;
        let clips: Vec<usize> = movie.instances.iter().filter(|(_, i)| i.character == circle.character).map(|(id, _)| *id).collect();
        for clip in clips {
            let children: Vec<(i32, usize)> = movie.instances[&clip].children.iter().map(|(d, c)| (*d, *c)).collect();
            for (depth, child) in children {
                if let (Some(color), Some(placement)) =
                    (circle.shown.get(&depth), movie.instances.get_mut(&child).and_then(|i| i.placement.as_mut()))
                {
                    placement.color = *color;
                }
            }
        }
    }

    /// A tick of the scoring: the movie advanced with its `input` and what happened (a new trick, the trick modified,
    /// the line's tricks closed). An error stops it (and is returned once).
    pub fn advance(&mut self, input: hud_runtime::Input, new: bool, modified: bool, close: bool) -> Result<(), String> {
        if self.failed {
            return Ok(());
        }
        self.changed = true;
        self.runtime.update(input, new, modified, close).inspect_err(|_| self.failed = true)?;
        self.hold_circle();
        Ok(())
    }

    /// The trick display as it is now, into `out` (its buffers kept), if it has advanced since it was last drawn:
    /// whether it has. An error drawing it stops it (and is returned once; the frame is then empty).
    pub fn frame(&mut self, out: &mut HudFrame) -> Result<bool, String> {
        if !std::mem::replace(&mut self.changed, false) {
            return Ok(false);
        }
        out.clear();
        out.canvas = Vec2::from_array(STAGE);
        out.parts.push(part());
        if self.failed {
            return Ok(true);
        }
        let draws = match self.draws() {
            Ok(draws) => draws,
            Err(error) => {
                self.failed = true;
                return Err(error);
            }
        };
        for draw in draws {
            let Some(&texture) = self.ids.get(&draw.texture) else { continue };
            let first = out.vertices.len() as u32;
            out.vertices.extend(draw.vertices.iter().map(|v| HudVertex {
                position: Vec2::from_array(v.position),
                uv: Vec2::from_array(v.uv),
                color: [255; 4],
            }));
            out.draws.push(HudDraw {
                first,
                count: draw.vertices.len() as u32,
                texture: Some(texture),
                part: 0,
                blend: HudBlend::Alpha,
                shading: Shading::Transform { multiply: draw.multiply, add: draw.add },
            });
        }
        Ok(true)
    }

    /// A texture's pixels, handed over once (none: not one of its ids, or fetched already).
    pub fn texture(&mut self, id: TextureId) -> Option<Rgba8> {
        self.textures.get_mut(id as usize)?.take()
    }
}

/// The one part: the trick display's corner of the stage, on a panel by the controller (its bottom left a little left
/// of the grip and up).
pub fn part() -> HudPart {
    let [x, y, width, height] = AREA;
    HudPart {
        name: "trick display".into(),
        size: Vec2::new(width, height),
        view: Vec2::new(x + width / 2.0, y + height / 2.0),
        resolution: PIXELS_PER_UNIT,
        empty: false,
        placement: Placement::Hand {
            metres_per_px: WIDTH_M / width,
            offset: OFFSET,
            anchor: Vec2::new(-0.5, -0.5),
            hand: None,
            on_left: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trick display's part: its corner of the stage at its pixels a unit, a panel `WIDTH_M` wide by the hand.
    #[test]
    fn the_part_is_the_trick_display_corner_by_the_hand() {
        let part = part();
        assert_eq!(part.size, Vec2::new(640.0, 360.0));
        assert_eq!(part.view, Vec2::new(360.0, 540.0));
        assert_eq!(part.resolution, 1.5);
        let Placement::Hand { metres_per_px, offset, anchor, hand, on_left } = part.placement else { panic!() };
        assert!((metres_per_px * part.size.x - WIDTH_M).abs() < 1e-6);
        assert_eq!((offset, anchor, hand, on_left), (OFFSET, Vec2::new(-0.5, -0.5), None, None));
    }

    fn input() -> hud_runtime::Input {
        hud_runtime::Input {
            sequence_score: 0,
            line_score: 0,
            sequence_timer: 0,
            line_time: 0.0,
            line_capacity: 0.0,
            multiplier: 1.0,
            clean: false,
            sketchy: false,
            stance: [false; 4],
            trick_name: String::new(),
            trick_metrics: Default::default(),
            context_tricks: Vec::new(),
        }
    }

    /// The real trick display (`SKATE3_ASSET_ROOT`; skipped without it): its circle faded out (`cleared`, the end of
    /// `clear`) is invisible as retail has it, and shown with the option, at its first frame's colours.
    #[test]
    fn the_circle_can_stay_shown() {
        let Some(assets) = std::env::var_os("SKATE3_ASSET_ROOT") else {
            eprintln!("SKATE3_ASSET_ROOT not set: skipped");
            return;
        };
        let mut hud = Hud::load(&root(Path::new(&assets)), input()).expect("the HUD");
        let character = hud.circle.as_ref().expect("its circle").character;
        let alphas = |hud: &Hud| -> Vec<u8> {
            let movie = &hud.runtime.bindings.movie;
            let clip = movie.instances.iter().find(|(_, i)| i.character == character).map(|(id, _)| *id).expect("placed");
            movie.instances[&clip].children.values().filter_map(|c| movie.instances.get(c)?.placement.as_ref().map(|p| p.color[0])).collect()
        };
        let cleared = |hud: &mut Hud| {
            let movie = &mut hud.runtime.bindings.movie;
            let clip = movie.instances.iter().find(|(_, i)| i.character == character).map(|(id, _)| *id).unwrap();
            let frame = movie.characters[&character].frames.len().min(43) - 1;
            movie.seek(&mut hud.runtime.vm, clip, frame, 0).expect("seek");
        };
        cleared(&mut hud);
        assert!(alphas(&hud).iter().all(|&a| a == 0), "retail's: faded out {:?}", alphas(&hud));
        hud.set_circle_always(true);
        let shown: Vec<u8> = hud.circle.as_ref().unwrap().shown.values().map(|c| c[0]).collect();
        assert!(shown.iter().filter(|&&a| a == 255).count() >= 3, "{shown:?}");
        assert!(alphas(&hud).iter().filter(|&&a| a == 255).count() >= 3, "held shown {:?}", alphas(&hud));
        // (and through a tick of the movie, which sets them again)
        cleared(&mut hud);
        hud.advance(input(), false, false, false).expect("a tick");
        assert!(alphas(&hud).iter().filter(|&&a| a == 255).count() >= 3, "after a tick {:?}", alphas(&hud));
        let mut frame = HudFrame::default();
        assert!(hud.frame(&mut frame).expect("drawn"));
    }

    /// Without its files it says where it looked.
    #[test]
    fn a_missing_hud_says_where() {
        let root = Path::new("/nonexistent/hud");
        let input = hud_runtime::Input {
            sequence_score: 0,
            line_score: 0,
            sequence_timer: 0,
            line_time: 0.0,
            line_capacity: 0.0,
            multiplier: 1.0,
            clean: false,
            sketchy: false,
            stance: [false; 4],
            trick_name: String::new(),
            trick_metrics: Default::default(),
            context_tricks: Vec::new(),
        };
        let error = Hud::load(root, input).err().expect("no HUD there");
        assert!(error.contains("/nonexistent/hud"), "{error}");
    }
}
