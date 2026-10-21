//! The partners a Drive Form takes vanish while it lasts (the user: "lets make it so that the
//! party member associated with the drive from also disappears (along with their hud icon) for
//! the duration of the drive from"). KH2's rule: Valor takes Goofy, Wisdom Donald, Master and
//! Final both, Limit no one, Anti both (`crate::partners::taken`); a guest in their place never
//! vanishes. They go as the change starts and come back beside Sora when the form ends, however
//! it ends. `docs/drive-model.md`, "Partners vanish in the form".
//!
//! How (KH1's own bits, `docs/drive-model.md`):
//! - the actor's flags word (`+0`): `0x8000` off (KH1's actor step `0x140295E50` skips all of an
//!   actor without it: his control and AI, motion and physics; the hit pass `0x14028FEE0` and the
//!   body push `0x14028FF80` leave him out; KH1's game over freezes everyone so), `0x1` and `0x2`
//!   off (his model and his shadow, `0x14029F421` / `0x14029F466`), `0x200` and `0x40` on (no
//!   enemy takes him for a foe, `0x14028FE30`, and nothing touches him: what KH1's own script
//!   op for a member leaving sets, `0x1402B9160`);
//! - his HUD entry: KH1's slot fade (`0x140266CC0`) set to hidden (mode 3, as that op does), kept
//!   hidden while he is away, and on his return faded in as KH1's revive does (mode 1), or left
//!   hidden if Sora's own entry is hidden then.
//! Nothing in the save is touched (its party list stays as it was), so a save in the form has
//! them, and a new room's actors come in whole. His HP, MP and state stay on his actor.
//!
//! Each of Sora's frames checks it (`takeover`'s physics step), and the form's end at once
//! (`form::revert`). A partner KH1 has put back itself (a new room, its game over's revive) is
//! taken again if the form is still on; during an event they are back (a cutscene has them).

use super::addr::Rva;
use super::{log, pose};
use crate::partners::Partner;
use std::sync::Mutex;

/// KH1's field party (Sora, then the two others).
const PARTY_RVA: Rva = Rva(0x2D37C80);
/// Non-zero while an event runs (`takeover`).
const EVENT_RVA: Rva = Rva(0x298D3A0);
/// KH1's HUD gauge slots: a u32 "bound" flag per slot at the start, and slot n's record at
/// `+ n * 0xDF0` (its actor `+0x28`, its alpha `+0xABC`, its fade speed `+0xAC0`).
const HUD_RVA: Rva = Rva(0x28123E0);
const HUD_SLOT: usize = 0xDF0;
/// KH1's HUD slot fade `(slot, mode)`: 0 fade out, 1 fade in, 2 shown, 3 hidden.
const HUD_MODE_RVA: Rva = Rva(0x266CC0);

/// The flag bits changed: on while here (acting, model, shadow), on while away (not a foe,
/// untouchable).
const HERE_BITS: u32 = 0x8000 | 0x1 | 0x2;
const AWAY_BITS: u32 = 0x200 | 0x40;
const MASK: u32 = HERE_BITS | AWAY_BITS;
/// KH1's actor flag stack (`0x14028FFF0` pushes the flags word to `+0x37C + depth * 4` and sets a
/// new one, `0x14028FFD0` pops it; depth `+0x388`), used by its game over.
const STACK: usize = 0x37C;
const DEPTH: usize = 0x388;
const STACK_MAX: u32 = 3;

struct Away {
    actor: usize,
    /// His flags' `MASK` bits before.
    was: u32,
    /// The flag stack's depth when he went.
    depth: u32,
}

/// Per partner (Donald, Goofy): hidden now, and whether he is away for the form (the log).
static STATE: Mutex<[(Option<Away>, bool); 2]> = Mutex::new([(None, false), (None, false)]);

const BOTH: [Partner; 2] = [Partner::Donald, Partner::Goofy];

unsafe fn rd<T: Copy>(p: usize) -> T {
    std::ptr::read_unaligned(p as *const T)
}

unsafe fn wr<T: Copy>(p: usize, v: T) {
    std::ptr::write_unaligned(p as *mut T, v)
}

fn base() -> usize {
    unsafe { super::sys::GetModuleHandleA(std::ptr::null()) as usize }
}

/// The actor of partner `p` in the field party now (live, his object's member id his), if there.
unsafe fn actor_of(base: usize, p: Partner) -> Option<usize> {
    (1..3).map(|slot| rd::<u64>(base + PARTY_RVA + slot * 8) as usize).find(|&a| {
        if a < 0x10000 || !super::crash::readable(a) || !super::crash::readable(a + 0x4AF) {
            return false;
        }
        let live = rd::<u32>(a + 0x374);
        if live & 1 == 0 || live & 2 != 0 {
            return false;
        }
        let obj = pose::ptr(rd::<u32>(a + 0x130));
        obj > 0x10000 && super::crash::readable(obj + 0x4D) && rd::<u16>(obj + 0x4C) == p.member()
    })
}

/// His HUD slot (1..3, bound to his actor).
unsafe fn hud_slot(base: usize, actor: usize) -> Option<usize> {
    let hud = base + HUD_RVA;
    (1..4).find(|&s| rd::<u32>(hud + s * 4) != 0 && rd::<u64>(hud + s * HUD_SLOT + 0x28) as usize == actor)
}

unsafe fn hud_mode(base: usize, slot: usize, mode: i32) {
    let f: unsafe extern "system" fn(i32, i32) = std::mem::transmute(base + HUD_MODE_RVA);
    f(slot as i32, mode);
}

/// A HUD slot's alpha and fade speed.
unsafe fn hud_fade(base: usize, slot: usize) -> (f32, f32) {
    let r = base + HUD_RVA + slot * HUD_SLOT;
    (rd::<f32>(r + 0xABC), rd::<f32>(r + 0xAC0))
}

/// Who the form now (or the change under way) takes, and its name.
fn wanted() -> Option<(&'static [Partner], &'static str)> {
    let kind = super::form::vanishing()?;
    Some((super::form::takes_partners(kind), kind.name()))
}

/// Each of Sora's frames (and at a form's end): partners away while the form takes them, back
/// when it doesn't.
pub fn frame() {
    let base = base();
    let Ok(mut st) = STATE.try_lock() else { return };
    let want = wanted();
    // (Out of a form with no one away: nothing to look at.)
    if want.is_none() && st.iter().all(|(a, g)| a.is_none() && !g) {
        return;
    }
    let event = unsafe { rd::<u32>(base + EVENT_RVA) } != 0;
    for (i, &p) in BOTH.iter().enumerate() {
        let (away, gone) = &mut st[i];
        let takes = want.is_some_and(|(w, _)| w.contains(&p));
        unsafe {
            let actor = actor_of(base, p);
            // His actor gone (a new room, the party changed) or KH1 put him back itself (a new
            // room's actor at the same place, the pop of its game over): nothing of ours left on
            // him (only KH1's creation and that pop give an actor `0x8000` back).
            if let Some(a) = away.as_ref() {
                if actor != Some(a.actor) || rd::<u32>(a.actor) & 0x8000 != 0 {
                    *away = None;
                }
            }
            match (takes && !event, actor, away.is_some()) {
                // (Not while KH1 holds him frozen itself, its game over: his flags then aren't his.)
                (true, Some(actor), false) if rd::<u32>(actor) & 0x8000 != 0 => {
                    *away = Some(hide(base, actor));
                    if !*gone {
                        *gone = true;
                        log(&format!("Drive: {} leaves for {}", p.name(), want.map_or("", |w| w.1)));
                    }
                }
                (true, Some(actor), true) => {
                    // (Kept so if KH1 turns his model or shadow back on.)
                    let f = rd::<u32>(actor);
                    let hidden = (f & !HERE_BITS) | AWAY_BITS;
                    if f != hidden {
                        wr(actor, hidden);
                    }
                    // (KH1 shows the whole HUD again at times, e.g. after an event or its game
                    // over: his entry stays hidden.)
                    if let Some(s) = hud_slot(base, actor) {
                        if hud_fade(base, s) != (0.0, 0.0) {
                            hud_mode(base, s, 3);
                        }
                    }
                }
                (false, _, true) => {
                    if let Some(a) = away.take() {
                        back(base, &a);
                    }
                }
                _ => {}
            }
        }
        if !takes && *gone {
            *gone = false;
            log(&format!("Drive: {} is back", p.name()));
        }
    }
}

unsafe fn hide(base: usize, actor: usize) -> Away {
    let flags = rd::<u32>(actor);
    let a = Away { actor, was: flags & MASK, depth: rd::<u32>(actor + DEPTH).min(STACK_MAX) };
    wr(actor, (flags & !HERE_BITS) | AWAY_BITS);
    if let Some(s) = hud_slot(base, actor) {
        hud_mode(base, s, 3);
    }
    a
}

/// His flags as they were, beside Sora, his HUD entry as Sora's is.
unsafe fn back(base: usize, a: &Away) {
    let actor = a.actor;
    let restore = |f: u32| (f & !MASK) | a.was;
    let depth = rd::<u32>(actor + DEPTH).min(STACK_MAX);
    if depth > a.depth {
        // KH1 pushed his flags while he was away (its game over freezes every actor and pops
        // them at Continue): what it pops is his own again, and it holds `0x8000` off till then.
        for i in a.depth..depth {
            let p = actor + STACK + i as usize * 4;
            wr(p, restore(rd::<u32>(p)));
        }
        let f = rd::<u32>(actor);
        wr(actor, (restore(f) & !0x8000) | (f & 0x8000));
    } else {
        wr(actor, restore(rd::<u32>(actor)));
    }
    // Beside Sora, on the side he was on (KH2: they reappear by him).
    let sora = rd::<u64>(base + PARTY_RVA) as usize;
    if sora > 0x10000 && super::crash::readable(sora + 0x20) {
        let s = rd::<[f32; 3]>(sora + 0x10);
        let mut at = rd::<[f32; 3]>(actor + 0x10);
        let (dx, dz) = (at[0] - s[0], at[2] - s[2]);
        let n = dx.hypot(dz);
        let (ux, uz) = if n.is_finite() && n > 1.0 { (dx / n, dz / n) } else { (1.0, 0.0) };
        if s.iter().all(|v| v.is_finite()) {
            at = [s[0] + ux * BESIDE, s[1], s[2] + uz * BESIDE];
            wr(actor + 0x10, at);
        }
    }
    if let Some(s) = hud_slot(base, actor) {
        let (alpha, speed) = hud_fade(base, 0);
        hud_mode(base, s, if alpha > 0.0 || speed > 0.0 { 1 } else { 3 });
    }
}

/// How far beside Sora a partner comes back.
const BESIDE: f32 = 60.0;
