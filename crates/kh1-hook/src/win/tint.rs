//! Sora's clothes recoloured in a Drive Form (the user: Wisdom Form, "recolor KH1 Sora blue").
//!
//! KH1 draws each of a model's meshes through `0x1401DB7F0` (its packet list in rcx), after
//! setting the mesh shader's constants: Ambient (a colour the texture is multiplied by, 1 =
//! as is), FogParam (`.w` the fog factor f) and FogColor; the pixel is
//! `tex × Ambient × f + FogColor × (1 − f)` (`docs/kh1-tint.md`). Sora's model (`xa_ex_0010`)
//! splits his clothes into their own meshes by texture: 3 and 4 the jacket, 5 the jumpsuit,
//! 6 and 7 the pockets, belt and chain. So: hooked, a draw of one of those meshes (their packet
//! lists, read from his model's mesh table, `refresh`) runs with our Ambient and fog over KH1's
//! (KH1's own fog folded in), and KH1's are put back after it. Per channel only (no channel
//! mixing in the shader): the red jumpsuit, which has next to no blue or green in its texture,
//! becomes blue mostly through the fog term.

use super::addr::Rva;
use super::{hook, log, pose};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

const DRAW_RVA: Rva = Rva(0x1DB7F0);
const DRAW_PROLOGUE: [u8; 15] = [0x4C, 0x8B, 0xDC, 0x49, 0x89, 0x5B, 0x20, 0x55, 0x56, 0x41, 0x54, 0x41, 0x56, 0x41, 0x57];
/// The render state (per thread) and its constant setter.
const STATE_RVA: Rva = Rva(0xFD9C0);
const SET_RVA: Rva = Rva(0x107770);
const AMBIENT: usize = 0x5BB8;
const FOG_PARAM: usize = 0x5BC8;
const FOG_COLOUR: usize = 0x5B34;
const SET_AMBIENT: i32 = 0x1001;
const SET_FOG_PARAM: i32 = 0x1002;
const SET_FOG_COLOUR: i32 = 0x2000;

/// Per piece: Ambient's multiplier, the fog factor, the fog colour (0..1).
struct Look {
    mul: [f32; 3],
    f: f32,
    fog: [f32; 3],
}

/// Wisdom Form's blue: the jacket deep blue (white → light blue, black stays dark), the
/// jumpsuit blue, the trim cooled a little.
#[allow(dead_code)]
const JACKET: Look = Look { mul: [0.55, 0.7, 1.25], f: 0.85, fog: [0.05, 0.10, 0.40] };
#[allow(dead_code)]
const SUIT: Look = Look { mul: [0.2, 0.2, 0.2], f: 0.55, fog: [0.10, 0.25, 1.0] };
#[allow(dead_code)]
const TRIM: Look = Look { mul: [0.75, 0.85, 1.15], f: 1.0, fog: [0.0, 0.0, 0.0] };

/// A piece by its mesh. The PC draws a remastered model, not the PS2 one the notes were taken
/// from (9 meshes on 4 textures, other sizes: the log's table), so which mesh is which isn't
/// known yet: for now each mesh gets its own plain colour (`PROBE`), for a screenshot to tell
/// them apart (s193).
fn look(mesh: usize) -> Option<&'static Look> {
    PROBE.get(mesh)
}

/// One strong colour per mesh: 0 red, 1 green, 2 blue, 3 yellow, 4 magenta, 5 cyan, 6 white,
/// 7 black, 8 orange.
const PROBE: [Look; 9] = [
    Look { mul: [1.0, 1.0, 1.0], f: 0.35, fog: [1.0, 0.0, 0.0] },
    Look { mul: [1.0, 1.0, 1.0], f: 0.35, fog: [0.0, 1.0, 0.0] },
    Look { mul: [1.0, 1.0, 1.0], f: 0.35, fog: [0.0, 0.0, 1.0] },
    Look { mul: [1.0, 1.0, 1.0], f: 0.35, fog: [1.0, 1.0, 0.0] },
    Look { mul: [1.0, 1.0, 1.0], f: 0.35, fog: [1.0, 0.0, 1.0] },
    Look { mul: [1.0, 1.0, 1.0], f: 0.35, fog: [0.0, 1.0, 1.0] },
    Look { mul: [1.0, 1.0, 1.0], f: 0.35, fog: [1.0, 1.0, 1.0] },
    Look { mul: [1.0, 1.0, 1.0], f: 0.35, fog: [0.0, 0.0, 0.0] },
    Look { mul: [1.0, 1.0, 1.0], f: 0.35, fog: [1.0, 0.5, 0.0] },
];

const MESHES: usize = 32;
static PACKETS: [AtomicUsize; MESHES] = [const { AtomicUsize::new(0) }; MESHES];
/// Each mesh's texture index (which piece it is, `look`).
static LOOKS: [AtomicU32; MESHES] = [const { AtomicU32::new(u32::MAX) }; MESHES];
static HEADER: AtomicUsize = AtomicUsize::new(0);
/// How much of the look (0..1, f32 bits), stepped toward the form's.
static AMOUNT: AtomicU32 = AtomicU32::new(0);
static DRAWN: AtomicU32 = AtomicU32::new(0);
static ORIG: AtomicUsize = AtomicUsize::new(0);
static BASE: AtomicUsize = AtomicUsize::new(0);
/// Frames for the look to come in / go.
const FADE_FRAMES: f32 = 15.0;

unsafe fn rd<T: Copy>(p: usize) -> T {
    std::ptr::read_unaligned(p as *const T)
}

/// Each KH2 frame (`form`): Sora's mesh packet lists (his model's, read again if it changed),
/// and the look stepped toward `on`.
pub unsafe fn refresh(actor: usize, on: bool) {
    if ORIG.load(Ordering::Relaxed) == 0 {
        return;
    }
    let k = f32::from_bits(AMOUNT.load(Ordering::Relaxed));
    let k = if on { (k + 1.0 / FADE_FRAMES).min(1.0) } else { (k - 1.0 / FADE_FRAMES).max(0.0) };
    AMOUNT.store(k.to_bits(), Ordering::Relaxed);
    if k <= 0.0 && !on {
        return;
    }
    let inst = pose::ptr(rd::<u32>(actor + 0x15C));
    if inst < 0x10000 || !super::crash::readable(inst + 8) {
        return;
    }
    let slot = rd::<u64>(inst) as usize;
    if slot < 0x10000 || !super::crash::readable(slot + 8) {
        return;
    }
    let header = rd::<u64>(slot) as usize;
    if header < 0x10000 || !super::crash::readable(header + 0x20) {
        return;
    }
    // (Read every frame: a room's reload can put the model back where it was with new lists.)
    let changed = header != HEADER.load(Ordering::Relaxed);
    HEADER.store(header, Ordering::Relaxed);
    // The mesh table (`0x1401DB4F0` / `0x1401DB570`): the header's own entries from `+0x10`, or,
    // when `+0x10` reads 0x96969696 (the PC's models), a table behind the handle at `+0x14`:
    // its count at `+4`, entries after a skip given by `+8`.
    let (first, count) = if rd::<u32>(header + 0x10) == 0x9696_9696 {
        let p = pose::ptr(rd::<u32>(header + 0x14));
        let t = if p > 0x10000 && super::crash::readable(p + 8) { rd::<u64>(p) as usize } else { 0 };
        if t < 0x10000 || !super::crash::readable(t + 0x10) {
            return;
        }
        let w = rd::<u32>(t + 8);
        let skip = if w & 0x1_0000 != 0 { (w & 0xFFFF) as usize } else { 0 };
        (t + 0x10 * (1 + skip * 2), rd::<u32>(t + 4) as usize)
    } else {
        (header + 0x10, rd::<u32>(header + 0xC) as usize)
    };
    if count == 0 || count > MESHES || !super::crash::readable(first + 0x10 * count) {
        if changed {
            log(&format!("Wisdom tint: Sora's model {header:#x} has {count} meshes: not tinted"));
        }
        for p in &PACKETS {
            p.store(0, Ordering::Relaxed);
        }
        return;
    }
    let mut got = Vec::new();
    for (i, p) in PACKETS.iter().enumerate() {
        if i >= count {
            p.store(0, Ordering::Relaxed);
            continue;
        }
        let e = first + 0x10 * i;
        let packets = pose::ptr(rd::<u32>(e + 0xC));
        // Which piece: by the mesh's texture (entry `+4`; jacket 3, jumpsuit 4, trim 5).
        let tex = rd::<u32>(e + 4);
        p.store(packets, Ordering::Relaxed);
        LOOKS[i].store(tex, Ordering::Relaxed);
        got.push(format!("{i}: {:08x} tex {tex} {:08x} -> {packets:#x}", rd::<u32>(e), rd::<u32>(e + 8)));
    }
    if changed {
        log(&format!("Wisdom tint: Sora's model {header:#x}, {count} meshes: {}", got.join("; ")));
    }
}

unsafe extern "system" fn draw(a: usize, b: usize, c: usize, d: usize) -> u64 {
    let orig: unsafe extern "system" fn(usize, usize, usize, usize) -> u64 = std::mem::transmute(ORIG.load(Ordering::Acquire));
    let k = f32::from_bits(AMOUNT.load(Ordering::Relaxed));
    if k <= 0.0 || a == 0 {
        return orig(a, b, c, d);
    }
    let Some(i) = PACKETS.iter().position(|p| p.load(Ordering::Relaxed) == a) else {
        return orig(a, b, c, d);
    };
    let Some(l) = look(i) else {
        return orig(a, b, c, d);
    };
    // (Seen drawn at all: once per mesh, for the log.)
    let bit = 1u32 << i;
    if DRAWN.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
        log(&format!("Wisdom tint: mesh {i} drawn tinted"));
    }
    let base = BASE.load(Ordering::Relaxed);
    let state: unsafe extern "system" fn() -> usize = std::mem::transmute(base + STATE_RVA);
    let set: unsafe extern "system" fn(i32, *const f32, i32) = std::mem::transmute(base + SET_RVA);
    let r = state();
    if r < 0x10000 {
        return orig(a, b, c, d);
    }
    let amb = rd::<[f32; 4]>(r + AMBIENT);
    let param = rd::<[f32; 4]>(r + FOG_PARAM);
    let colour = rd::<[f32; 3]>(r + FOG_COLOUR);
    // Ours under KH1's fog: (tex·A·fk + Fo·(1−fk))·fs + Fs·(1−fs), one fog term again.
    let mut new_amb = amb;
    for i in 0..3 {
        new_amb[i] = amb[i] * (1.0 + (l.mul[i] - 1.0) * k);
    }
    let fs = param[3].clamp(0.0, 1.0);
    let fk = 1.0 + (l.f - 1.0) * k;
    let f = fk * fs;
    let mut new_param = param;
    new_param[3] = f;
    let mut new_colour = colour;
    if 1.0 - f > 1e-4 {
        for i in 0..3 {
            new_colour[i] = (l.fog[i] * (1.0 - fk) * fs + colour[i] * (1.0 - fs)) / (1.0 - f);
        }
    }
    set(SET_AMBIENT, new_amb.as_ptr(), 0);
    set(SET_FOG_PARAM, new_param.as_ptr(), 0);
    set(SET_FOG_COLOUR, new_colour.as_ptr(), 0);
    let ret = orig(a, b, c, d);
    set(SET_AMBIENT, amb.as_ptr(), 0);
    set(SET_FOG_PARAM, param.as_ptr(), 0);
    set(SET_FOG_COLOUR, colour.as_ptr(), 0);
    ret
}

pub unsafe fn install() {
    let base = super::sys::GetModuleHandleA(std::ptr::null()) as usize;
    BASE.store(base, Ordering::Relaxed);
    if !super::settings::get().drive {
        return;
    }
    if hook::install(base + DRAW_RVA, &DRAW_PROLOGUE, draw as *const () as usize, &ORIG, "mesh draw (Wisdom tint)").is_none() {
        log("Wisdom tint: the mesh draw wasn't as expected: Sora keeps his colours in the form");
    }
}
