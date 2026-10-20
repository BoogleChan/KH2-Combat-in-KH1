//! KH2's player model (`player_model = kh2`, the Config menu's Player Model row): Sora drawn
//! with KH2's own model, his Drive Forms with their KH2 models, and his Keyblade with KH2's model
//! of the same Keyblade where KH2 has one (`docs/kh1-kh2-model.md`).
//!
//! - **Load** (a thread at start): `obj/P_EX100.mdlx` and the forms' (`_BTLF`, `_MAGF`, `_ULTF`)
//!   from the KH2 install (the same files `pose` reads), parsed by `kh2_formats::mdlx`; and which
//!   model each KH2 Keyblade has: `03system.bin`'s `item` (its weapon number) and `went` (base
//!   Sora's weapon objects by number), `00objentry.bin` (the object's model), named by
//!   `msg/us/sys.bar`. A Keyblade's model is loaded the first time KH1's Keyblade of the same
//!   name is held (another thread); until it is in, KH1's own is drawn.
//! - **Pose**: `pose` hands over each frame's KH2 pose (body and Keyblade, KH2 model space) with
//!   KH1's placement and the two skeletons' size ratio; `leftblade` the left Keyblade's (Valor,
//!   Final).
//! - **Hide**: KH1's model queue (`leftblade`'s hook on `0x1401D1F10`) asks `hide` for Sora's
//!   draw: every body mesh masked off (the draw params' mesh mask), and his Keyblade too when
//!   ours replaces it. KH1 still poses him (hitboxes, effects, the trail and clanks use those
//!   bones), only his draw is held back.
//! - **Draw**: from the world-model pass (`kh2fx`), the triangles skinned on the CPU and drawn by
//!   `fx12::draw_mesh` (depth-tested and depth-writing, cut out at half alpha, KH1's camera).
//!
//! When KH1 animates him (its own moves, KH1-style moves set in the Config menu, scripted walks),
//! `pose` carries KH1's pose onto KH2's skeleton (`kh1_data::inverse`) and hands that over
//! instead, with no Keyblade pose: KH1's own Keyblade is drawn in his hand then. Any frame
//! without a fresh pose (pause menus, a copy of Sora elsewhere) draws KH1's model as before.

use super::addr::Rva;
use super::form::Kind;
use super::log;
use glam::Mat4;
use kh2_formats::mdlx;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// The draw params: the weapon model, the hidden-mesh mask (`leftblade`'s).
const P_WEAPON: usize = 0x18;
const P_MASK: usize = 0x38;
/// The save (Sora's weapon at `+0x36`).
const SAVE_PTR_RVA: Rva = Rva(0x28695A0);
/// A pose older than this isn't drawn (KH1 has him).
const FRESH_S: f32 = 0.1;

/// One loaded model, its textures keyed for `fx12`'s model texture buffer.
struct Loaded {
    name: String,
    model: mdlx::Model,
    key: u64,
}

impl Loaded {
    fn new(name: &str, bytes: &[u8]) -> Result<Loaded, String> {
        let model = mdlx::parse(bytes).map_err(|e| format!("{name}: {e}"))?;
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut h);
        Ok(Loaded { name: name.to_string(), model, key: h.finish() & !0xFF })
    }
}

/// A Keyblade's KH2 model: loading, loaded, or none.
enum Weapon {
    Loading,
    Ready(Arc<Loaded>),
    Failed,
}

struct Library {
    install: kh2_data::Install,
    source: kh2_data::Source,
    /// Base Sora, then Valor, Wisdom, Final.
    bodies: [Option<Arc<Loaded>>; 4],
    /// KH2's Keyblades: name (lower case) → model name.
    keyblades: Vec<(String, String)>,
    /// By model name.
    weapons: HashMap<String, Weapon>,
}

static LIB: OnceLock<Result<Mutex<Library>, String>> = OnceLock::new();
static BASE: AtomicUsize = AtomicUsize::new(0);

/// This frame's pose, from `pose`.
struct Frame {
    body: Vec<Mat4>,
    weapon: Vec<Mat4>,
    to_world: Mat4,
    ratio: f32,
    at: Instant,
}

static POSE: Mutex<Option<Frame>> = Mutex::new(None);
/// KH2's left Keyblade (Valor, Final), from `leftblade`, and when.
static LEFT: Mutex<Option<(Vec<Mat4>, Instant)>> = Mutex::new(None);
/// Sora's draw was held back since the last world pass (ours is due), and what replaced his
/// Keyblade: bit 0 the right, bit 1 the left.
static HIDDEN: AtomicBool = AtomicBool::new(false);
static WEAPONS_OURS: AtomicU32 = AtomicU32::new(0);
/// KH1's left Keyblade's draw (`leftblade`) is ours this frame.
static LEFT_OURS: AtomicBool = AtomicBool::new(false);
static HIDES: AtomicU64 = AtomicU64::new(0);
static DRAWN: AtomicU64 = AtomicU64::new(0);

/// The Config row / ini choice.
pub fn on() -> bool {
    super::styles::kh2(super::styles::Kind::PlayerModel)
}

/// Starts loading the models (a thread).
pub fn load() {
    let base = unsafe { super::sys::GetModuleHandleA(std::ptr::null()) } as usize;
    BASE.store(base, Ordering::Relaxed);
    std::thread::spawn(|| {
        let t = Instant::now();
        let r = (|| -> Result<Library, String> {
            let exe = format!("{}\\KINGDOM HEARTS II FINAL MIX.exe", super::exe_dir());
            let install = kh2_data::Install::locate(&exe).map_err(|e| format!("KH2 install: {e}"))?;
            let source = if install.has_backup() { kh2_data::Source::Backup } else { kh2_data::Source::Live };
            let read = |name: &str| install.read_any(source, name).map_err(|e| format!("{name}: {e}"));
            let mut bodies: [Option<Arc<Loaded>>; 4] = [None, None, None, None];
            for (i, name) in ["P_EX100", "P_EX100_BTLF", "P_EX100_MAGF", "P_EX100_ULTF"].iter().enumerate() {
                match read(&format!("obj/{name}.mdlx")).and_then(|b| Loaded::new(name, &b)) {
                    Ok(l) => {
                        log(&format!(
                            "KH2 model: {name}: {} bones, {} triangles in {} groups, {} textures",
                            l.model.bones.len(),
                            l.model.triangle_count(),
                            l.model.groups.len(),
                            l.model.textures.len()
                        ));
                        bodies[i] = Some(Arc::new(l));
                    }
                    Err(e) => log(&format!("KH2 model: {e}")),
                }
            }
            if bodies[0].is_none() {
                return Err("base Sora's model did not load".into());
            }
            let keyblades = keyblade_models(&read).unwrap_or_else(|e| {
                log(&format!("KH2 model: Keyblade models not found ({e}); KH1's Keyblades kept"));
                Vec::new()
            });
            log(&format!("KH2 model: {} KH2 Keyblades named ({}), {source:?} files, in {:.1}s", keyblades.len(), keyblades.iter().map(|(n, m)| format!("{n} {m}")).collect::<Vec<_>>().join(", "), t.elapsed().as_secs_f32()));
            Ok(Library { install, source, bodies, keyblades, weapons: HashMap::new() })
        })();
        if let Err(e) = &r {
            log(&format!("KH2 model: off ({e})"));
        }
        let _ = LIB.set(r.map(Mutex::new));
    });
}

/// KH2's Keyblades by name: (name in lower case, model).
fn keyblade_models(read: &dyn Fn(&str) -> Result<Vec<u8>, String>) -> Result<Vec<(String, String)>, String> {
    let system = read("03system.bin")?;
    let items = kh2_formats::system::parse_items(&system).map_err(|e| e.to_string())?;
    let objects = kh2_formats::objentry::parse_objentry(&read("00objentry.bin")?).map_err(|e| e.to_string())?;
    let msg = kh2_formats::msg::Messages::parse(&read("msg/us/sys.bar")?).map_err(|e| e.to_string())?;
    let bar = kh2_formats::bar::Bar::parse(&system).map_err(|e| e.to_string())?;
    let went = bar.get("went").ok_or("no went table")?.data;
    let word = |i: usize| went.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    // Base Sora's weapon set (moveset 1): its list's offset (in words), then objects by number.
    let list = word(1).ok_or("went too short")? as usize;
    let mut out = Vec::new();
    for it in items.items.iter().filter(|i| i.kind == kh2_formats::system::item_type::KEYBLADE) {
        let Some(name) = msg.text(it.name as u32).filter(|n| !n.is_empty()) else { continue };
        let Some(obj) = word(list + it.number as usize).filter(|o| *o != 0) else { continue };
        let Some(model) = objects.iter().find(|o| o.id == obj).map(|o| o.model.clone()).filter(|m| m.starts_with("W_")) else { continue };
        if !out.iter().any(|(n, _): &(String, String)| *n == name.to_lowercase()) {
            out.push((name.to_lowercase(), model));
        }
    }
    Ok(out)
}

/// The body model for the form he is in (base Sora's if the form's didn't load).
fn body(lib: &Library) -> Option<Arc<Loaded>> {
    let i = match super::form::current() {
        Some(Kind::Valor) => 1,
        Some(Kind::Wisdom) => 2,
        Some(Kind::Final) => 3,
        None => 0,
    };
    lib.bodies[i].clone().or_else(|| lib.bodies[0].clone())
}

/// KH2's model of KH1's Keyblade `item`, if KH2 has one by that name and it is in (asked for
/// the first time: loaded on a thread).
unsafe fn weapon_model(lib: &mut Library, item: u8) -> Option<Arc<Loaded>> {
    static NAMES: Mutex<Vec<(u8, Option<String>)>> = Mutex::new(Vec::new());
    let base = BASE.load(Ordering::Relaxed);
    let model = {
        let mut names = NAMES.lock().ok()?;
        match names.iter().find(|(i, _)| *i == item) {
            Some((_, m)) => m.clone(),
            None => {
                let name = super::equip::item_name(base, item)?;
                let m = lib.keyblades.iter().find(|(n, _)| *n == name.to_lowercase()).map(|(_, m)| m.clone());
                log(&format!("KH2 model: KH1's {name} ({item:#x}): {}", m.as_deref().map_or("no KH2 Keyblade of that name, KH1's drawn".to_string(), |m| format!("KH2's {m}"))));
                names.push((item, m.clone()));
                m
            }
        }
    }?;
    match lib.weapons.get(&model) {
        Some(Weapon::Ready(l)) => return Some(l.clone()),
        Some(_) => return None,
        None => {}
    }
    lib.weapons.insert(model.clone(), Weapon::Loading);
    let (install, source) = (lib.install.clone(), lib.source);
    std::thread::spawn(move || {
        let r = install.read_any(source, &format!("obj/{model}.mdlx")).map_err(|e| format!("{model}: {e}")).and_then(|b| Loaded::new(&model, &b));
        let w = match r {
            Ok(l) => {
                log(&format!("KH2 model: {model}: {} bones, {} triangles, {} textures", l.model.bones.len(), l.model.triangle_count(), l.model.textures.len()));
                Weapon::Ready(Arc::new(l))
            }
            Err(e) => {
                log(&format!("KH2 model: {e}; KH1's Keyblade drawn"));
                Weapon::Failed
            }
        };
        if let Some(Ok(lib)) = LIB.get() {
            if let Ok(mut g) = lib.lock() {
                g.weapons.insert(model, w);
            }
        }
    });
    None
}

/// Sora's Keyblade (KH1 item id), if the save reads.
unsafe fn held_weapon(base: usize) -> Option<u8> {
    let save = std::ptr::read_unaligned((base + SAVE_PTR_RVA) as *const u64) as usize;
    (save >= 0x10000 && super::crash::readable(save + 0x36)).then(|| std::ptr::read_unaligned((save + 0x36) as *const u8))
}

/// The left Keyblade's KH1 item (Valor's, Final's), if he holds two.
unsafe fn left_weapon(base: usize) -> Option<u8> {
    match super::form::current()? {
        Kind::Final => Some(super::valor::FINAL_KEYBLADE),
        Kind::Valor => super::equip::valor_keyblade(base),
        Kind::Wisdom => None,
    }
}

/// `pose`, each frame KH2 poses him: the body and the right Keyblade (KH2 model space), KH1's
/// placement, and KH1's size against KH2's.
pub fn note_pose(body: &[Mat4], weapon: &[Mat4], to_world: &Mat4, ratio: f32) {
    if !on() {
        return;
    }
    if let Ok(mut g) = POSE.try_lock() {
        match g.as_mut() {
            Some(f) if f.body.len() == body.len() && f.weapon.len() == weapon.len() => {
                f.body.copy_from_slice(body);
                f.weapon.copy_from_slice(weapon);
                f.to_world = *to_world;
                f.ratio = ratio;
                f.at = Instant::now();
            }
            _ => *g = Some(Frame { body: body.to_vec(), weapon: weapon.to_vec(), to_world: *to_world, ratio, at: Instant::now() }),
        }
    }
}

/// `leftblade`, each KH2 step: KH2's left Keyblade (KH2 model space), if the form has one.
pub fn note_left(bones: Option<&[Mat4]>) {
    if !on() {
        return;
    }
    if let (Ok(mut g), Some(b)) = (LEFT.try_lock(), bones.filter(|b| !b.is_empty())) {
        *g = Some((b.to_vec(), Instant::now()));
    }
}

fn fresh_pose() -> bool {
    POSE.try_lock().ok().is_some_and(|g| g.as_ref().is_some_and(|f| f.at.elapsed().as_secs_f32() < FRESH_S))
}

/// Draw params (KH1's are 0x60 bytes; 16-aligned as KH1's own).
#[repr(C, align(16))]
pub struct Params(pub [u8; 0x100]);

/// KH1's model queue, for Sora's draw (`leftblade::queue`): the params to draw him with instead
/// (his body masked off, his Keyblade too if ours replaces it), or `None` to draw him as KH1 does.
pub unsafe fn hide(params: usize) -> Option<Params> {
    if !on() || !super::fx12::usable() || !fresh_pose() {
        return None;
    }
    let base = BASE.load(Ordering::Relaxed);
    if base == 0 || super::kh2fx::paused(base).is_some() || super::pose::menu_sora_within(150) {
        return None;
    }
    let lib = LIB.get()?.as_ref().ok()?;
    let mut lib = lib.try_lock().ok()?;
    body(&lib)?;
    let mut ours = 0u32;
    let has_weapon = std::ptr::read_unaligned((params + P_WEAPON) as *const u64) != 0;
    let weapon_pose = POSE.try_lock().ok().and_then(|g| g.as_ref().map(|f| !f.weapon.is_empty())).unwrap_or(false);
    if has_weapon && weapon_pose && held_weapon(base).and_then(|w| weapon_model(&mut lib, w)).is_some() {
        ours |= 1;
    }
    let left_fresh = LEFT.try_lock().ok().is_some_and(|g| g.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() < FRESH_S));
    if has_weapon && left_fresh && super::form::two_blades() && left_weapon(base).and_then(|w| weapon_model(&mut lib, w)).is_some() {
        ours |= 2;
    }
    let mut p = Params([0u8; 0x100]);
    std::ptr::copy_nonoverlapping(params as *const u8, p.0.as_mut_ptr(), 0x60);
    std::ptr::write_unaligned(p.0.as_mut_ptr().add(P_MASK) as *mut u32, 0xFFFF_FFFF);
    if ours & 1 != 0 {
        std::ptr::write_unaligned(p.0.as_mut_ptr().add(P_WEAPON) as *mut u64, 0);
    }
    WEAPONS_OURS.store(ours, Ordering::Relaxed);
    LEFT_OURS.store(ours & 2 != 0, Ordering::Relaxed);
    HIDDEN.store(true, Ordering::Relaxed);
    let n = HIDES.fetch_add(1, Ordering::Relaxed);
    if n < 3 || n % 3600 == 0 {
        log(&format!("KH2 model: KH1's Sora held back ({n} times; drawn {}; Keyblades ours {ours:#b})", DRAWN.load(Ordering::Relaxed)));
    }
    Some(p)
}

/// Whether KH1's left Keyblade (`leftblade::second`) gives way to ours this frame.
pub fn left_is_ours() -> bool {
    on() && HIDDEN.load(Ordering::Relaxed) && LEFT_OURS.load(Ordering::Relaxed)
}

/// The world pass (`kh2fx`): Sora drawn with KH2's model, if KH1's was held back.
pub fn draw() {
    if !HIDDEN.swap(false, Ordering::Relaxed) {
        return;
    }
    let ours = WEAPONS_OURS.load(Ordering::Relaxed);
    let base = BASE.load(Ordering::Relaxed);
    let Some(Ok(lib)) = LIB.get() else { return };
    let Ok(mut lib) = lib.try_lock() else { return };
    let Some(body_model) = body(&lib) else { return };
    let (right, left) = unsafe {
        (
            if ours & 1 != 0 { held_weapon(base).and_then(|w| weapon_model(&mut lib, w)) } else { None },
            if ours & 2 != 0 { left_weapon(base).and_then(|w| weapon_model(&mut lib, w)) } else { None },
        )
    };
    drop(lib);
    let Ok(pose) = POSE.lock() else { return };
    let Some(f) = pose.as_ref() else { return };
    let k = if f.ratio.is_finite() && f.ratio > 0.1 { f.ratio } else { 1.0 };
    let place = f.to_world * Mat4::from_scale(glam::Vec3::splat(k));
    let left_bones = LEFT.lock().ok().and_then(|g| g.as_ref().map(|(b, _)| b.clone()));
    let mut parts: Vec<(&Loaded, &[Mat4])> = vec![(&body_model, &f.body)];
    if let Some(w) = right.as_deref() {
        parts.push((w, &f.weapon));
    }
    if let (Some(w), Some(b)) = (left.as_deref(), left_bones.as_deref()) {
        parts.push((w, b));
    }
    let mut vertices: Vec<kh2_fx::Vertex> = Vec::with_capacity(12_000);
    let mut batches = Vec::new();
    let mut textures = Vec::new();
    for (l, bones) in &parts {
        let first_tex = textures.len();
        for (i, t) in l.model.textures.iter().enumerate() {
            textures.push(super::fx12::MeshTexture { key: l.key | i as u64, width: t.width, height: t.height, rgba: &t.rgba });
        }
        let start = vertices.len();
        let mut runs: Vec<(usize, usize)> = Vec::new(); // (group, first vertex)
        kh2_anim::skin::skin(&l.model, bones, place, |g, p, uv| {
            if runs.last().is_none_or(|r| r.0 != g) {
                runs.push((g, vertices.len()));
            }
            for k in 0..3 {
                vertices.push(kh2_fx::Vertex { pos: p[k].to_array(), uv: uv[k], color: [1.0; 4] });
            }
        });
        for (i, &(g, first)) in runs.iter().enumerate() {
            let end = runs.get(i + 1).map_or(vertices.len(), |r| r.1);
            let group = &l.model.groups[g];
            batches.push(super::fx12::MeshBatch { first: first as u32, count: (end - first) as u32, texture: first_tex + group.texture, translucent: group.translucent() });
        }
        let _ = start;
    }
    let Some((view, proj)) = (unsafe { super::fx12::camera(base) }) else { return };
    if unsafe { super::fx12::draw_mesh(base, &vertices, &batches, &textures, view, proj) } {
        let n = DRAWN.fetch_add(1, Ordering::Relaxed);
        if n < 3 {
            log(&format!("KH2 model: drew {} ({} triangles{})", body_model.name, vertices.len() / 3, parts.iter().skip(1).map(|(l, _)| format!(", {}", l.name)).collect::<String>()));
        }
    }
}
