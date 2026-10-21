//! Shared states: KH1's own state machine owns base Sora; KH2 brings its animations and its
//! physics to the moves both games have, and borrows him only for the moves KH1 doesn't have.
//!
//! The user's rewrite (2026-10-20, testing branch): "kh2 replaces the base animations that share
//! logic (idle walk, run) with kh2s animations ... but then reinjects kh1s soras animations back
//! into the game. We need to have a clean rewrite". The rule, per move style (`styles`, every one
//! KH1 by default):
//!
//! - **A shared move** (idle / walk / run / item use: Misc Animations; jump, high jump, fall,
//!   landing: Jump; the ground combo's swings and finisher: Base Combo; the glide: Glide) always
//!   runs in KH1's own states, with KH1's own rules (its motion slots, its transitions, ledge
//!   grabs, landing, its combo's rows, hit objects and damage). At KH1 nothing of ours touches it.
//!   At KH2, KH1's slot is drawn with KH2's motion for the same thing (`kh2_look`,
//!   `pose::follow`) and KH2's numbers move him (`physics`: KH2's run speed, KH2's jump arc, KH2's
//!   swing travel, KH2's glide speed, fall and turn), "KH1 should treat KH2's animations as its
//!   own". The user (after s261): "currently don't share animations but should: sora base
//!   attack 2, kh2 base attack 2 / kh1 base finisher, kh2 base finishers / glide".
//! - **A KH2-only move** (Aerial Dodge, KH2's Square moves, KH2's Quick Run out of KH1's roll,
//!   the aerial combo at KH2, KH2's spells and Drives for now) is KH2's controller's: it borrows
//!   Sora out of KH1's states for it (`borrow`), and gives him back to KH1's ground or air state
//!   the moment the move is over (`give_back`). So with Jump at KH1, Aerial Dodge is still his
//!   double jump.
//! - **A KH1-only move** (KH1's specials, summons) is KH1's, as unmodded.
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
/// fall, 11 landing; 98..101 the ground combo's swings (98 the first and third, 99 the second,
/// 101 the finisher), 102..104 the air combo's; 113..116 KH1's glide (its pre-step
/// `0x1402B06F0` picks 0x71 / 0x72 gliding slow / fast, 0x73 / 0x74 banking into a turn; 113 and
/// 115 seen in the s261 log).
pub fn slot_kind(slot: usize) -> Option<Kind> {
    Some(match slot {
        0..=7 => Kind::MiscAnims,
        8..=13 => Kind::Jump,
        98..=101 => Kind::GroundCombo,
        102..=104 => Kind::AirCombo,
        GLIDE_SLOTS_START..=GLIDE_SLOTS_END => Kind::Glide,
        _ => return None,
    })
}

/// KH1's glide slots (`slot_kind`).
pub const GLIDE_SLOTS_START: usize = 113;
pub const GLIDE_SLOTS_END: usize = 116;

/// The Attack command runs KH1's own row where he is (KH1's command step, `combat::exec`): the
/// ground combo is a shared move (KH1's states at either style, KH2's swings and lunges drawn on
/// them at KH2: `pose::follow`, `physics`); the air combo is KH1's at KH1 and a KH2 borrow at KH2.
pub fn kh1_combo(air: bool) -> bool {
    !air || styles::kh1_now(Kind::AirCombo)
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
/// Square: KH2's dodge roll / guard / Quick Run on the ground (Dodge Roll at KH2).
pub const SQUARE: u8 = 1;
/// The jump button in the air: Aerial Dodge.
pub const AIR_JUMP: u8 = 2;
/// Square held through KH1's own roll (Dodge Roll at KH1): KH2's Quick Run out of it, when Sora
/// has Quick Run (it has no style of its own: KH2's move, by its ability).
pub const QUICK_RUN: u8 = 3;
static BORROW: AtomicU8 = AtomicU8::new(NONE);
/// Square held since it started KH1's roll (`QUICK_RUN`), and how long KH1's roll motion has
/// played in that hold (frames at 60 Hz, f32 bits; 0 not yet seen).
static ROLL_HELD: AtomicBool = AtomicBool::new(false);
static ROLL_FRAMES: AtomicU32 = AtomicU32::new(0);
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
    /// KH1's motion is in its combo window (a swing of KH1's own combo can chain now).
    pub combo_window: bool,
    /// KH1 has him knocked away in the air, far enough in for KH2's Aerial Recovery
    /// (`combomods::launched`).
    pub launched: bool,
    /// KH2 has something for Square on the ground: a dodge roll, Guard or Quick Run (KH1's
    /// menu: Dodge Roll, Guard, Quick Run equipped).
    pub kh2_square: bool,
    /// KH2's Quick Run is equipped.
    pub quick_run: bool,
    /// KH1's roll motion (`styleroll::ROLL_MOTION`) is playing.
    pub kh1_rolling: bool,
    /// KH1 has its own roll (its Dodge Roll equipped: bit `0x20` of `0x142D5F610`).
    pub kh1_roll: bool,
    /// The stick is pushed (past KH1's roll's 0.15).
    pub moving: bool,
    /// Where in KH2's roll its Quick Run check comes (frames).
    pub quick_run_check: f32,
    /// KH1's frame step (1.0 at 60 fps).
    pub dt: f32,
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
    let on_square = super::settings::get().glide_on_square;
    let roll_kh2 = styles::kh2(Kind::DodgeRoll);
    // KH2's ability moves out of KH1's own moves whatever the styles (`combomods`): a Square move
    // in KH1's combo window, Aerial Recovery / Retaliating Slash while KH1 has him knocked away.
    // The press is KH2's: KH1 sees none of it.
    if let Some(hide) = super::combomods::control(s, jump_fresh, square_fresh) {
        return hide;
    }
    // KH1's roll held into KH2's Quick Run (the user: "kh1 dodge roll doesn't go into quick run
    // when holding square"): Square pressed for KH1's roll and held since, KH1's roll motion seen,
    // then KH2 takes him for its Quick Run once KH1 lets him act at or past KH2's own Quick Run
    // check in its roll (or when KH1's roll is over).
    if !s.square {
        ROLL_HELD.store(false, Ordering::Relaxed);
        ROLL_FRAMES.store(0, Ordering::Relaxed);
    } else if s.state == 0 && square_fresh && !roll_kh2 {
        ROLL_HELD.store(true, Ordering::Relaxed);
        ROLL_FRAMES.store(0, Ordering::Relaxed);
    }
    let roll_frames = f32::from_bits(ROLL_FRAMES.load(Ordering::Relaxed));
    let (roll_frames, roll_over) = if !ROLL_HELD.load(Ordering::Relaxed) {
        (0.0, false)
    } else if s.kh1_rolling {
        let n = roll_frames + s.dt.clamp(0.0, 4.0);
        ROLL_FRAMES.store(n.to_bits(), Ordering::Relaxed);
        (n, false)
    } else {
        (roll_frames, roll_frames > 0.0)
    };
    let quick_run = s.state == 0 && s.square && s.quick_run && roll_frames > 0.0 && (roll_over || roll_frames >= s.quick_run_check);
    let mut borrow = NONE;
    if s.can_act {
        if quick_run {
            ROLL_HELD.store(false, Ordering::Relaxed);
            ROLL_FRAMES.store(0, Ordering::Relaxed);
            borrow = QUICK_RUN;
        } else if s.state == 0 && square_fresh && !roll_kh2 && !s.kh1_roll && s.quick_run && s.moving {
            // (Dodge Roll at KH1 without KH1's roll equipped: Square while moving is KH2's Quick
            // Run, which has no style of its own, when it is equipped.)
            borrow = SQUARE;
        } else if s.state == 0 && square_fresh && roll_kh2 {
            // (Nothing of KH2's for Square without those abilities: KH2 would only take him and
            // give him back two frames later, s261's "kh2 dodge roll is unusable" with Dodge
            // Roll, Guard and Quick Run all taken off in KH1's menu. KH1's own roll needs its
            // Dodge Roll equipped too, its bit `0x20` of `0x142D5F610`, set in its control step
            // from `0x140282A70 (22)`.)
            if s.kh2_square {
                borrow = SQUARE;
            } else {
                static N: AtomicU32 = AtomicU32::new(0);
                if N.fetch_add(1, Ordering::Relaxed) < 10 {
                    super::log("Square with the Dodge Roll style KH2: KH2 has no dodge roll, guard or Quick Run (none of Dodge Roll, Guard, Quick Run is equipped in KH1's menu), so it doesn't take him");
                }
            }
        } else if air && jump_fresh && s.air_dodge && !AIR_DODGED.load(Ordering::Relaxed) {
            AIR_DODGED.store(true, Ordering::Relaxed);
            borrow = AIR_JUMP;
        }
    }
    if borrow != NONE {
        BORROW.store(borrow, Ordering::Relaxed);
        // (The press is KH2's: KH1 sees none of the button this step.)
        return if matches!(borrow, SQUARE | QUICK_RUN) { KH1_ACTION_BIT } else { KH1_JUMP_BITS | KH1_ACTION_BIT };
    }
    let mut hide = 0;
    // On the ground: Square is KH2's with the Dodge Roll style at KH2 (KH2's roll, guard and
    // Quick Run); KH1's own roll and guard with it at KH1.
    if s.state == 0 && roll_kh2 {
        hide |= KH1_ACTION_BIT;
    }
    // In the air past the rise: KH1's glide comes from its held jump (and the action bit its held
    // jump sets). The glide is a shared move: KH1's glide state at either style (KH2's glide
    // motions and numbers on it at KH2, `pose::follow`, `physics`). With the ini's `glide = jump`
    // KH1's own held jump glides him; on Square (`glide = square`) KH1's glide is started from
    // Square (`takeover`), so its jump isn't seen there (the user: "Sora is still able to do a
    // normal kh1 glide by pressing the jump button"). Rising, the held jump is KH1's own, for its
    // height.
    if air && !s.rising && on_square {
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
/// - Base Combo at KH2: KH1's ground swings (slots 98..101, its action flag 4 up) move him by the
///   root travel of KH2's swing drawn on them (`pose::swing_motion`), stretched over KH1's swing,
///   along his facing, in place of KH1's own travel. KH1's hit objects, timing and damage stay.
/// - Glide at KH2: KH1's glide (state 0x15) turns toward the stick at KH2's glide turn rate (its
///   pre-step `0x1402B06F0`, the facing set after KH1's own turn is undone) and moves at KH2's
///   glide speed and fall (its step `0x1402B0890`: along `+0x30`, KH1's speed up to its top and a
///   steady fall otherwise), KH2's growth numbers at Sora's Glide level. KH1's glide start, end,
///   slots and collision stay.
pub mod physics {
    use super::super::addr::Rva;
    use super::styles::{self, Kind};
    use character_core::Arc;
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
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
    static ORIG_GLIDE_PRE: AtomicUsize = AtomicUsize::new(0);
    static ORIG_GLIDE: AtomicUsize = AtomicUsize::new(0);
    /// KH1's glide state (`0x1402B0BA0` starts it; registered by `0x1402B0560`: pre-step
    /// `0x1402B06F0 (actor, input)`, step `0x1402B0890 (actor, &displacement)`).
    pub const GLIDE_STATE: usize = 0x15;
    /// The actor's facing setter (`0x140295C80 (actor, angle)`, `takeover`'s too).
    const SET_FACING_RVA: Rva = Rva(0x295C80);
    /// The stick's tilt on KH1's last glide pre-step (f32 bits), for its step.
    static GLIDE_TILT: AtomicU32 = AtomicU32::new(0);
    /// KH1's ground swing now (its slot, and KH1 frames into it: KH1's swings alternate slots,
    /// 98 / 99, so a new slot is a new swing), for KH2's swing travel.
    static SWING: Mutex<Option<(usize, f32)>> = Mutex::new(None);

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
        /// KH2's glide at Sora's Glide level (none without it).
        pub glide: Option<Glide>,
    }

    /// KH2's glide numbers (its growth table, per 60 Hz frame): top speed, how much of the last
    /// frame's speed is kept, the fall (`(height - fall_height) × fall_ratio`, at most
    /// `fall_max`), the turn rate (radians).
    #[derive(Clone, Copy, Debug)]
    pub struct Glide {
        pub speed: f32,
        pub acceleration: f32,
        pub fall_ratio: f32,
        pub fall_height: f32,
        pub fall_max: f32,
        pub turn: f32,
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
        let slots: [(usize, &AtomicUsize, usize); 5] = [
            (table + STEP, &ORIG_GROUND, ground_step as *const () as usize),
            (table + 2 * ENTRY + STEP, &ORIG_AIR, air_step as *const () as usize),
            (table + ENTRY + PRE_STEP, &ORIG_TAKEOFF, takeoff as *const () as usize),
            (table + GLIDE_STATE * ENTRY + PRE_STEP, &ORIG_GLIDE_PRE, glide_pre as *const () as usize),
            (table + GLIDE_STATE * ENTRY + STEP, &ORIG_GLIDE, glide_step as *const () as usize),
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

    /// KH1's frame step now (1.0 at 60 fps).
    pub fn dt() -> f32 {
        let dt: f32 = unsafe { rd(base() + DT_RVA) };
        if dt.is_finite() && dt > 0.0 && dt < 4.0 {
            dt
        } else {
            1.0
        }
    }

    /// State 0's step: KH2's walk or run speed (Misc Animations at KH2); KH2's swing travel in
    /// KH1's ground combo (Base Combo at KH2).
    unsafe extern "system" fn ground_step(actor: usize, disp: *mut [f32; 4]) -> usize {
        let f: StepFn = std::mem::transmute(ORIG_GROUND.load(Ordering::Acquire));
        let r = f(actor, disp);
        if !is_sora(actor) || rd::<i32>(actor + 0x70) != 0 || disp.is_null() {
            return r;
        }
        let slot = super::super::pose::sora_slot();
        if matches!(slot, Some(98..=101)) && rd::<u32>(actor) & 4 != 0 {
            if kh2(Kind::GroundCombo) {
                swing_travel(actor, &mut *disp, slot.unwrap_or(0));
            }
            return r;
        }
        if let Ok(mut s) = SWING.try_lock() {
            *s = None;
        }
        // (KH1's walk and run only: not one of its actions, flag 4, an item's use.)
        if !kh2(Kind::MiscAnims) || rd::<u32>(actor) & 4 != 0 {
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

    /// KH1's ground swing (its slot `slot`) moved as KH2's swing for it moves him: KH2's clip's
    /// root travel (`Clip::root`, +Z forward, +X right), stretched over KH1's swing as its motion
    /// is (`pose::follow`), along his facing. KH1's own travel this frame (its swing's step
    /// forward, its dash at a target) is replaced: one owner state (KH1's swing, its hit objects
    /// and timing), KH2's physics on it.
    unsafe fn swing_travel(actor: usize, d: &mut [f32; 4], slot: usize) {
        let Some(kh2) = super::super::pose::kh2() else { return };
        let Some(motion) = super::super::pose::swing_motion(slot) else { return };
        let Some(clip) = kh2.character.moveset.clips.get(&motion) else { return };
        let kh1_len = super::super::pose::slot_frames(slot).unwrap_or(0) as f32;
        if clip.root.len() < 2 || kh1_len < 1.0 {
            return;
        }
        let Ok(mut g) = SWING.try_lock() else { return };
        let s = match *g {
            Some((s, t)) if s == slot => (s, t),
            _ => (slot, 0.0),
        };
        let dt = dt();
        let k = clip.length.max(1) as f32 / kh1_len;
        let at = |t: f32| -> [f32; 3] {
            let u = (t * k).clamp(0.0, (clip.root.len() - 1) as f32);
            let i = (u as usize).min(clip.root.len() - 2);
            let w = u - i as f32;
            let (a, b) = (clip.root[i], clip.root[i + 1]);
            [a[0] + (b[0] - a[0]) * w, a[1] + (b[1] - a[1]) * w, a[2] + (b[2] - a[2]) * w]
        };
        let (a, b) = (at(s.1), at(s.1 + dt));
        *g = Some((s.0, s.1 + dt));
        let local = [b[0] - a[0], b[2] - a[2]];
        let yaw: f32 = rd(actor + 0x3C);
        if !yaw.is_finite() || !local.iter().all(|x| x.is_finite()) {
            return;
        }
        // (KH2's world turned by his facing as the controller does, `fo.root`; KH1's x / z are
        // KH2's.)
        let fw = [yaw.sin(), yaw.cos()];
        let right = [fw[1], -fw[0]];
        d[0] = local[0] * right[0] + local[1] * fw[0];
        d[2] = local[0] * right[1] + local[1] * fw[1];
    }

    /// The glide's pre-step (KH1 picks its glide slot and turns him toward the stick): KH2's turn
    /// rate in place of KH1's (Glide at KH2).
    unsafe extern "system" fn glide_pre(actor: usize, input: usize) -> usize {
        let f: PreFn = std::mem::transmute(ORIG_GLIDE_PRE.load(Ordering::Acquire));
        let sora = is_sora(actor);
        let before: f32 = if sora { rd(actor + 0x3C) } else { 0.0 };
        let r = f(actor, input);
        if !sora || rd::<i32>(actor + 0x70) != GLIDE_STATE as i32 || input < 0x10000 {
            return r;
        }
        // (The input: the stick's direction x, y, z, then its tilt.)
        let i: [f32; 4] = rd(input);
        GLIDE_TILT.store(if i[3].is_finite() { i[3].clamp(0.0, 1.0) } else { 0.0 }.to_bits(), Ordering::Relaxed);
        if !kh2(Kind::Glide) || !before.is_finite() || !(i[3] > 0.0) || !i[0].is_finite() || !i[2].is_finite() {
            return r;
        }
        let Some(g) = numbers().and_then(|n| n.glide) else { return r };
        let want = i[0].atan2(i[2]);
        let step = (g.turn * dt()).max(0.0);
        let d = wrap(want - before);
        let facing = wrap(before + d.clamp(-step, step));
        let set: unsafe extern "system" fn(usize, f32) = std::mem::transmute(base() + SET_FACING_RVA);
        set(actor, facing);
        wr(actor + 0x150, facing);
        r
    }

    /// The glide's step (KH1: along his facing, speeding up to its top speed, a steady fall): KH2's
    /// speed (`(1 - a) × top × tilt + a × last`) and fall (by the height over the floor) in place
    /// of KH1's (Glide at KH2). KH1 keeps its glide's start and end, its slots and collision.
    unsafe extern "system" fn glide_step(actor: usize, disp: *mut [f32; 4]) -> usize {
        let f: StepFn = std::mem::transmute(ORIG_GLIDE.load(Ordering::Acquire));
        let sora = is_sora(actor);
        let v0: [f32; 4] = if sora { rd(actor + 0x3C0) } else { [0.0; 4] };
        let r = f(actor, disp);
        if !sora || rd::<i32>(actor + 0x70) != GLIDE_STATE as i32 || disp.is_null() || !kh2(Kind::Glide) {
            return r;
        }
        let Some(g) = numbers().and_then(|n| n.glide) else { return r };
        let dt = dt();
        let tilt = f32::from_bits(GLIDE_TILT.load(Ordering::Relaxed));
        let last = v0[0].hypot(v0[2]);
        let last = if last.is_finite() { last } else { 0.0 };
        let speed = ((1.0 - g.acceleration) * g.speed * tilt + g.acceleration * last).max(0.0);
        let fw: [f32; 3] = rd(actor + 0x30);
        let n = fw[0].hypot(fw[2]);
        let dir = if n.is_finite() && n > 1e-3 {
            [fw[0] / n, fw[2] / n]
        } else {
            let yaw: f32 = rd(actor + 0x3C);
            [yaw.sin(), yaw.cos()]
        };
        let above: f32 = rd(actor + 0x3D4);
        let above = if above.is_finite() { above } else { 0.0 };
        let fall = ((above - g.fall_height).max(0.0) * g.fall_ratio).min(g.fall_max).max(0.0);
        let d = &mut *disp;
        d[0] = dir[0] * speed * dt;
        d[2] = dir[1] * speed * dt;
        // (KH1's world is y-down.)
        d[1] = fall * dt;
        wr(actor + 0x3C0, [dir[0] * speed, d[1], dir[1] * speed, speed]);
        r
    }

    fn wrap(a: f32) -> f32 {
        let t = std::f32::consts::TAU;
        let a = a.rem_euclid(t);
        if a > std::f32::consts::PI {
            a - t
        } else {
            a
        }
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
        // without it), from the speed before KH1's own steering this frame. (Both are per 60 Hz
        // frame: `dt` of them a game frame, 2 at 30 fps.)
        let per = |k: f32| if dt == 1.0 { k } else { k.powf(dt) };
        let (air_keep, air_drag) = (per(n.air_keep), per(n.air_drag));
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
            v = [v[0] * air_keep + t[0] * (1.0 - air_keep), v[1] * air_keep + t[1] * (1.0 - air_keep)];
        } else {
            v = [v[0] * air_drag, v[1] * air_drag];
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
