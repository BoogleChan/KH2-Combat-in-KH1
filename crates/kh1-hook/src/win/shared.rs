//! Shared states: KH1's own state machine owns base Sora; KH2 brings its animations and its
//! physics to the moves both games have, and borrows him only for the moves KH1 doesn't have.
//!
//! The user's rewrite (2026-10-20, testing branch): "kh2 replaces the base animations that share
//! logic (idle walk, run) with kh2s animations ... but then reinjects kh1s soras animations back
//! into the game. We need to have a clean rewrite". The rule, per move style (`styles`, every one
//! KH1 by default):
//!
//! - **A shared move** (idle / walk / run / item use: Misc Animations; jump, high jump, fall,
//!   landing: Jump) always runs in KH1's own states, with KH1's own rules (its motion slots, its
//!   transitions, ledge grabs, landing). At KH1 nothing of ours touches it. At KH2, KH1's slot is
//!   drawn with KH2's motion for the same thing (`kh2_look`, `pose::follow`) and KH2's numbers
//!   move him (`physics`: KH2's run speed, KH2's jump arc), "KH1 should treat KH2's animations as
//!   its own".
//! - **A KH2-only move** (Aerial Dodge, KH2's glide, KH2's Square moves, KH2's combos, spells and
//!   Drives for now) is KH2's controller's: it borrows Sora out of KH1's states for it
//!   (`borrow`), and gives him back to KH1's ground or air state the moment the move is over
//!   (`give_back`). So with Jump at KH1, Aerial Dodge is still his double jump.
//! - **A KH1-only move** (KH1's specials, its own glide, summons) is KH1's, as unmodded.
//! - Drive Forms keep KH2's controller all through (their own movesets), as before.
//!
//! `takeover` keeps the hand-off mechanics (what KH1's fields are set to either way); this
//! module says when. `docs/shared-states.md`.

use super::styles::{self, Kind};
use super::takeover;
use character_core::StateName;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

/// Base Sora is KH1's own states' (the ini's `shared`, not in a Drive Form).
pub fn on() -> bool {
    super::settings::get().shared && !super::form::in_form()
}

/// The move style a KH1 motion slot of Sora's shows (`pose`'s slot numbers: the user's play
/// logs): 0..7 field and battle idle / walk / run; 8 / 9 high jump, 12 / 13 jump and rise, 10
/// fall, 11 landing; 98..101 the ground combo's swings, 102..104 the air combo's.
pub fn slot_kind(slot: usize) -> Option<Kind> {
    Some(match slot {
        0..=7 => Kind::MiscAnims,
        8..=13 => Kind::Jump,
        98..=101 => Kind::GroundCombo,
        102..=104 => Kind::AirCombo,
        _ => return None,
    })
}

/// KH2's motion is drawn over this KH1 slot: its style is KH2's (base Sora; a Drive Form draws
/// the form's motions over every slot it has one for, as before).
pub fn kh2_look(slot: Option<usize>) -> bool {
    if super::form::in_form() {
        return true;
    }
    slot.and_then(slot_kind).is_some_and(styles::kh2)
}

/// The hand-off now running is ours: KH1 has base Sora in its own states.
static MODE: AtomicBool = AtomicBool::new(false);
/// KH2's controller has him for one of its moves (`borrow` .. `give_back`).
static KH2_MOVE: AtomicBool = AtomicBool::new(false);

/// KH1 has base Sora in its own states now.
pub fn active() -> bool {
    MODE.load(Ordering::Relaxed) && takeover::handing_off()
}

/// KH2 has him for one of its own moves.
pub fn kh2_move() -> bool {
    KH2_MOVE.load(Ordering::Relaxed)
}

/// Any hand-off (`takeover::hand_to_kh1`) ends the mode; ours sets it after.
pub fn clear() {
    MODE.store(false, Ordering::Relaxed);
}

/// KH1's states have him from now (after `takeover::hand_to_kh1`).
pub fn started() {
    MODE.store(true, Ordering::Relaxed);
    KH2_MOVE.store(false, Ordering::Relaxed);
    BORROW.store(NONE, Ordering::Relaxed);
}

/// KH2's controller took him for a move.
pub fn lent() {
    MODE.store(false, Ordering::Relaxed);
    KH2_MOVE.store(true, Ordering::Relaxed);
    WANT_KH2.store(false, Ordering::Relaxed);
}

/// What KH2 borrows him for out of KH1's states, read off the buttons in KH1's control step
/// (`control`) and acted on in its physics step (`takeover`).
pub const NONE: u8 = 0;
/// Square: KH2's dodge roll / guard / Quick Run on the ground (Dodge Roll at KH2), KH2's glide in
/// the air (Glide at KH2, on Square).
pub const SQUARE: u8 = 1;
/// The jump button in the air: Aerial Dodge, or KH2's glide on the jump button (Glide at KH2).
pub const AIR_JUMP: u8 = 2;
static BORROW: AtomicU8 = AtomicU8::new(NONE);
/// A command of KH2's chosen while KH1 has him (`combat::command`: a KH2 spell, Reflect, a Drive):
/// KH2 takes him for it, then runs it (`combat::resume_command`).
static WANT_KH2: AtomicBool = AtomicBool::new(false);
/// Aerial Dodge used in this air time (KH2 allows one; back on the ground, again).
static AIR_DODGED: AtomicBool = AtomicBool::new(false);
/// Buttons on KH1's last control step, for fresh presses.
static LAST: AtomicU32 = AtomicU32::new(0);

pub fn want_kh2() {
    WANT_KH2.store(true, Ordering::Relaxed);
}

pub fn take_want_kh2() -> bool {
    WANT_KH2.swap(false, Ordering::Relaxed)
}

/// This frame's borrow, taken.
pub fn take_borrow() -> u8 {
    BORROW.swap(NONE, Ordering::Relaxed)
}

/// What KH1's control step sees of the buttons while KH1 has base Sora, and what it should not
/// see (`kh1_hides`: KH1's own bits to take out of its button state for its step).
pub struct Seen {
    /// KH1's movement state (0 ground, 1 jump, 2 fall, 0x15 glide...).
    pub state: i32,
    /// KH1's can-act test.
    pub can_act: bool,
    /// Going up this frame (KH1's jump still rising).
    pub rising: bool,
    pub jump: bool,
    /// Square on the pad (not KH1's action bit its held jump sets in the air).
    pub square: bool,
    /// KH2's Aerial Dodge is equipped.
    pub air_dodge: bool,
}

/// KH1's button bits for its jump (6) and its Square action (`0x100000`).
pub const KH1_JUMP_BITS: u32 = 0x6;
pub const KH1_ACTION_BIT: u32 = 0x10_0000;

/// One control step of KH1's with base Sora in its states: the borrow it starts (kept for the
/// physics step), and KH1's bits it doesn't get this step.
pub fn control(s: &Seen) -> u32 {
    let was = LAST.swap((s.jump as u32) | (s.square as u32) << 1, Ordering::Relaxed);
    let jump_fresh = s.jump && was & 1 == 0;
    let square_fresh = s.square && was & 2 == 0;
    let air = matches!(s.state, 1 | 2);
    if s.state == 0 {
        AIR_DODGED.store(false, Ordering::Relaxed);
    }
    let glide_kh2 = styles::kh2(Kind::Glide);
    let on_square = super::settings::get().glide_on_square;
    let roll_kh2 = styles::kh2(Kind::DodgeRoll);
    let mut borrow = NONE;
    if s.can_act {
        if s.state == 0 && square_fresh && roll_kh2 {
            borrow = SQUARE;
        } else if air && square_fresh && glide_kh2 && on_square && !s.rising {
            borrow = SQUARE;
        } else if air && jump_fresh && s.air_dodge && !AIR_DODGED.load(Ordering::Relaxed) {
            AIR_DODGED.store(true, Ordering::Relaxed);
            borrow = AIR_JUMP;
        } else if air && jump_fresh && glide_kh2 && !on_square && !s.rising {
            borrow = AIR_JUMP;
        }
    }
    if borrow != NONE {
        BORROW.store(borrow, Ordering::Relaxed);
        // (The press is KH2's: KH1 sees none of the button this step.)
        return if borrow == SQUARE { KH1_ACTION_BIT } else { KH1_JUMP_BITS | KH1_ACTION_BIT };
    }
    let mut hide = 0;
    // On the ground: Square is KH2's with the Dodge Roll style at KH2 (KH2's roll, guard and
    // Quick Run); KH1's own roll and guard with it at KH1.
    if s.state == 0 && roll_kh2 {
        hide |= KH1_ACTION_BIT;
    }
    // In the air past the rise: KH1's glide comes from its held jump (and the action bit its held
    // jump sets). KH1's glide only with the Glide style at KH1 and the ini's `glide = jump`; on
    // Square (`glide = square`) it is started from Square (`takeover`), and with Glide at KH2 it
    // is KH2's (the user: "Sora is still able to do a normal kh1 glide by pressing the jump
    // button"). Rising, the held jump is KH1's own, for its height.
    if air && !s.rising && (glide_kh2 || on_square) {
        hide |= KH1_JUMP_BITS | KH1_ACTION_BIT;
    }
    hide
}

/// KH2 gives him back to KH1's states: on the ground (KH1's ground state), or falling (KH1's fall
/// state).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    Ground,
    Air,
}

/// Frames KH2's controller has been in a state KH1 shares.
static SHARED_FRAMES: AtomicU32 = AtomicU32::new(0);
/// An air combo ended this long ago (frames since its last swing): KH1's fall from there (KH2
/// keeps a combo going until landing otherwise).
const AIR_COMBO_DONE: u32 = 24;

/// After KH2's step during one of its moves: whether its move is over and where he goes back to.
/// `combo`: KH2's combo going and frames since its last swing; `waiting`: a command or press of
/// KH2's still to run.
pub fn give_back(state: StateName, grounded: bool, falling: bool, combo: (bool, u32), waiting: bool) -> Option<Back> {
    let back = match state {
        StateName::Idle | StateName::Walk | StateName::Run | StateName::Land if grounded && !combo.0 => Some(Back::Ground),
        StateName::Air if !grounded && falling && (!combo.0 || combo.1 >= AIR_COMBO_DONE) => Some(Back::Air),
        _ => None,
    };
    let back = back.filter(|_| !waiting);
    let n = if back.is_some() { SHARED_FRAMES.fetch_add(1, Ordering::Relaxed) + 1 } else { SHARED_FRAMES.store(0, Ordering::Relaxed); 0 };
    // (Two frames in it: a move's own one-frame pass through idle doesn't end it.)
    back.filter(|_| n >= 2).inspect(|_| SHARED_FRAMES.store(0, Ordering::Relaxed))
}

/// KH2's physics on KH1's own states, per move style at KH2 (the user: "KH2 will still handle its
/// own unique properties, such as running speed"; "KH2 must still handle its unique jump
/// physics"). KH1's state table (`0x142DB4240`, 0x40 bytes a state: `+0x18` pre-step `(actor,
/// input)`, `+0x20` step `(actor, &displacement)`, `+0x28` transition) is writable: the steps
/// below are wrapped there for Sora, KH1's own step running first.
///
/// - Misc Animations at KH2: the ground step's displacement (`0x1402AF6C0`: the move direction on
///   the floor × `+0x2C`, which is KH1's walk 2.0 or run 8.0 from the stats' `+4` / `+8`, by the
///   stick past 0.8) is scaled to KH2's walk or run speed. KH1's choice of walk or run (and so its
///   animation) is untouched.
/// - Jump at KH2: KH1's jump (its takeoff, state 1's pre-step `0x1402ADC10`, then its rise and
///   fall in state 2's step `0x1402AD510`) follows KH2's arc: KH2's jump or High Jump height and
///   gravity, its early-release hop (after 6 frames of the rise, more than 5 before the apex), its
///   terminal fall speed, its air steering. KH1's own arc fields (`+0x3B8` time to the apex,
///   `+0x3BC` the height) are kept in step with it, so KH1's apex animation and its air-move
///   gating line up; KH1's own early cut (flag `0x40000`) is kept off. Landing stays KH1's (its
///   collision).
pub mod physics {
    use super::super::addr::Rva;
    use super::styles::{self, Kind};
    use character_core::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    const TABLE_RVA: Rva = Rva(0x2DB4240);
    /// KH1's frame scale (1.0 at 60 fps).
    const DT_RVA: Rva = Rva(0x23405DC);
    const PARTY_RVA: Rva = Rva(0x2D37C80);
    const ENTRY: usize = 0x40;
    const PRE_STEP: usize = 0x18;
    const STEP: usize = 0x20;
    /// KH1's arc's start height, its time to the apex and its height (negative: KH1 is y-down).
    const ARC_T: usize = 0x3B0;
    const ARC_Y0: usize = 0x3B4;
    const ARC_UP_T: usize = 0x3B8;
    const ARC_H: usize = 0x3BC;
    /// KH1's early-release cut is still open (its button handler, `0x1402A59A0`).
    const CUT_OPEN: u32 = 0x4_0000;
    /// KH1's gravity (`0x1403EAB54`).
    const KH1_G: f32 = 0.484_693_88;

    static ORIG_GROUND: AtomicUsize = AtomicUsize::new(0);
    static ORIG_AIR: AtomicUsize = AtomicUsize::new(0);
    static ORIG_TAKEOFF: AtomicUsize = AtomicUsize::new(0);

    /// KH2's movement numbers with Sora's abilities now (`takeover::refresh_kh2_numbers`).
    #[derive(Clone, Copy, Debug)]
    pub struct Numbers {
        pub walk: f32,
        pub run: f32,
        pub jump: f32,
        pub high_jump: f32,
        pub gravity: f32,
        pub fall_max: f32,
        pub air_keep: f32,
        pub air_drag: f32,
    }
    static NUMBERS: Mutex<Option<Numbers>> = Mutex::new(None);

    pub fn set_numbers(n: Numbers) {
        if let Ok(mut g) = NUMBERS.try_lock() {
            *g = Some(n);
        }
    }

    fn numbers() -> Option<Numbers> {
        NUMBERS.try_lock().ok().and_then(|g| *g).filter(|n| n.gravity > 0.0)
    }

    /// KH2's arc for KH1's jump or fall now, the height KH1 reached on its last step and the rise
    /// asked for then (a ceiling: less than half of it happened).
    struct Air {
        arc: Arc,
        last_y: f32,
        asked_up: f32,
    }
    static AIR: Mutex<Option<Air>> = Mutex::new(None);

    /// Not in KH1's air state (or KH2 has him): no arc of ours.
    pub fn off_air() {
        if let Ok(mut a) = AIR.try_lock() {
            *a = None;
        }
    }

    unsafe fn rd<T: Copy>(p: usize) -> T {
        std::ptr::read_unaligned(p as *const T)
    }

    unsafe fn wr<T: Copy>(p: usize, v: T) {
        std::ptr::write_unaligned(p as *mut T, v)
    }

    fn base() -> usize {
        super::super::takeover::game_base()
    }

    unsafe fn is_sora(actor: usize) -> bool {
        actor > 0x10000 && actor == rd::<u64>(base() + PARTY_RVA) as usize
    }

    /// KH2's physics apply to KH1's states for this kind of move now.
    fn kh2(kind: Kind) -> bool {
        super::active() && styles::kh2(kind) && !super::super::form::in_form()
    }

    /// Puts our wrappers in KH1's state table (each Sora physics frame: KH1 fills the table itself
    /// when it sets its states up, which would drop them).
    pub unsafe fn install(base: usize) {
        let table = base + TABLE_RVA;
        let slots: [(usize, &AtomicUsize, usize); 3] = [
            (table + STEP, &ORIG_GROUND, ground_step as *const () as usize),
            (table + 2 * ENTRY + STEP, &ORIG_AIR, air_step as *const () as usize),
            (table + ENTRY + PRE_STEP, &ORIG_TAKEOFF, takeoff as *const () as usize),
        ];
        for (slot, orig, ours) in slots {
            let now: usize = rd(slot);
            if now != ours && now > 0x10000 {
                orig.store(now, Ordering::Release);
                wr(slot, ours);
                super::super::log(&format!("shared states: KH1's state step at {slot:#x} wrapped ({now:#x})"));
            }
        }
    }

    type StepFn = unsafe extern "system" fn(usize, *mut [f32; 4]) -> usize;

    /// State 0's step: KH2's walk or run speed (Misc Animations at KH2).
    unsafe extern "system" fn ground_step(actor: usize, disp: *mut [f32; 4]) -> usize {
        let f: StepFn = std::mem::transmute(ORIG_GROUND.load(Ordering::Acquire));
        let r = f(actor, disp);
        if !is_sora(actor) || rd::<i32>(actor + 0x70) != 0 || disp.is_null() || !kh2(Kind::MiscAnims) {
            return r;
        }
        let Some(n) = numbers() else { return r };
        let speed: f32 = rd(actor + 0x2C);
        let stats = super::super::pose::ptr(rd::<u32>(actor + 0x6C));
        if speed <= 0.01 || stats < 0x10000 {
            return r;
        }
        let kh1_run: f32 = rd(stats + 8);
        let kh2 = if speed >= kh1_run - 0.01 { n.run } else { n.walk };
        let k = kh2 / speed;
        if k.is_finite() && k > 0.0 {
            let d = &mut *disp;
            d[0] *= k;
            d[2] *= k;
        }
        r
    }

    type PreFn = unsafe extern "system" fn(usize, usize) -> usize;

    /// State 1's pre-step (KH1's takeoff): when it launches him (state 2), KH2's jump arc.
    unsafe extern "system" fn takeoff(actor: usize, input: usize) -> usize {
        let f: PreFn = std::mem::transmute(ORIG_TAKEOFF.load(Ordering::Acquire));
        let r = f(actor, input);
        if !is_sora(actor) || rd::<i32>(actor + 0x70) != 2 || !kh2(Kind::Jump) {
            return r;
        }
        let Some(n) = numbers() else { return r };
        // (KH1's High Jump: its landing-motion base 5, `0x1402AD860`.)
        let high = rd::<u32>(actor + 0x3D0) == 5;
        let h = if high { n.high_jump } else { n.jump };
        let mut arc = Arc::new(h, n.gravity);
        arc.holdable = true;
        wr(actor + ARC_UP_T, arc.uptime);
        wr(actor + ARC_H, -h);
        if let Ok(mut a) = AIR.lock() {
            *a = Some(Air { arc, last_y: rd(actor + 0x14), asked_up: 0.0 });
        }
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if N.fetch_add(1, Ordering::Relaxed) < 20 {
            super::super::log(&format!("KH1's {} with KH2's arc: {h:.0} high, {:.1} frames up (KH1's own: {:.0})", if high { "High Jump" } else { "jump" }, arc.uptime, {
                let stats = super::super::pose::ptr(rd::<u32>(actor + 0x6C));
                if stats > 0x10000 { rd::<f32>(stats + 0x10) } else { 0.0 }
            }));
        }
        r
    }

    /// State 2's step (KH1's rise and fall): KH2's arc and air steering (Jump at KH2).
    unsafe extern "system" fn air_step(actor: usize, disp: *mut [f32; 4]) -> usize {
        let f: StepFn = std::mem::transmute(ORIG_AIR.load(Ordering::Acquire));
        let sora = is_sora(actor);
        let v0: [f32; 4] = if sora { rd(actor + 0x3C0) } else { [0.0; 4] };
        let r = f(actor, disp);
        if !sora || rd::<i32>(actor + 0x70) != 2 || disp.is_null() || !kh2(Kind::Jump) {
            return r;
        }
        let Some(n) = numbers() else { return r };
        let Ok(mut g) = AIR.try_lock() else { return r };
        // KH1's own action in the air (flag 4: an air attack, a spell's cast) moves him its own way.
        if rd::<u32>(actor) & 4 != 0 {
            *g = None;
            return r;
        }
        let dt: f32 = rd(base() + DT_RVA);
        let dt = if dt.is_finite() && dt > 0.0 && dt < 4.0 { dt } else { 1.0 };
        let y: f32 = rd(actor + 0x14);
        // (A fall KH1 started itself: off a ledge, out of a ladder or a move.)
        let air = g.get_or_insert_with(|| Air { arc: Arc::fall(), last_y: y, asked_up: 0.0 });
        // A ceiling stopped the rise (less than half of it happened): falling from here.
        if air.asked_up > 0.5 && (air.last_y - y) < air.asked_up * 0.49 {
            air.arc = Arc::fall();
            wr(actor + ARC_UP_T, rd::<f32>(actor + ARC_T));
        }
        // KH2's early release (its own rule; KH1's cut kept off).
        wr(actor, rd::<u32>(actor) & !CUT_OPEN);
        if air.arc.holdable && air.arc.rising() {
            let held = super::super::takeover::buttons() & super::super::takeover::jump_button() != 0;
            if !held && air.arc.t > character_core::HOLD_MIN_FRAMES && air.arc.t < air.arc.uptime - character_core::HOLD_APEX_MARGIN {
                let hop = character_core::release_hop(n.gravity);
                air.arc = Arc::new(hop, n.gravity);
                wr(actor + ARC_UP_T, rd::<f32>(actor + ARC_T) + air.arc.uptime);
                wr(actor + ARC_H, -hop);
                wr(actor + ARC_Y0, y);
            } else if air.arc.t >= air.arc.uptime - character_core::HOLD_APEX_MARGIN {
                air.arc.holdable = false;
            }
        }
        let up = air.arc.advance(dt, n.gravity, n.fall_max);
        // Past KH2's apex: KH1's own arc past its apex too (its fall animation, its air moves).
        if !air.arc.rising() && rd::<f32>(actor + ARC_UP_T) > rd::<f32>(actor + ARC_T) {
            wr(actor + ARC_UP_T, rd::<f32>(actor + ARC_T));
        }
        let d = &mut *disp;
        d[1] = -up;
        wr(actor + 0x3C4, -up);
        air.last_y = y;
        air.asked_up = up;
        // Steering: KH2's (`air_keep` toward the stick at KH2's walk or run speed, `air_drag`
        // without it), from the speed before KH1's own steering this frame.
        let speed: f32 = rd(actor + 0x2C);
        let dir: [f32; 3] = rd(actor + 0x20);
        let stats = super::super::pose::ptr(rd::<u32>(actor + 0x6C));
        let kh1_run: f32 = if stats > 0x10000 { rd(stats + 8) } else { 8.0 };
        let mut v = [v0[0], v0[2]];
        if !v.iter().all(|x| x.is_finite()) {
            v = [0.0; 2];
        }
        if speed > 0.01 && dir.iter().all(|x| x.is_finite()) {
            let s = if speed >= kh1_run - 0.01 { n.run } else { n.walk };
            let t = [dir[0] * s, dir[2] * s];
            v = [v[0] * n.air_keep + t[0] * (1.0 - n.air_keep), v[1] * n.air_keep + t[1] * (1.0 - n.air_keep)];
        } else {
            v = [v[0] * n.air_drag, v[1] * n.air_drag];
        }
        let len = v[0].hypot(v[1]);
        wr(actor + 0x3C0, [v[0], 0.0f32, v[1], len]);
        d[0] = v[0] * dt;
        d[2] = v[1] * dt;
        r
    }

    /// KH1's fall state just set up by a give-back (`takeover::shared_give_back`): his fall goes on
    /// at KH2's speed `up` (per frame, KH2's way up): KH2's arc with Jump at KH2, else KH1's own
    /// fall picked up at that speed.
    pub unsafe fn fall_from(actor: usize, up: f32) {
        let up = if up.is_finite() { up } else { 0.0 };
        if kh2_now() {
            if let Some(n) = numbers() {
                let mut arc = if up > 0.5 { Arc::new(up * up / (2.0 * n.gravity), n.gravity) } else { Arc::fall() };
                if up <= 0.5 {
                    arc.t = (-up).max(0.0) / n.gravity;
                }
                if let Ok(mut a) = AIR.lock() {
                    *a = Some(Air { arc, last_y: rd(actor + 0x14), asked_up: 0.0 });
                }
                return;
            }
        }
        // KH1's fall: `y = start + g/2 t^2` (y-down) from its air time, so that time is where its
        // speed is his.
        let down = (-up).max(0.0).min(20.0);
        let t0 = down / KH1_G;
        let y: f32 = rd(actor + 0x14);
        wr(actor + ARC_T, t0);
        wr(actor + ARC_Y0, y - 0.5 * KH1_G * t0 * t0);
        off_air();
    }

    /// Jump at KH2 for base Sora (the give-back sets the mode right after).
    fn kh2_now() -> bool {
        styles::kh2(Kind::Jump) && !super::super::form::in_form()
    }
}
