//! Sora's clothes recoloured in a Drive Form (the user: Wisdom Form, "recolor KH1 Sora blue").
//!
//! The PC draws the remastered Sora (`docs/kh1-tint.md`): 9 meshes on 4 HD textures, the
//! jumpsuit, jacket, hood, zipper and crown all on the second (`-1c1.dds`, slot 1), and a strip
//! of red collar on the first (`-1c0.dds`, face, skin and hair). A shader tint per mesh can't
//! make only the red blue (the mesh shader scales and offsets each channel on its own), so the
//! texture itself is swapped: a copy of each with its red recoloured to blue (hue within ~12° of
//! red and saturated → hue 225°, same saturation and brightness; skin, the black and white
//! jacket, the blue trim stay), made once as a game texture (`0x1400F9F80`, KH1's own creator,
//! from the game thread), and written into the source texture's GPU-texture slot (`+0x70`) while
//! in the form, KH1's put back after. Everything that samples the source follows the pointer
//! (the pause menu's Sora too).
//!
//! The pixels: the HD texture file KH1 keeps in memory (`+0x68`, a DDS), else read from the PKG
//! (the model's remastered assets, `Kh1Install::read_remastered`), recoloured in the background.

use super::addr::Rva;
use super::{log, pose};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// The render state (per thread) and KH1's texture creator (`(R, &D3D12_RESOURCE_DESC, pixels,
/// name, 0)` → GPU texture object, synchronous).
const STATE_RVA: Rva = Rva(0xFD9C0);
const CREATE_RVA: Rva = Rva(0xF9F80);
/// `Axa::EntryTex`: its magic, the DDS in memory, the GPU texture, the size.
const TEX_MAGIC: u32 = 0x02F0_2EB5;
const TEX_DDS: usize = 0x68;
const TEX_GPU: usize = 0x70;
const TEX_W: usize = 0x7C;
const TEX_H: usize = 0x7E;
/// The slots recoloured (mesh table entries 0 and 1: `-1c0`, the collar; `-1c1`, the clothes).
const SLOTS: [usize; 2] = [0, 1];
const MODEL: &str = "xa_ex_0010.mdls";
/// Red → blue: the hue that is moved, how wide (degrees, soft edge after), the saturation it
/// takes from (soft edge after), and the hue it goes to.
const RED_WIDTH: f32 = 12.0;
const RED_SOFT: f32 = 6.0;
const SAT_FROM: f32 = 0.55;
const SAT_SOFT: f32 = 0.15;
const TO_HUE: f32 = 225.0;

static BASE: AtomicUsize = AtomicUsize::new(0);

/// A recoloured texture: width, height, B8G8R8A8 rows.
struct Pixels {
    w: u32,
    h: u32,
    bgra: Vec<u8>,
}

/// Each slot's recoloured pixels from the PKG (`None`: not read).
static FROM_PKG: [OnceLock<Option<Arc<Pixels>>>; 2] = [OnceLock::new(), OnceLock::new()];

struct Swap {
    /// Per slot: the source texture swapped (its address), KH1's GPU texture to put back.
    swapped: [Option<(usize, u64)>; 2],
    /// Our GPU texture for a source texture (made once per source).
    ours: Option<HashMap<usize, u64>>,
    /// Sources we couldn't make one for (not tried again).
    failed: Vec<usize>,
}

static SWAP: Mutex<Swap> = Mutex::new(Swap { swapped: [None, None], ours: None, failed: Vec::new() });

unsafe fn rd<T: Copy>(p: usize) -> T {
    std::ptr::read_unaligned(p as *const T)
}
unsafe fn wr<T: Copy>(p: usize, v: T) {
    std::ptr::write_unaligned(p as *mut T, v)
}

/// Red pixels to blue, in place (B8G8R8A8). Returns how many changed.
fn recolour(bgra: &mut [u8]) -> usize {
    let mut changed = 0;
    for px in bgra.chunks_exact_mut(4) {
        let (b, g, r) = (px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0);
        let mx = r.max(g).max(b);
        let mn = r.min(g).min(b);
        let c = mx - mn;
        if c <= 1e-6 || mx <= 0.0 {
            continue;
        }
        let h = if mx == r {
            ((g - b) / c).rem_euclid(6.0)
        } else if mx == g {
            (b - r) / c + 2.0
        } else {
            (r - g) / c + 4.0
        } * 60.0;
        let s = c / mx;
        let d = h.min(360.0 - h);
        let w = ((RED_WIDTH - d) / RED_SOFT).clamp(0.0, 1.0) * ((s - SAT_FROM) / SAT_SOFT).clamp(0.0, 1.0);
        if w <= 0.0 {
            continue;
        }
        changed += 1;
        // The same saturation and brightness at the new hue.
        let hh = TO_HUE / 60.0;
        let i = hh.floor() as i32 % 6;
        let f = hh - hh.floor();
        let v = mx;
        let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
        let (nr, ng, nb) = match i {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        let mix = |a: f32, z: f32| ((a * (1.0 - w) + z * w) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        px[0] = mix(b, nb);
        px[1] = mix(g, ng);
        px[2] = mix(r, nr);
    }
    changed
}

/// An uncompressed 32-bit DDS's pixels as B8G8R8A8 (whatever its masks), with its size.
fn dds_bgra(dds: &[u8]) -> Option<Pixels> {
    if dds.len() < 128 || &dds[..4] != b"DDS " {
        return None;
    }
    let u = |o: usize| u32::from_le_bytes(dds[o..o + 4].try_into().unwrap());
    let (h, w) = (u(12), u(16));
    let (flags, bits) = (u(80), u(88));
    let masks = [u(92), u(96), u(100), u(104)];
    // RGB (0x40), 32 bits, no four-cc (0x4).
    if flags & 0x40 == 0 || flags & 0x4 != 0 || bits != 32 || w == 0 || h == 0 {
        return None;
    }
    let n = (w * h) as usize;
    let data = dds.get(128..128 + n * 4)?;
    let chan = |v: u32, m: u32| if m == 0 { 255 } else { ((v & m) >> m.trailing_zeros()) as u8 };
    let mut bgra = Vec::with_capacity(n * 4);
    for px in data.chunks_exact(4) {
        let v = u32::from_le_bytes(px.try_into().unwrap());
        bgra.extend_from_slice(&[chan(v, masks[2]), chan(v, masks[1]), chan(v, masks[0]), chan(v, masks[3])]);
    }
    Some(Pixels { w, h, bgra })
}

fn recoloured(mut p: Pixels, what: &str) -> Arc<Pixels> {
    let n = recolour(&mut p.bgra);
    log(&format!("Wisdom tint: {what}, {}x{}, {n} texels recoloured ({:.1} %)", p.w, p.h, n as f32 * 100.0 / (p.w * p.h).max(1) as f32));
    Arc::new(p)
}

/// At start: the HD textures from the PKG, recoloured (for when KH1 no longer keeps the file in
/// memory).
pub fn install() {
    let base = unsafe { super::sys::GetModuleHandleA(std::ptr::null()) } as usize;
    BASE.store(base, Ordering::Relaxed);
    if !super::settings::get().drive {
        return;
    }
    std::thread::spawn(|| {
        let install = match kh1_data::Kh1Install::locate(super::exe_dir()) {
            Ok(i) => i,
            Err(e) => {
                log(&format!("Wisdom tint: KH1's files not found ({e})"));
                return;
            }
        };
        for (k, slot) in SLOTS.iter().enumerate() {
            let asset = format!("-1c{slot}.dds");
            let p = match install.read_remastered(kh1_data::Source::Live, MODEL, &asset) {
                Ok((name, bytes)) => match dds_bgra(&bytes) {
                    Some(p) => Some(recoloured(p, &format!("{MODEL} {name} (from the PKG)"))),
                    None => {
                        log(&format!("Wisdom tint: {name} isn't a plain 32-bit DDS"));
                        None
                    }
                },
                Err(e) => {
                    log(&format!("Wisdom tint: {MODEL} {asset} not read from the PKG ({e})"));
                    None
                }
            };
            let _ = FROM_PKG[k].set(p);
        }
    });
}

/// Sora's model's source texture for mesh-table entry `entry` (`docs/kh1-tint.md`), if it is an
/// HD texture.
unsafe fn source_texture(actor: usize, entry: usize) -> Option<usize> {
    let inst = pose::ptr(rd::<u32>(actor + 0x15C));
    if inst < 0x10000 || !super::crash::readable(inst + 8) {
        return None;
    }
    let slot = rd::<u64>(inst) as usize;
    if slot < 0x10000 || !super::crash::readable(slot + 8) {
        return None;
    }
    let header = rd::<u64>(slot) as usize;
    if header < 0x10000 || !super::crash::readable(header + 0x20) || rd::<u32>(header + 0x10) != 0x9696_9696 {
        return None;
    }
    // The remastered table behind `+0x14`: its count at `+4`, entries after a skip given by `+8`.
    let p = pose::ptr(rd::<u32>(header + 0x14));
    let t = if p > 0x10000 && super::crash::readable(p + 8) { rd::<u64>(p) as usize } else { 0 };
    if t < 0x10000 || !super::crash::readable(t + 0x10) {
        return None;
    }
    let w = rd::<u32>(t + 8);
    let skip = if w & 0x1_0000 != 0 { (w & 0xFFFF) as usize } else { 0 };
    let count = rd::<u32>(t + 4) as usize;
    if entry >= count || count > 32 {
        return None;
    }
    let e = t + 0x10 * (1 + skip * 2 + entry);
    if !super::crash::readable(e + 0x10) {
        return None;
    }
    // Entry `+4` → texture descriptor X; X `+8` → the source `Axa::EntryTex`.
    let x = pose::ptr(rd::<u32>(e + 4));
    if x < 0x10000 || !super::crash::readable(x + 0x10) {
        return None;
    }
    let src = pose::ptr(rd::<u32>(x + 8));
    if src < 0x10000 || !super::crash::readable(src + 0x98) || rd::<u32>(src + 0x10) != TEX_MAGIC {
        return None;
    }
    Some(src)
}

#[repr(C)]
struct Desc {
    dimension: u32,
    alignment: u64,
    width: u64,
    height: u32,
    depth: u16,
    mips: u16,
    format: u32,
    samples: u32,
    quality: u32,
    layout: u32,
    flags: u32,
}

/// Our recoloured GPU texture for `src` (made on first use).
unsafe fn ours_for(s: &mut Swap, k: usize, src: usize) -> Option<u64> {
    if let Some(&t) = s.ours.as_ref().and_then(|o| o.get(&src)) {
        return Some(t);
    }
    if s.failed.contains(&src) {
        return None;
    }
    let (w, h) = (rd::<u16>(src + TEX_W) as u32, rd::<u16>(src + TEX_H) as u32);
    let name: String = (0..32).map(|i| rd::<u8>(src + 0x14 + i)).take_while(|&b| b != 0).map(|b| b as char).collect();
    // The pixels: the DDS KH1 keeps (the same file), else the PKG's.
    let dds = rd::<u64>(src + TEX_DDS) as usize;
    let mem = if dds > 0x10000 && super::crash::readable(dds + 128) && rd::<u32>(dds) == 0x2053_4444 {
        let (dh, dw) = (rd::<u32>(dds + 12), rd::<u32>(dds + 16));
        let len = 128 + (dw as usize) * (dh as usize) * 4;
        (dw == w && dh == h && super::crash::readable(dds + len - 1))
            .then(|| dds_bgra(std::slice::from_raw_parts(dds as *const u8, len)))
            .flatten()
            .map(|p| recoloured(p, &format!("{name} (KH1's copy in memory)")))
    } else {
        None
    };
    let pixels = mem.or_else(|| FROM_PKG[k].get().cloned().flatten()).filter(|p| p.w == w && p.h == h);
    let Some(p) = pixels else {
        if FROM_PKG[k].get().is_some() {
            log(&format!("Wisdom tint: no pixels for {name} ({w}x{h}; neither KH1's copy nor the PKG's fits)"));
            s.failed.push(src);
        }
        return None;
    };
    let desc = Desc { dimension: 3, alignment: 0, width: w as u64, height: h, depth: 1, mips: 1, format: 0x57, samples: 1, quality: 0, layout: 0, flags: 0 };
    let base = BASE.load(Ordering::Relaxed);
    let state: unsafe extern "system" fn() -> usize = std::mem::transmute(base + STATE_RVA);
    let create: unsafe extern "system" fn(usize, *const Desc, *const u8, *const u8, u32) -> u64 = std::mem::transmute(base + CREATE_RVA);
    let r = state();
    if r < 0x10000 {
        return None;
    }
    let label = b"kh2combat_wisdom_blue\0";
    let t = create(r, &desc, p.bgra.as_ptr(), label.as_ptr(), 0);
    if t < 0x10000 {
        log(&format!("Wisdom tint: KH1 made no texture for {name} ({t:#x})"));
        s.failed.push(src);
        return None;
    }
    log(&format!("Wisdom tint: blue {name} ready ({w}x{h}, texture {t:#x}; KH1's {:#x})", rd::<u64>(src + TEX_GPU)));
    s.ours.get_or_insert_with(HashMap::new).insert(src, t);
    Some(t)
}

/// Each KH2 frame (`form`, the game thread): in the form, Sora's textures swapped for the blue
/// copies; out of it, KH1's back.
pub unsafe fn refresh(actor: usize, on: bool) {
    if BASE.load(Ordering::Relaxed) == 0 || !super::settings::get().drive {
        return;
    }
    let Ok(mut s) = SWAP.try_lock() else { return };
    for (k, &entry) in SLOTS.iter().enumerate() {
        let src = source_texture(actor, entry);
        match (on, s.swapped[k], src) {
            (true, None, Some(src)) => {
                if let Some(t) = ours_for(&mut s, k, src) {
                    let orig = rd::<u64>(src + TEX_GPU);
                    wr(src + TEX_GPU, t);
                    s.swapped[k] = Some((src, orig));
                }
            }
            (true, Some((was, _)), Some(src)) if was != src => {
                // The model was loaded again (a new source, KH1's own texture in it); the old
                // one, ours with it, is KH1's to free.
                if let Some(o) = s.ours.as_mut() {
                    o.remove(&was);
                }
                s.swapped[k] = None;
            }
            (false, Some((was, orig)), _) => {
                let ours = s.ours.as_ref().and_then(|o| o.get(&was)).copied().unwrap_or(0);
                if src == Some(was) && rd::<u64>(was + TEX_GPU) == ours {
                    wr(was + TEX_GPU, orig);
                }
                s.swapped[k] = None;
            }
            _ => {}
        }
    }
    static N: AtomicU32 = AtomicU32::new(0);
    if on && s.swapped.iter().all(|x| x.is_none()) && N.fetch_add(1, Ordering::Relaxed) == 600 {
        log("Wisdom tint: still nothing swapped after 10 s in the form");
    }
}
