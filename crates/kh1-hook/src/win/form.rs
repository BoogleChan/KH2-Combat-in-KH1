//! Wisdom Form (the user: "lets add wisdom form", from the second command page's Drive, costing
//! MP, Sora recoloured blue). Chosen from `drive`'s list, it plays out as KH2's Drive does
//! (`kh2-bevy`'s `drive.rs`, from the exe):
//! - Sora's Drive motion (52, invulnerable) with base Sora's moveset; when it ends, the swap:
//!   the form's moveset, motions and combo rows (`P_EX100_MAGF`, moveset 3), his facing and
//!   lock kept, and its arrival (53). Its abilities: Sora's support abilities, not his action
//!   or growth ones, and the form's own growth ability, Quick Run, one level above Sora's (the user);
//! - the form runs on Sora's MP (the user): its yellow part drained bit by bit, a blue bar
//!   broken into it when it runs out (KH1's own payment; `after_step`); out of MP, or on the
//!   Revert command (the Drive row itself in the form), back to Sora, at once, in the air too:
//!   what was playing is cut and he is free again with his movement kept (walking, running or
//!   idle by the stick; in the air, falling). Knocked out: back at once;
//! - Drive and Revert are taken only on a frame Sora can change (`Character::can_change_form`:
//!   free, jumping, falling, landing, or at the end of a move or a cast), never buffered: a
//!   press before that does nothing (the menu refuses it, `combat::run_entry`);
//! - the change takes 45 frames (the user): Sora's Drive motion for `DRIVE_FRAMES`, then the
//!   swap and the form's short arrival; in the air he is held where he is through it.
//! - its shots and its spells: `formmagic`.
//! The cost is MP (`cost_mp`, KH1's own payment), not a Drive gauge, which KH1 has none of.
//!
//! Valor Form (stage 211, the user: "i want to try to add valor form"): the same change and
//! revert, `P_EX100_BTLF` (moveset 2, its two Keyblades' swings), its growth ability High Jump
//! (at MAX, as Wisdom's Quick Run), no magic (KH2's Valor can't cast), and its second Keyblade
//! (`valor`): entered only with an eligible one (the user: "If no keyblade is eligible to be in
//! that slot, dont let players go into valor form").
//!
//! Final Form (stage 231, the user: "lets work on implementing final form ... make it a secret"):
//! the same change and revert, `P_EX100_ULTF` (moveset 5), its growth ability Glide at MAX, its
//! two Keyblades floating where KH2's own weapon motions put them (the second always the Kingdom
//! Key, no menu slot: the user, "don't add the final form keyblade selection into the menu like we
//! did for valor, just default it to kingdom key"). Not in the Drive list: a secret button
//! code on the pad (`pad`) asks for it, as the Drive list would. Its magic is Wisdom's (the user:
//! "bring it to final form, with final form's animations"): `Kind::magic_sub`, `formmagic`.
//! (Since s271 Final is in the Drive list and the code asks for Anti Form.)
//!
//! Anti Form (the user: "that cheat code we were using ... I want you to add anti form. I want
//! 1:1 combat parity. I dont want it to cost any MP to enter, and I dont want sora to be able to
//! revert out of it"; since s285 it takes half his MP: "Antiform should take half of your MP"): reached only by the secret code (`pad`), never from the Drive list
//! (`Kind::EVERY`, not `Kind::ALL`). `P_EX100_HTLF` (moveset 6): its own moveset, motions, combo
//! rows, movement numbers, effects and sounds, no Keyblade (KH2's Anti holds none). As KH2's:
//! - no magic, items, summons or KH1 specials (`combat`), no guard (no motion 42 in its set), no
//!   growth ability (00battle `fmlv`'s Anti rows give none, so `growth_up` doesn't apply);
//! - 1.5 times the damage taken (`hurt`, `character_core::drive::DriveParams::anti_damage`);
//! - no Revert: the Drive row greyed and refused (`usable`, `choose`, `drive`);
//! - Square (the user: "lets add anti-form's square move"): Anti Glide (row 11, motion 173) on the
//!   ground, standing or moving; Side Spin (row 12, motion 161) in a ground combo
//!   (`ANTI_SIDE_SPIN_ROW`). Neither guards.
//! It takes half his MP to enter (`cost_mp`), and it runs on Sora's MP as every form here does (the user: "Drive
//! forms in this mod run out when you're out of MP, so sora will revert when he is out of MP"):
//! the same drain (`after_step`), the same 2 extra bars (`valorstats::DRIVE_MP`), and out of MP
//! (or knocked out) back to Sora.
//!
//! Limit Form (the user: "lets begin working on limit form"; `docs/drive-model.md`, "Limit Form in
//! the KH1 port"): in the Drive list after Wisdom, the same change and revert, `P_EX100_KH1F`
//! (moveset 0x42): its own moveset, motions, combo rows (Zantetsuken, Ripple Drive and Hurricane
//! Period as its finishers, the Zantetsu Counter from its guard), effects and sounds, and one
//! Keyblade (Sora's own, as in base form). Its growth ability is Dodge Roll, one level above
//! Sora's (as the others'). It keeps KH1 Sora's own look (no tint: the model drawn is KH1's
//! already). No magic: KH2's Limit Form has its Limit commands in place of Magic (Sonic Rave, Last
//! Arcanum, Strike Raid, Infinity), run by its model's own script (`limi`): `limits`, the Limit
//! menu in Magic's place (Sonic Rave and Last Arcanum so far). It costs `LIMIT_COST_MP` and opens at level `LIMIT_LEVEL` (the
//! user).

use super::{log, pose};
use character_core::{Abilities, Character, EventKind};
use kh2_formats::battle::Battle;
use kh2_rig::RigAnim;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

pub use super::formmagic::{element_sounds, package_spell, shots_step, spell_file, LAST_PACKAGE, SPELL_NAMES};

pub const DRIVE_MOTION: u16 = 52;
pub const ARRIVE_MOTION: u16 = 53;
// (No revert motion: base Sora's motion 4, which the old revert played, is his fall loop (A004),
// and left him in KH2's fall on the ground, the user.)
/// Frames of Sora's Drive motion before the swap (its own runs 160; the user: "it should only
/// take 45 frames ... from start to end"): with the form's arrival (motion 53, 3 frames at 30
/// fps: 6), 45 in all.
const DRIVE_FRAMES: u32 = 39;
/// The revert's light and sound: Sora's package elements 15..27 (no motion of his starts them;
/// they mirror the Drive motion's 1..13, element 15 with sound 115 as element 1 has the change's
/// 114; Data, KH2's own call for them not traced), started on him as he comes back (the user:
/// "missing its sfx sound and effect when reverting back from a drive form").
pub const REVERT_ELEMENTS: [i16; 13] = [15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27];
/// The motion id the revert's effects and sound are started under (no motion of Sora's has it).
pub const REVERT_FX: u16 = u16::MAX - 2;
/// MP to change (KH1's payment takes hundredths of a bar from 100 up), with `have` MP now:
/// Valor and Wisdom 1 (the user: "have valor and wisdom forms cost 1 MP"), Final 3, Anti half of
/// what he has, rounded down (the user: "Antiform should take half of your MP").
fn cost_mp(kind: Kind, have: i32) -> i32 {
    match kind {
        Kind::Valor | Kind::Wisdom => 1,
        Kind::Limit => LIMIT_COST_MP,
        Kind::Final => 3,
        Kind::Anti => have.max(0) / 2,
    }
}

/// Limit Form's MP cost (the user: 1 MP to enter, as Valor and Wisdom).
pub const LIMIT_COST_MP: i32 = 1;

/// Anti Form's Side Spin: its PTYA row 12 (motion 161), Square in a ground combo (Data).
const ANTI_SIDE_SPIN_ROW: u8 = 12;

/// Whether `have` MP lets form `kind` start: its cost and some left to run on. Anti starts at any
/// MP (it takes half; its `valorstats::DRIVE_MP` bars are there to run on).
fn mp_enough(have: i32, kind: Kind) -> bool {
    kind == Kind::Anti || have > cost_mp(kind, have)
}
/// The form's level (KH1 has no form experience): Lv 1, 40 s of battle.
const LEVEL: u32 = 1;
/// Frames of battle per unit of yellow MP the form uses up (half as fast out of battle). A blue
/// bar breaks into its number × 30 units (KH1's own conversion), so the top bar lasts that many
/// units.
/// (3: the user, "lets also have drive forms drain a little slower"; it was 2.)
const DRAIN_FRAMES: u32 = 3;
/// Upper Slash's ability (KH2's id): Sora's Square launch, given to Valor (`pose::port_moves`).
const UPPER_SLASH: u16 = 95;

/// The forms (`drive::FORMS` in the same order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Valor,
    Wisdom,
    /// `P_EX100_KH1F`, KH1 Sora's look.
    Limit,
    Final,
    /// Only by the secret code (`pad`), never in the Drive list.
    Anti,
}

impl Kind {
    /// The Drive list's (Final too now, the user: "put it into the drive menu"; Limit after
    /// Wisdom, as in KH2's list).
    pub const ALL: [Kind; 4] = [Kind::Valor, Kind::Wisdom, Kind::Limit, Kind::Final];
    /// Every form (Anti too, the secret code's), in the enum's order (`code`).
    const EVERY: [Kind; 5] = [Kind::Valor, Kind::Wisdom, Kind::Limit, Kind::Final, Kind::Anti];
    pub fn name(self) -> &'static str {
        match self {
            Kind::Valor => "Valor Form",
            Kind::Wisdom => "Wisdom Form",
            Kind::Limit => "Limit Form",
            Kind::Final => "Final Form",
            Kind::Anti => "Anti Form",
        }
    }
    fn code(self) -> u8 {
        self as u8 + 1
    }
    fn from_code(c: u8) -> Option<Kind> {
        Kind::EVERY.get((c as usize).checked_sub(1)?).copied()
    }
    /// Its KH2 data, if it loaded.
    pub fn data(self, kh2: &pose::Kh2) -> Option<&pose::Form> {
        match self {
            Kind::Valor => kh2.valor.as_ref(),
            Kind::Wisdom => kh2.wisdom.as_ref(),
            Kind::Limit => kh2.limit.as_ref(),
            Kind::Final => kh2.final_form.as_ref(),
            Kind::Anti => kh2.anti.as_ref(),
        }
    }
    /// With a second Keyblade (Valor's slot's).
    pub fn two_blades(self) -> bool {
        matches!(self, Kind::Valor | Kind::Final)
    }
    /// KH2's magic sub offset in the form (`docs/drive-model.md`: + 10 in Wisdom, + 30 in Final;
    /// 03system `magi` and the spells' ATKP rows have subs 30..35 for Final), for a form that casts
    /// as Wisdom does (`formmagic`); `None` for one that can't cast (Valor, Limit, Anti: the
    /// command's form mask; Limit has its Limit commands instead, `limits`).
    /// (Final: the user, "lets try to incorporate our rules, logic, and process from wisdom forms'
    /// magic, and bring it to final form, with final form's animations". KH2 has no spell files
    /// of Final's own: MAGC's rows are the same as Sora's, and the form's moveset gives the
    /// motions under its ids, `P_EX100_ULTF`'s E36x / E37x.)
    pub fn magic_sub(self) -> Option<i32> {
        match self {
            Kind::Valor | Kind::Limit | Kind::Anti => None,
            Kind::Wisdom => Some(10),
            Kind::Final => Some(30),
        }
    }
}

/// A request: 1 a form (`WANT`), 2 revert (the menu), 3 knocked out (`hurt`).
static REQUEST: AtomicU8 = AtomicU8::new(0);
/// The form asked for (`Kind::code`).
static WANT: AtomicU8 = AtomicU8::new(0);
/// The form he is in (`Kind::code`, 0 none).
static CURRENT: AtomicU8 = AtomicU8::new(0);
/// In a form (the menu's entry reads "Revert"), or changing into one.
static IN_FORM: AtomicBool = AtomicBool::new(false);
static CHANGING: AtomicBool = AtomicBool::new(false);

/// Sora went down (`hurt`): out of the form at once, when KH2 next has him.
pub fn knocked_out() {
    if in_form() || CHANGING.load(Ordering::Relaxed) {
        REQUEST.store(3, Ordering::Relaxed);
    }
}

pub fn in_form() -> bool {
    IN_FORM.load(Ordering::Relaxed)
}

/// The form he is in.
pub fn current() -> Option<Kind> {
    if !in_form() {
        return None;
    }
    Kind::from_code(CURRENT.load(Ordering::Relaxed))
}

/// In Wisdom Form (its shots, its spells, its look).
pub fn in_wisdom() -> bool {
    current() == Some(Kind::Wisdom)
}

/// In Valor Form (no magic).
pub fn in_valor() -> bool {
    current() == Some(Kind::Valor)
}

/// In Limit Form (no magic; its own effects and sounds).
pub fn in_limit() -> bool {
    current() == Some(Kind::Limit)
}

/// In Final Form.
pub fn in_final() -> bool {
    current() == Some(Kind::Final)
}

/// In Anti Form (no magic, items, summons or specials, no Revert; 1.5 times the damage).
pub fn in_anti() -> bool {
    current() == Some(Kind::Anti)
}

/// The form he is in, if it casts as Wisdom does (Wisdom, Final: `Kind::magic_sub`): its Fire,
/// Blizzard and Thunder KH2's (`formmagic`), its Cure, Gravity, Stop and Aero KH1's on its own
/// cast motions (`takeover::command_started`, `pose::follow`).
pub fn casting_form() -> Option<Kind> {
    current().filter(|k| k.magic_sub().is_some())
}

/// In a form with a second Keyblade (Valor, Final).
pub fn two_blades() -> bool {
    current().is_some_and(|k| k.two_blades())
}

/// The secret code, kept unreadable on purpose (the user: it stays a secret): `code_bit`.
const CODE: [u8; 10] = [94, 94, 92, 92, 93, 95, 93, 95, 87, 84];

/// The pad bit of the code's `i`th button.
fn code_bit(i: usize) -> u16 {
    1 << (CODE[i] ^ 0x5A)
}
/// The buttons the code watches (any other pressed starts it over), and the longest gap.
const CODE_BUTTONS: u16 = (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 13) | (1 << 14) | (1 << 12) | (1 << 15);
const CODE_GAP_MS: u64 = 1500;
/// The code so far: buttons held last time, how far in, when the last one came; and whether it was
/// just completed (taken on the game thread, `before_step`).
static CODE_STATE: std::sync::Mutex<(u16, usize, Option<std::time::Instant>)> = std::sync::Mutex::new((0, 0, None));
static CODE_DONE: AtomicBool = AtomicBool::new(false);

/// The pad as pressed, every read (`probe::pad`): the secret code, once for Final Form (the user:
/// a button sequence, "in that exact sequence, put them into final form"), now for Anti Form
/// (the user: "that cheat code we were using ... I want you to add anti form").
pub fn pad(buttons: u16) {
    let Ok(mut g) = CODE_STATE.try_lock() else { return };
    let (held, at, last) = &mut *g;
    let pressed = buttons & !*held & CODE_BUTTONS;
    *held = buttons;
    if pressed == 0 {
        return;
    }
    if last.is_some_and(|t| t.elapsed().as_millis() as u64 > CODE_GAP_MS) {
        *at = 0;
    }
    *last = Some(std::time::Instant::now());
    for bit in (0..16).map(|i| 1u16 << i).filter(|b| pressed & b != 0) {
        if bit == code_bit(*at) {
            *at += 1;
        } else {
            *at = usize::from(bit == code_bit(0));
        }
        if *at == CODE.len() {
            *at = 0;
            CODE_DONE.store(true, Ordering::Relaxed);
            log("Anti Form: the secret code entered");
        }
    }
}

pub fn busy() -> bool {
    CHANGING.load(Ordering::Relaxed) || REQUEST.load(Ordering::Relaxed) != 0
}

/// KH2's magic sub offset now (`reflect`, `formmagic`): the form's (`Kind::magic_sub`), else 0.
pub fn magic_sub() -> i32 {
    current().and_then(Kind::magic_sub).unwrap_or(0)
}

unsafe fn rd<T: Copy>(p: usize) -> T {
    std::ptr::read_unaligned(p as *const T)
}

unsafe fn mp(sora: usize) -> i32 {
    let stats = pose::ptr(rd::<u32>(sora + 0x6C));
    if stats < 0x10000 {
        return 0;
    }
    rd::<i32>(stats + 0x44)
}

/// KH1's HP change `(actor, amount)` (`0x1402A2790`: clamped to the maximum, its HUD mirror kept).
const HP_ADD_RVA: super::addr::Rva = super::addr::Rva(0x2A2790);

/// Sora's HP back to full as a Drive starts (the user: "activating a drive form should restore
/// soras health entirely", as KH2's does).
unsafe fn heal_full(sora: usize) {
    let stats = pose::ptr(rd::<u32>(sora + 0x6C));
    if stats < 0x10000 {
        return;
    }
    let (now, max) = (rd::<i32>(stats + 0x3C), rd::<i32>(stats + 0x40));
    if max > now && now > 0 {
        let base = super::sys::GetModuleHandleA(std::ptr::null()) as usize;
        let add: unsafe extern "system" fn(usize, i32) -> u32 = std::mem::transmute(base + HP_ADD_RVA);
        add(sora, max - now);
        log(&format!("Drive: HP {now} -> {} of {max}", rd::<i32>(stats + 0x3C)));
    }
}

unsafe fn hp(sora: usize) -> i32 {
    let stats = pose::ptr(rd::<u32>(sora + 0x6C));
    if stats < 0x10000 {
        return 1;
    }
    rd::<i32>(stats + 0x3C)
}

/// Whether a Drive or Revert pressed now is taken (`combat::run_entry`; the user: "return
/// should only be accepted when you press return on an end frame", no queue): KH2 has Sora and
/// he can change on this frame.
pub fn accept_now() -> bool {
    // (Or in one of KH1's moves at its cut frame, out of a form: the user, "goes into the drive
    // submenu and selects a drive" from frame 49 of KH1's Fire.)
    !busy() && (super::takeover::with_kh2(|c, _| c.can_change_form()).unwrap_or(false) || !in_form() && super::takeover::kh1_cut_window())
}

/// Whether form `kind`'s entry can be chosen now (`drive`'s menu): reverting in a form (not in
/// Anti Form, which has no Revert), else MP for it and KH2 having Sora, its data loaded, and what
/// the form itself needs (Valor's second Keyblade, `valor`).
pub unsafe fn usable(sora: usize, kind: Kind) -> bool {
    if busy() {
        return false;
    }
    if in_form() {
        // (The user: "I dont want sora to be able to revert out of it".)
        return !in_anti();
    }
    unlocked(kind) && pose::kh2().is_some_and(|k| kind.data(k).is_some()) && (super::takeover::kh2_has() || super::takeover::kh1_cut_window()) && sora > 0x10000 && mp_enough(mp(sora), kind) && own_needs(kind).is_ok()
}

/// Sora's save (`menu::save_block`): his level (`+4`), the Dive to the Heart's choice (`+0x48C`,
/// set by `0x1402835B0`; it picks his level-up table at `0x142529A10`), and Traverse Town's story
/// progress (`+0x1504`, the worlds' bytes `menu` reads for the bosses' gifts; 0x2B and up: Guard
/// Armor beaten, as gaithern's KH-1FM-AP-LUA reads it).
const SAVE_LEVEL: usize = 4;
const SAVE_CHOICE: usize = 0x48C;
const SAVE_TT_PROGRESS: usize = 0x1504;
const GUARD_ARMOR_BEATEN: u8 = 0x2B;
/// The choice's values, taken in the dream weapons' order (Sword 0x52, Shield 0x53, Rod 0x54).
const CHOSE_ROD: u8 = 2;
/// The other "first" form's level, and Final's (the user: "for the time being, lets have final
/// form be unlocked at level 99 ... whichever form is the opposite ... unlocked at level 98").
const SECOND_FORM_LEVEL: u8 = 98;
const FINAL_LEVEL: u8 = 99;
/// Limit Form's level (the user: unlocked "at level 97", by the same level check).
pub const LIMIT_LEVEL: u8 = 97;

/// The form is open to Sora (the Drive list's; Anti is the code's, always). The user: "if the
/// player picked the sword or the shield as their main power in the beginning of the game, have
/// valor form be their first drive form. it should unlock after defeating guard armor. likewise,
/// if the player picked wand, wisdom form should be their first drive form".
pub unsafe fn unlocked(kind: Kind) -> bool {
    let base = super::sys::GetModuleHandleA(std::ptr::null()) as usize;
    let Some(save) = super::menu::save_block(base) else { return false };
    if !super::crash::readable(save + SAVE_TT_PROGRESS) {
        return false;
    }
    let level = rd::<u8>(save + SAVE_LEVEL);
    let choice = rd::<u8>(save + SAVE_CHOICE);
    let guard_armor = rd::<u8>(save + SAVE_TT_PROGRESS) >= GUARD_ARMOR_BEATEN;
    let first = if choice == CHOSE_ROD { Kind::Wisdom } else { Kind::Valor };
    let open = match kind {
        Kind::Anti => true,
        Kind::Limit => level >= LIMIT_LEVEL,
        Kind::Final => level >= FINAL_LEVEL,
        k if k == first => guard_armor || level >= SECOND_FORM_LEVEL,
        _ => level >= SECOND_FORM_LEVEL,
    };
    static LAST: AtomicU32 = AtomicU32::new(u32::MAX);
    let key = (level as u32) << 16 | (choice as u32) << 8 | guard_armor as u32;
    if LAST.swap(key, Ordering::Relaxed) != key {
        log(&format!("Drive forms: Dive choice {choice} ({}), level {level}, Guard Armor {}: first form {}", ["Sword", "Shield", "Rod"].get(choice as usize).unwrap_or(&"?"), if guard_armor { "beaten" } else { "not yet" }, first.name()));
    }
    open
}

/// What a form needs besides MP: Valor its second Keyblade (`valor::keyblade`).
unsafe fn own_needs(kind: Kind) -> Result<(), String> {
    match kind {
        Kind::Valor => super::valor::keyblade().map(|_| ()).ok_or_else(|| "no Keyblade for the second hand (one Sora owns that isn't equipped)".into()),
        Kind::Wisdom | Kind::Limit | Kind::Final | Kind::Anti => Ok(()),
    }
}

/// Chosen from the Drive list (`drive::chosen`), or Anti Form by the secret code. Returns why
/// not, if refused.
pub unsafe fn choose(sora: usize, kind: Kind) -> Result<&'static str, String> {
    if busy() {
        return Err("a change is under way".into());
    }
    if in_anti() {
        // No Revert in Anti Form (the user: "I dont want sora to be able to revert out of it").
        return Err("Anti Form has no Revert".into());
    }
    if in_form() {
        REQUEST.store(2, Ordering::Relaxed);
        return Ok("Revert");
    }
    if !pose::kh2().is_some_and(|k| kind.data(k).is_some()) {
        return Err(format!("{}'s files didn't load", kind.name()));
    }
    own_needs(kind)?;
    if !unlocked(kind) {
        return Err(format!("{} isn't unlocked yet", kind.name()));
    }
    let cut = super::takeover::kh1_cut_window();
    if !super::takeover::kh2_has() && !cut {
        return Err("KH1 has Sora".into());
    }
    let have = mp(sora);
    if !mp_enough(have, kind) {
        return Err(format!("MP {have}: it costs {} and needs some left to run on", cost_mp(kind, have)));
    }
    WANT.store(kind.code(), Ordering::Relaxed);
    REQUEST.store(1, Ordering::Relaxed);
    if cut && !super::takeover::kh2_has() {
        // (KH2 takes him out of KH1's move this frame and changes him.)
        super::takeover::DRIVE_CUT.store(true, Ordering::Relaxed);
    }
    Ok(kind.name())
}

/// The Driver's form state (`takeover`).
#[derive(Default)]
pub struct Drive {
    /// Changing: Sora's Drive motion playing, its frames.
    change: Option<u32>,
    /// In the form: battle frames left, Sora's own abilities (back on revert), the form's.
    form: Option<InForm>,
    /// Out of MP: a revert as soon as he can change.
    revert: bool,
    /// The form the change under way goes to.
    changing_to: Option<Kind>,
}

struct InForm {
    kind: Kind,
    /// Half frames toward the next MP drained (2 a frame in battle, 1 out of it).
    left: u32,
    sora: Abilities,
    own: Abilities,
    logged_s: u32,
    /// KH1's world when the form started (`WORLD_RVA`): in another one, back to Sora.
    world: u32,
}

/// KH1's current world (u32; `takeover`'s room line reads it too).
const WORLD_RVA: super::addr::Rva = super::addr::Rva(0x2340E5C);

unsafe fn world_now() -> u32 {
    let base = super::sys::GetModuleHandleA(std::ptr::null()) as usize;
    rd::<u32>(base + WORLD_RVA)
}

/// The form's controller data while in it, else Sora's.
pub fn character<'a>(d: &Drive, kh2: &'a pose::Kh2) -> &'a Character {
    match d.form.as_ref().and_then(|f| f.kind.data(kh2)) {
        Some(f) => &f.character,
        None => &kh2.character,
    }
}

pub fn battle<'a>(d: &Drive, kh2: &'a pose::Kh2) -> &'a Arc<Battle> {
    match d.form.as_ref().and_then(|f| f.kind.data(kh2)) {
        Some(f) => &f.battle,
        None => &kh2.battle,
    }
}

/// A growth ability's level (0 none, 1..3, 4 MAX) in a form: one above base Sora's, MAX at most
/// (the user: "have the grown abilities that are associated with each drive form ... always be
/// one level higher than the level unlocked by base sora": none -> LV1, LV1 -> LV2, MAX stays
/// MAX).
fn growth_up(level: u8) -> u8 {
    level.saturating_add(1).min(GROWTH_MAX)
}
const GROWTH_MAX: u8 = 4;

/// The form's abilities (KH2's, exe): Sora's support abilities carry over, his action and
/// growth ones don't; the form's own growth ability at its level; every combo row of the form's
/// table unlocked.
fn form_abilities(sora: &Abilities, form: &Character, kind: Kind) -> Abilities {
    let mut a = Abilities {
        // (Never in Anti Form: the user, "you shouldnt be able to guard in anti form".)
        guard: kind != Kind::Anti && form.moveset.clips.contains_key(&character_core::motion::GUARD),
        // (Sora's Aerial Recovery in the form, with its own recovery motion: the user, Wisdom and
        // Valor were "missing aerial recovery".)
        aerial_recovery: sora.aerial_recovery && form.moveset.clips.contains_key(&character_core::motion::AIR_RECOVERY),
        combo_plus: sora.combo_plus,
        air_combo_plus: sora.air_combo_plus,
        finishing_plus: sora.finishing_plus,
        negative_combo: sora.negative_combo,
        combo_master: sora.combo_master,
        one_swing_on_miss: sora.one_swing_on_miss,
        // Each form's growth ability (Valor High Jump, Wisdom Quick Run, Limit Dodge Roll, Final
        // Glide) one level above base Sora's, MAX at most (`growth_up`). Anti has none (00battle `fmlv`'s Anti
        // rows give no ability at any level), so none of Sora's growth abilities either.
        high_jump: if kind == Kind::Valor { growth_up(sora.high_jump) } else { 0 },
        quick_run: if kind == Kind::Wisdom { growth_up(sora.quick_run) } else { 0 },
        aerial_dodge: 0,
        glide: if kind == Kind::Final { growth_up(sora.glide) } else { 0 },
        dodge_roll: if kind == Kind::Limit { growth_up(sora.dodge_roll) } else { 0 },
        ..sora.clone()
    };
    a.equipped = form.moveset.combo_rows.iter().map(|r| r.ability).filter(|&x| x != 0).collect();
    // (Sora's own moves added to the form, `pose::port_moves`: only while he has them equipped.)
    if kind == Kind::Valor {
        a.equipped.retain(|&x| x != UPPER_SLASH || sora.equipped.contains(&UPPER_SLASH));
    }
    a
}

/// Replaces the controller (keeps facing, lock-on, the glide button, and his movement: speed,
/// fall, in the air or not).
fn swap(ch: &mut Character, anim: &mut RigAnim, data: &Character, rig: &Arc<kh2_data::sora::Rig>, abilities: Abilities) {
    let (facing, lock, glide) = (ch.facing(), ch.lock(), ch.glide_button);
    let mut c = data.clone();
    c.carry_movement(ch);
    c.abilities = abilities;
    c.glide_button = glide;
    c.air_magic_falls = ch.air_magic_falls;
    c.air_magic_gravity = ch.air_magic_gravity;
    c.set_facing(facing);
    c.set_lock(lock);
    *ch = c;
    let field = anim.field;
    *anim = RigAnim::new(rig.clone());
    anim.queued_blend = 14.0;
    anim.field = field;
}

/// Before KH2's step: the menu's request, the change under way, the form's end.
pub unsafe fn before_step(d: &mut Drive, ch: &mut Character, anim: &mut RigAnim, kh2: &pose::Kh2, actor: usize) {
    // Knocked out in the form: Sora again at once (KH2: a KO reverts).
    let knocked = REQUEST.load(Ordering::Relaxed) == 3 && REQUEST.swap(0, Ordering::Relaxed) == 3;
    if (d.form.is_some() || d.change.is_some()) && (knocked || hp(actor) <= 0) {
        revert(d, ch, anim, kh2, "knocked out");
        return;
    }
    // Landed in another world in the form: Sora again at once, wherever he lands (the user: "when
    // leaving a world in a drive form, wherever world sora lands, he should be automatically
    // reverted"). Anti Form too.
    if let Some(w) = d.form.as_ref().map(|f| f.world) {
        let now = world_now();
        if now != w {
            log(&format!("Drive: a new world ({w} -> {now}): back to Sora"));
            revert(d, ch, anim, kh2, "a new world");
            return;
        }
    }
    // The secret code (`pad`): Anti Form (it asked for Final Form until s271), as if chosen from
    // the Drive list (KH2 must be able to change him now, as for the list: `accept_now`: free,
    // jumping, falling, landing or at a move's end; not in a form).
    if CODE_DONE.swap(false, Ordering::Relaxed) {
        if d.form.is_some() || d.change.is_some() || !ch.can_change_form() {
            log("Anti Form: not now (in a form, changing, or mid-move)");
        } else {
            match choose(actor, Kind::Anti) {
                Ok(_) => log("Anti Form: asked for by the secret code"),
                Err(e) => log(&format!("Anti Form: refused ({e})")),
            }
        }
    }
    match REQUEST.swap(0, Ordering::Relaxed) {
        1 if d.form.is_none() && d.change.is_none() => {
            let Some(kind) = Kind::from_code(WANT.load(Ordering::Relaxed)) else { return };
            // (Taken on the frame it was pressed in, `accept_now`; in the air too, held there.)
            if !mp_enough(mp(actor), kind) {
                log(&format!("Drive: MP {} short of {} by now", mp(actor), cost_mp(kind, mp(actor))));
                return;
            }
            let air = ch.airborne();
            if !ch.play_held(DRIVE_MOTION, true) {
                log("Drive: Sora has no Drive motion (52)");
                return;
            }
            let before = mp(actor);
            let cost = cost_mp(kind, before);
            if cost > 0 {
                super::reflect::pay_mp(actor, cost * 100);
            }
            super::valorstats::drive_started();
            heal_full(actor);
            // KH1's "Give me strength!" (the user: "add the battle quote from kh1 where he says
            // "give me strength" when your transforming into a drive").
            super::kh1voice::give_me_strength(actor);
            d.change = Some(0);
            d.changing_to = Some(kind);
            CHANGING.store(true, Ordering::Relaxed);
            log(&format!("Drive: {} (MP {before} -> {}), Sora's Drive motion{}", kind.name(), mp(actor), if air { ", held in the air" } else { "" }));
        }
        // (Never in Anti Form, which has no Revert: `choose` refuses it already.)
        2 if d.form.as_ref().is_some_and(|f| f.kind != Kind::Anti) => {
            revert(d, ch, anim, kh2, "the Revert command");
            return;
        }
        _ => {}
    }
    if let Some(n) = d.change {
        let n = n + 1;
        d.change = Some(n);
        if !ch.scripted() || n >= DRIVE_FRAMES {
            d.change = None;
            arrive(d, ch, anim, kh2);
        }
        return;
    }
    if d.revert && ch.can_change_form() {
        d.revert = false;
        revert(d, ch, anim, kh2, "out of MP");
    }
}

unsafe fn arrive(d: &mut Drive, ch: &mut Character, anim: &mut RigAnim, kh2: &pose::Kh2) {
    CHANGING.store(false, Ordering::Relaxed);
    let kind = d.changing_to.take().unwrap_or(Kind::Wisdom);
    let Some(f) = kind.data(kh2) else { return };
    let sora = ch.abilities.clone();
    let own = form_abilities(&sora, &f.character, kind);
    swap(ch, anim, &f.character, &f.rig, own.clone());
    // Valor: Square in a combo goes into its finisher, row 12 (155) on the ground, 38 (183) in the
    // air (its kind-0 rows, reached only by id; the user: "valor forms ability to go into a
    // finisher by pressing square during a combo").
    if kind == Kind::Valor {
        ch.square_finishers = Some((12, 38));
    }
    // Anti: Square in a ground combo is its Side Spin, row 12 (motion 161; PTYA ability 0, so it
    // needs none), one swing of the combo as Upper Slash is (KH Wiki: "pressing Square in the
    // middle of a ground combo"; the user: "lets add anti-form's square move"). Square otherwise is
    // its Anti Glide, row 11 (motion 173), from the controller's guard-row path.
    if kind == Kind::Anti {
        ch.square_swing = Some(ANTI_SIDE_SPIN_ROW);
    }
    // (s220 showed Valor's stance in place of its landing, whose clip is Sora's own; the user,
    // s229: "valor form is currently missing kh2's landing animation": KH2's landing plays.)
    super::kh2fx::owner_gone();
    // (Still held in the air through it; then he falls.)
    if ch.clip_exists(ARRIVE_MOTION) {
        ch.play_held(ARRIVE_MOTION, true);
    }
    let lv = (sora.clone(), own.clone());
    d.form = Some(InForm { kind, left: 0, sora, own, logged_s: 0, world: world_now() });
    CURRENT.store(kind.code(), Ordering::Relaxed);
    IN_FORM.store(true, Ordering::Relaxed);
    if kind.two_blades() {
        super::valor::started();
    }
    let (growth, sora_lv, form_lv) = match kind {
        Kind::Valor => ("High Jump", lv.0.high_jump, lv.1.high_jump),
        Kind::Wisdom => ("Quick Run", lv.0.quick_run, lv.1.quick_run),
        Kind::Limit => ("Dodge Roll", lv.0.dodge_roll, lv.1.dodge_roll),
        Kind::Final => ("Glide", lv.0.glide, lv.1.glide),
        Kind::Anti => {
            log(&format!(
                "Anti Form: entered for half his MP; yellow MP drained 1 every {DRAIN_FRAMES} frames of battle (half as fast out of it), back to Sora out of MP; no growth ability, no guard ({}), no Revert, 1.5x damage taken",
                if lv.1.guard { "it has a guard motion!" } else { "no motion 42" }
            ));
            return;
        }
    };
    log(&format!("{} (Lv{LEVEL}): yellow MP drained 1 every {DRAIN_FRAMES} frames of battle (half as fast out of it), {growth} level {form_lv} (Sora's {sora_lv}, one above, 4 = MAX)", kind.name()));
}

/// Back to Sora: what played is cut, he is free with his movement (the user: on the ground
/// "return you to whatever animation is being called from the control stick and not halt his
/// movement"; in the air, falling).
unsafe fn revert(d: &mut Drive, ch: &mut Character, anim: &mut RigAnim, kh2: &pose::Kh2, why: &str) {
    CHANGING.store(false, Ordering::Relaxed);
    d.change = None;
    d.revert = false;
    d.changing_to = None;
    let Some(f) = d.form.take() else { return };
    IN_FORM.store(false, Ordering::Relaxed);
    CURRENT.store(0, Ordering::Relaxed);
    if f.kind.two_blades() {
        super::valor::ended();
    }
    swap(ch, anim, &kh2.character, &kh2.rig, f.sora);
    super::formmagic::end_all();
    super::limits::end_all();
    super::kh2fx::owner_gone();
    for el in REVERT_ELEMENTS {
        super::kh2fx::event(REVERT_FX, EventKind::Effect(el));
        super::kh2sound::event(REVERT_FX, EventKind::Effect(el));
    }
    log(&format!("Revert ({why}){}", if ch.airborne() { ", falling" } else { "" }));
}

/// After KH1's menu set Sora's abilities (`menu::Growth`): in the form, they are his own for
/// after it, and the form's stay.
pub fn abilities(d: &mut Drive, ch: &mut Character) {
    if let Some(f) = d.form.as_mut() {
        if ch.abilities != f.own {
            f.sora = ch.abilities.clone();
            // (Its own growth ability and rows stay.)
            let mut own = f.own.clone();
            own.combo_plus = f.sora.combo_plus;
            own.air_combo_plus = f.sora.air_combo_plus;
            own.finishing_plus = f.sora.finishing_plus;
            own.negative_combo = f.sora.negative_combo;
            own.combo_master = f.sora.combo_master;
            // (Its growth ability follows Sora's: one level above.)
            match f.kind {
                Kind::Valor => own.high_jump = growth_up(f.sora.high_jump),
                Kind::Wisdom => own.quick_run = growth_up(f.sora.quick_run),
                Kind::Limit => own.dodge_roll = growth_up(f.sora.dodge_roll),
                Kind::Final => own.glide = growth_up(f.sora.glide),
                // (No growth ability of its own.)
                Kind::Anti => {}
            }
            f.own = own.clone();
            ch.abilities = own;
        }
    }
}

/// After KH2's step: the look (`tint`), the MP the form uses up, its end.
///
/// (Anti Form too, though it cost nothing to enter: the user, "Drive forms in this mod run out
/// when you're out of MP, so sora will revert when he is out of MP".)
///
/// The form's gauge is Sora's MP (the user: "drive forms consume mp over time"), drained the
/// way KH1 takes a cost under one bar (the user: "drain yellow mp incrementally, when the value
/// that stores yellow mp reaches zero then we take away one bar of blue mp"): KH1's payment
/// (`0x1402A39D0`) with a cost below 100 takes it from the yellow part (stats `+0xBE`) and, when
/// that runs out, breaks the top blue bar into yellow. One unit every `DRAIN_FRAMES` in battle,
/// half as often out of it (KH2's gauge drains so); with none left, back to Sora (on the
/// ground, `before_step`).
pub unsafe fn after_step(d: &mut Drive, actor: usize, battle: bool) {
    super::tint::refresh(
        actor,
        d.form.as_ref().and_then(|f| match f.kind {
            Kind::Wisdom => Some(super::tint::Look::Wisdom),
            Kind::Valor => Some(super::tint::Look::Valor),
            Kind::Final => Some(super::tint::Look::Final),
            // (KH1 Sora's outfit recoloured, the user: "change sora's outfit in limit form to
            // have his jumpsuit be black, his belt yellow, and his shoes black".)
            Kind::Limit => Some(super::tint::Look::Limit),
            // (Shadow Sora's look, the user: "make sora look like shadow sora in this form".)
            Kind::Anti => Some(super::tint::Look::Anti),
        }),
    );
    // Valor's second Keyblade's Strength and MP (and back to Sora's own after).
    super::valorstats::step(actor);
    let Some(f) = d.form.as_mut() else { return };
    // (Valor, Final: its Keyblade checked every 10 frames, so a change made in the menu reaches
    // his hand, `valor::keyblade` / `valor::final_keyblade`.)
    if f.kind.two_blades() {
        static TICK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        if TICK.fetch_add(1, Ordering::Relaxed) % 10 == 0 {
            if f.kind == Kind::Final {
                super::valor::final_keyblade();
            } else {
                super::valor::keyblade();
            }
        }
    }
    f.left += if battle { 2 } else { 1 };
    if f.left >= DRAIN_FRAMES * 2 {
        f.left = 0;
        if super::reflect::has_mp(actor, 1) {
            let before = (mp(actor), yellow(actor));
            super::reflect::pay_mp(actor, 1);
            if mp(actor) != before.0 && f.logged_s < 40 {
                f.logged_s += 1;
                log(&format!("{}: a blue bar broken into yellow: MP {} -> {} (yellow {} -> {})", f.kind.name(), before.0, mp(actor), before.1, yellow(actor)));
            }
        }
    }
    if !super::reflect::has_mp(actor, 1) && !d.revert {
        d.revert = true;
        log(&format!("{}: out of MP", f.kind.name()));
    }
}

/// The yellow part of Sora's MP (stats `+0xBE`).
unsafe fn yellow(sora: usize) -> u16 {
    let stats = pose::ptr(rd::<u32>(sora + 0x6C));
    if stats < 0x10000 {
        return 0;
    }
    rd::<u16>(stats + 0xBE)
}

/// Sora's Drive motion: its effects and sounds play whatever the ini's lists (`kh2fx`,
/// `kh2sound`).
pub fn own_motion(motion: u16) -> bool {
    matches!(motion, DRIVE_MOTION | REVERT_FX)
}

/// Whether this frame's event goes to the form instead of Sora's effects: Wisdom's shots,
/// Valor's elements (its own archive's, `P_EX100_BTLF.a.us`: `effect`).
pub fn takes(d: &Drive, e: &EventKind) -> bool {
    d.form.is_some() && matches!(e, EventKind::Effect(_))
}

/// A frame trigger 1 of the form's motion: Wisdom's shots, Valor's (Limit's, Final's, Anti's)
/// own elements.
pub fn effect(motion: u16, n: i16) {
    if in_wisdom() {
        super::formmagic::effect(n);
    } else if in_valor() || in_limit() || in_final() || in_anti() {
        super::kh2fx::form_event(motion, n);
    }
}
