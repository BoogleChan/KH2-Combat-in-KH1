//! KH2's motions as new slots in KH1 Sora's own motion table (ini `kh1_slots = on`; the user:
//! "lets have it so that our kh2 animations can run entirely through kh1s animation system as new
//! animation slots"). `docs/kh1-anim-slots.md` has the RE and the plan; this is its phase 2.
//!
//! KH1 plays a request through two tables on Sora's actor: the request table (`+0x1D8`, the
//! mset's section 1: request id → slot and kind) and the slot table (`+0x1D4`: slot → the
//! motion's handle). Both are handles, and `0x14038B080` makes a handle of any pointer, so Sora
//! gets copies of both: the slot table with our motions after KH1's 118, the request table with
//! the KH1 requests a KH2 move stands in for pointed at its slot while that move's style is KH2.
//! KH1's state, its attack row and attack object (tied to the request id, `+0x164`), its combo
//! windows, scripts and our own checks of `+0x164` all see KH1's request; only the motion played
//! is ours.
//!
//! Each of our motions is a KH1 motion (the slot the KH2 move stands in for, as the template)
//! retimed to the KH2 motion's length (`kh1_data::slot::retimed`), with the KH2 clip's hit,
//! combo and trail windows as KH1's motion events: KH1's clock, queue, blend clock, hit window,
//! combo window, trail and its own sounds and foot effects run it. `pose` draws the KH2 motion
//! over it at KH1's time (`actor + 0x16C`), and KH2's own sounds and effects for it fire from
//! here on that time (`tick`).
//!
//! The tables are put on Sora's actor again whenever it is a new one (each zone makes one) and
//! shows KH1's own tables (its 118 slots, its request table as in the file); never over another
//! motion set (a world's own, `+0x1E0`).

use super::addr::Rva;
use super::styles::Kind;
use super::{log, pose};
use character_core::{EventKind, WindowKind};
use kh1_data::slot::{self as ks, EventTrack};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const TO_HANDLE_RVA: Rva = Rva(0x38B080);
/// KH1 Sora's own slots (`xa_ex_0010.mset`).
pub const KH1_SLOTS: usize = 118;
/// KH1's request ids (`0x141` in use, the table 0x160 long); ours for KH2-only moves would
/// start at `0x200` (none yet).
const KH1_REQUESTS: usize = 0x141;
const REQUESTS: usize = 0x220;
const SLOT_TABLE: usize = 0x1D4;
const REQUEST_TABLE: usize = 0x1D8;
const SLOT_NOW: usize = 0x168;
const TIME_NOW: usize = 0x16C;
const BLEND_LEN: usize = 0x170;
const BLEND_CLOCK: usize = 0x174;

/// A KH2 motion played as a KH1 slot.
struct Spec {
    motion: u16,
    /// The KH1 slot it stands in for (its template) and KH1's request for that slot.
    stands_for: usize,
    request: usize,
    kind: Kind,
}

/// KH1's ground combo, KH2's swings for it as `pose` draws them (`pose::swing_motion`): KH1's
/// first and third swing (98, request 0xC8) KH2's first (151); its second (99, 0xC9) KH2's second
/// (152; the user: "sora base attack 2, kh2 base attack 2"); slot 100 (0xCA, taken as another
/// swing) KH2's first; its finisher (101, 0xCB) KH2's finisher (154; "kh1 base finisher, kh2 base
/// finishers").
///
/// KH1's air combo (slots 102..104, requests 0xCC..0xCE), KH2's aerial swings for it as `pose`
/// draws them: 181 for KH1's first air swing (102), 182 for its second (103), 183 for its finisher
/// (104), KH2's own order in the air (181 / 182 alternate, 183 finishes). With these built, the Attack in the air at Aerial Base Combo KH2 stays in KH1's air
/// state (`covers`, `shared::kh1_combo`) instead of borrowing him for KH2's own air combo.
const SPECS: [Spec; 7] = [
    Spec { motion: 152, stands_for: 99, request: 0xC9, kind: Kind::GroundCombo },
    Spec { motion: 151, stands_for: 98, request: 0xC8, kind: Kind::GroundCombo },
    Spec { motion: 151, stands_for: 100, request: 0xCA, kind: Kind::GroundCombo },
    Spec { motion: 154, stands_for: 101, request: 0xCB, kind: Kind::GroundCombo },
    Spec { motion: 181, stands_for: 102, request: 0xCC, kind: Kind::AirCombo },
    Spec { motion: 182, stands_for: 103, request: 0xCD, kind: Kind::AirCombo },
    Spec { motion: 183, stands_for: 104, request: 0xCE, kind: Kind::AirCombo },
];

pub struct Move {
    pub motion: u16,
    pub stands_for: usize,
    pub request: usize,
    pub kind: Kind,
    /// Its slot in KH1's table.
    pub slot: usize,
    pub frames: u32,
    /// The KH2 clip's sounds, voices and effects, by frame.
    events: Vec<(u32, EventKind)>,
}

struct Built {
    moves: Vec<Move>,
    /// Our motions (file layout), one after another, 16-aligned.
    blocks: Vec<Vec<u8>>,
    /// KH1's request table as in the file (to know KH1's own when the actor shows it).
    requests: Vec<u32>,
}

static BUILT: OnceLock<Option<Built>> = OnceLock::new();

/// Builds our motions (the loader thread, `pose::load_kh2`): `mset` is KH1 Sora's file.
pub fn prepare(mset: &[u8], kh2: &pose::Kh2) {
    let Some(which) = super::settings::get().kh1_slots.clone() else {
        let _ = BUILT.set(None);
        return;
    };
    let r = (|| -> Result<Built, String> {
        let requests = ks::request_table(mset).map_err(|e| e.to_string())?;
        if requests.len() < KH1_REQUESTS {
            return Err(format!("KH1's request table has {} entries", requests.len()));
        }
        let mut moves = Vec::new();
        let mut blocks = Vec::new();
        for s in SPECS.iter().filter(|s| which.as_ref().map_or(true, |w| w.contains(&s.motion))) {
            let Some(clip) = kh2.character.moveset.clips.get(&s.motion) else {
                log(&format!("KH2 slots: KH2 motion {} has no clip, left out", s.motion));
                continue;
            };
            if requests.get(s.request).map(|e| (e & 0xFFF) as usize) != Some(s.stands_for) {
                // (Left out, not a failure of the rest: one wrong request shouldn't drop the others.)
                log(&format!("KH2 slots: request {:#x} is not KH1's slot {}; KH2 motion {} left out", s.request, s.stands_for, s.motion));
                continue;
            }
            let template = ks::motion_bytes(mset, s.stands_for).map_err(|e| e.to_string())?;
            let old = u32::from_le_bytes(template[4..8].try_into().unwrap());
            let frames = clip.length.max(1);
            let tracks = kh1_tracks(&ks::events(&template).map_err(|e| e.to_string())?, clip, frames as f32 / old.max(1) as f32);
            let block = ks::retimed(&template, frames, &tracks).map_err(|e| e.to_string())?;
            let slot = KH1_SLOTS + moves.len();
            log(&format!(
                "KH2 slots: KH2 motion {} as KH1 slot {slot} (for KH1's slot {} / request {:#x}, {old} -> {frames} frames): {}",
                s.motion,
                s.stands_for,
                s.request,
                tracks.iter().map(|t| format!("{}.{} {:?}", t.kind, t.id, t.keys.iter().map(|k| (k.time, k.a)).collect::<Vec<_>>())).collect::<Vec<_>>().join("; ")
            ));
            let events = clip.events.iter().filter(|e| matches!(e.kind, EventKind::Sound(_) | EventKind::Voice(_) | EventKind::Effect(_))).map(|e| (e.frame, e.kind)).collect();
            moves.push(Move { motion: s.motion, stands_for: s.stands_for, request: s.request, kind: s.kind, slot, frames, events });
            blocks.push(block);
        }
        Ok(Built { moves, blocks, requests })
    })();
    match r {
        Ok(b) => {
            let _ = BUILT.set(Some(b));
        }
        Err(e) => {
            log(&format!("KH2 slots: not built ({e}); KH2's moves are drawn as before"));
            let _ = BUILT.set(None);
        }
    }
}

/// KH1's events for a KH2 clip on a retimed template (`r`: new length / the template's): the
/// clip's hit windows as KH1's hit window (attack group 0, as KH1's own combo motions), its combo
/// window, its trail (else its hit windows from 4 frames before); the template's sounds, foot
/// effects, face changes and the point its action may be left, rescaled.
fn kh1_tracks(template: &[EventTrack], clip: &character_core::Clip, r: f32) -> Vec<EventTrack> {
    let len = clip.length as f32;
    let span = |w: &character_core::Window| (w.start as f32, w.end.map_or(len, |e| e as f32));
    let hits: Vec<(f32, f32)> = clip.windows.iter().filter(|w| matches!(w.kind, WindowKind::Hitbox { .. })).map(span).collect();
    let combo: Vec<(f32, f32)> = clip.windows.iter().filter(|w| matches!(w.kind, WindowKind::Combo)).map(span).collect();
    let mut trail: Vec<(f32, f32)> = clip.windows.iter().filter(|w| matches!(w.kind, WindowKind::WeaponTrail)).map(span).collect();
    if trail.is_empty() {
        trail = hits.iter().map(|&(a, b)| ((a - 4.0).max(0.0), b)).collect();
    }
    let mut out: Vec<EventTrack> = ks::scaled(template, r).into_iter().filter(|t| matches!(t.kind, ks::FOOT | ks::SOUND | ks::FACE | ks::CAN_LEAVE)).collect();
    for (kind, w) in [(ks::HIT, hits), (ks::COMBO, combo), (ks::TRAIL, trail)] {
        if !w.is_empty() {
            out.push(EventTrack::level(kind, 0, &w));
        }
    }
    out
}

/// Our motions placed in memory (the game thread), and Sora's table copies.
struct Placed {
    /// Each motion's address (ours, leaked) and its handle.
    motions: Vec<(usize, u32)>,
    slot_table: &'static mut [u32],
    slot_handle: u32,
    request_table: &'static mut [u32],
    request_handle: u32,
    /// The actor they were put on last.
    actor: usize,
}

static PLACED: Mutex<Option<Placed>> = Mutex::new(None);
/// Our motions' addresses, for `pose::slot_of` (slot by the instance's motion pointer).
static MOTION_AT: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());
/// Our slot playing on Sora (`usize::MAX` none), and its KH2 motion (for `kh2fx::tick`).
static PLAYING: AtomicUsize = AtomicUsize::new(usize::MAX);
static PLAYING_MOTION: AtomicU32 = AtomicU32::new(u32::MAX);

fn built() -> Option<&'static Built> {
    BUILT.get()?.as_ref()
}

fn moved(slot: usize) -> Option<&'static Move> {
    built()?.moves.iter().find(|m| m.slot == slot)
}

/// Every slot of this kind of move is built (so KH1's state can own the whole move at KH2's
/// style, not just part of its combo).
pub fn covers(kind: Kind) -> bool {
    let Some(b) = built() else { return false };
    SPECS.iter().any(|s| s.kind == kind) && SPECS.iter().filter(|s| s.kind == kind).all(|s| b.moves.iter().any(|m| m.request == s.request))
}

/// Our slot's KH2 motion, if `slot` is one of ours.
pub fn kh2_motion(slot: usize) -> Option<u16> {
    moved(slot).map(|m| m.motion)
}

/// The KH1 slot ours stands in for (callers that test KH1's slots see that one).
pub fn stands_for(slot: usize) -> Option<usize> {
    moved(slot).map(|m| m.stands_for)
}

/// Our slot playing on Sora now, if any.
pub fn playing() -> Option<usize> {
    let s = PLAYING.load(Ordering::Relaxed);
    (s != usize::MAX).then_some(s)
}

/// The length of what plays for KH1's `slot` now, if ours stands in for it.
pub fn frames_for(slot: usize) -> Option<u32> {
    let m = moved(playing()?)?;
    (m.stands_for == slot || m.slot == slot).then_some(m.frames)
}

/// The KH2 motion of our slot playing, for `kh2fx::tick` while KH1 has him.
pub fn fx_motion() -> Option<u16> {
    let m = PLAYING_MOTION.load(Ordering::Relaxed);
    (m != u32::MAX).then_some(m as u16)
}

/// The slot of a motion of ours (the instance's motion pointer).
pub fn slot_of_motion(motion: usize) -> Option<usize> {
    MOTION_AT.try_lock().ok()?.iter().find(|(p, _)| *p == motion).map(|(_, s)| *s)
}

/// KH1's blend into the motion playing: its weight so far, while it blends.
pub unsafe fn kh1_blend(actor: usize) -> Option<f32> {
    let (len, clock) = (rd::<f32>(actor + BLEND_LEN), rd::<f32>(actor + BLEND_CLOCK));
    (len > 1.0 && clock.is_finite() && clock < len).then(|| (clock / len).clamp(0.0, 1.0))
}

/// KH1's time in our slot playing on `actor` (frames), if it is `slot`.
pub unsafe fn kh1_time(actor: usize, slot: usize) -> Option<f32> {
    (rd::<u32>(actor + SLOT_NOW) as usize == slot).then(|| rd::<f32>(actor + TIME_NOW)).filter(|t| t.is_finite() && *t >= 0.0)
}

unsafe fn rd<T: Copy>(p: usize) -> T {
    std::ptr::read_unaligned(p as *const T)
}

unsafe fn wr<T: Copy>(p: usize, v: T) {
    std::ptr::write_unaligned(p as *mut T, v)
}

unsafe fn handle(p: usize) -> u32 {
    let f: unsafe extern "system" fn(usize) -> u32 = std::mem::transmute(super::takeover::game_base() + TO_HANDLE_RVA);
    f(p)
}

/// Our motions in memory, their offsets turned into handles as KH1's loader does (`0x1401D5CE0`).
unsafe fn place(b: &Built) -> Placed {
    let mut motions = Vec::new();
    let mut at = Vec::new();
    for (m, block) in b.moves.iter().zip(&b.blocks) {
        // (16-aligned, never freed.)
        let words: &'static mut [u128] = Box::leak(vec![0u128; block.len().div_ceil(16)].into_boxed_slice());
        let p = words.as_mut_ptr() as usize;
        std::ptr::copy_nonoverlapping(block.as_ptr(), p as *mut u8, block.len());
        // Event groups' key offsets first (the group list is found by +0x3C's offset).
        let mut g = p + rd::<u32>(p + ks::EVENTS) as usize;
        while rd::<u32>(g) != ks::END {
            wr(g + 8, handle(p + rd::<u32>(g + 8) as usize));
            g += 12;
        }
        for f in ks::OFFSET_FIELDS {
            wr(p + f, handle(p + rd::<u32>(p + f) as usize));
        }
        motions.push((p, handle(p)));
        at.push((p, m.slot));
    }
    if let Ok(mut v) = MOTION_AT.lock() {
        *v = at;
    }
    let slot_table: &'static mut [u32] = Box::leak(vec![0u32; KH1_SLOTS + motions.len() + 1].into_boxed_slice());
    let request_table: &'static mut [u32] = Box::leak(vec![0xFFFu32; REQUESTS].into_boxed_slice());
    let slot_handle = handle(slot_table.as_ptr() as usize);
    let request_handle = handle(request_table.as_ptr() as usize);
    log(&format!("KH2 slots: {} motions placed (handles {:08x?}), tables {slot_handle:08x} / {request_handle:08x}", motions.len(), motions.iter().map(|m| m.1).collect::<Vec<_>>()));
    Placed { motions, slot_table, slot_handle, request_table, request_handle, actor: 0 }
}

/// KH1's own tables on `actor` (its 118 slots, its request table as in the file).
unsafe fn kh1_own(b: &Built, actor: usize) -> Option<(usize, usize)> {
    let (s, r) = (pose::ptr(rd::<u32>(actor + SLOT_TABLE)), pose::ptr(rd::<u32>(actor + REQUEST_TABLE)));
    let n = b.requests.len().min(REQUESTS);
    if s < 0x10000 || r < 0x10000 || !super::crash::readable(s) || !super::crash::readable(s + (KH1_SLOTS + 1) * 4 - 1) || !super::crash::readable(r) || !super::crash::readable(r + n * 4 - 1) {
        return None;
    }
    let slots_ok = (0..KH1_SLOTS).all(|i| rd::<u32>(s + i * 4) != 0) && rd::<u32>(s + KH1_SLOTS * 4) == 0;
    let requests_ok = (0..n).all(|i| rd::<u32>(r + i * 4) == b.requests[i]);
    (slots_ok && requests_ok).then_some((s, r))
}

/// Each frame Sora is posed (`pose`, the game thread): our tables on his actor, the requests
/// pointed at our slots by their moves' styles, and our slot's KH2 sounds and effects.
pub unsafe fn frame(actor: usize) {
    let Some(b) = built() else { return };
    if actor < 0x10000 || !super::crash::readable(actor + 0x1E0) {
        return;
    }
    let Ok(mut g) = PLACED.try_lock() else { return };
    let p = g.get_or_insert_with(|| place(b));
    let ours = rd::<u32>(actor + SLOT_TABLE) == p.slot_handle && rd::<u32>(actor + REQUEST_TABLE) == p.request_handle;
    if !ours {
        let Some((s, r)) = kh1_own(b, actor) else {
            static N: AtomicU32 = AtomicU32::new(0);
            if N.fetch_add(1, Ordering::Relaxed) < 5 {
                log(&format!("KH2 slots: Sora's actor {actor:#x} shows another motion set; left to KH1"));
            }
            PLAYING.store(usize::MAX, Ordering::Relaxed);
            PLAYING_MOTION.store(u32::MAX, Ordering::Relaxed);
            return;
        };
        for i in 0..KH1_SLOTS {
            p.slot_table[i] = rd::<u32>(s + i * 4);
        }
        for (i, m) in p.motions.iter().enumerate() {
            p.slot_table[KH1_SLOTS + i] = m.1;
        }
        *p.slot_table.last_mut().unwrap() = 0;
        for i in 0..b.requests.len().min(REQUESTS) {
            p.request_table[i] = rd::<u32>(r + i * 4);
        }
        wr(actor + SLOT_TABLE, p.slot_handle);
        wr(actor + REQUEST_TABLE, p.request_handle);
        if p.actor != actor {
            log(&format!("KH2 slots: on Sora's actor {actor:#x} ({} slots, {} of ours)", KH1_SLOTS + p.motions.len(), p.motions.len()));
        }
        p.actor = actor;
    }
    // The requests by style (base Sora under shared states; a change applies from KH1's next
    // request, never to the motion playing).
    for m in &b.moves {
        let kh1 = b.requests[m.request];
        let on = super::shared::on() && super::styles::kh2(m.kind);
        p.request_table[m.request] = if on { (kh1 & !0xFFF) | m.slot as u32 } else { kh1 };
    }
    drop(g);
    tick(actor);
}

/// Our slot's KH2 sounds, voices and effects between KH1's last time and its time now.
unsafe fn tick(actor: usize) {
    static LAST: Mutex<Option<(usize, f32)>> = Mutex::new(None);
    let slot = rd::<u32>(actor + SLOT_NOW) as usize;
    let Some(m) = moved(slot) else {
        PLAYING.store(usize::MAX, Ordering::Relaxed);
        PLAYING_MOTION.store(u32::MAX, Ordering::Relaxed);
        if let Ok(mut l) = LAST.try_lock() {
            *l = None;
        }
        return;
    };
    PLAYING.store(slot, Ordering::Relaxed);
    PLAYING_MOTION.store(m.motion as u32, Ordering::Relaxed);
    let now: f32 = rd(actor + TIME_NOW);
    let Ok(mut l) = LAST.try_lock() else { return };
    let from = match *l {
        Some((s, t)) if s == slot && t <= now => t,
        _ => -1.0,
    };
    if from == -1.0 {
        static N: AtomicU32 = AtomicU32::new(0);
        if N.fetch_add(1, Ordering::Relaxed) < 40 {
            log(&format!("KH2 slots: KH1 plays slot {slot} (KH2 motion {}, for KH1's {} / request {:#x}; Sora's request {:#x})", m.motion, m.stands_for, m.request, rd::<u32>(actor + 0x164)));
        }
    }
    *l = Some((slot, now));
    for (f, e) in &m.events {
        let f = *f as f32;
        if f > from && f <= now {
            super::kh2sound::event(m.motion, *e);
            super::kh2fx::event(m.motion, *e);
        }
    }
    super::kh2sound::tick();
}
