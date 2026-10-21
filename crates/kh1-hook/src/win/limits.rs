//! Limit Form's own limits, KH2's (the user: "KH2's own limits" for Limit Form, its Limit menu in
//! Magic's place). `docs/drive-model.md`, "Limit Form in the KH1 port", has what is KH2's and what
//! is ours.
//!
//! - **Menu.** In Limit Form the top's Magic row reads "Limit" (record 2's word, `word`) and its
//!   list (the magic list, `0x140529638`) holds our four records 0x82..0x85 (spell-type records
//!   with arguments `LIMIT_ARG + k`, written over empty ones as `drive`'s forms are): Sonic Rave,
//!   Last Arcanum, Strike Raid and Infinity, KH2's names. KH1's magic list shows no costs, so none
//!   is shown. A row is greyed when it can't be used now (in the air: the commands' Land flag; MP
//!   short; not in yet).
//! - **MP.** KH2's costs (03system `cmd`: 60, 75, 65, 80 of KH2's 100) are each one of KH1's bars:
//!   the mod's KH2 spells already cost KH1's own 1 MP each whatever KH2's cost (`reflect::COST`),
//!   and KH2's cost / 100 rounded up gives the same bar. Short of it, the limit can't be used (the
//!   menu greys it; KH2's own rule for a cost above what is left isn't traced, `docs`).
//! - **Running.** A limit chosen runs when KH2's move can take a command (`can_take_command`;
//!   before that the list stays open, as for a spell): KH1's bar paid, the form model's script
//!   (`limi`) started at its limit trigger with the command (`kh2_magic::Magic::limit`). The
//!   script drives Sora (`LimitWorld`): it starts and chains his motions (`Character::play`,
//!   `push_scripted`), turns him, and offers the follow-ups. The motions carry KH2's hits (the
//!   Keyblade's ATKP rows), invulnerability, effects and sounds, through the controller as for
//!   every move.
//! - **Triangle.** A follow-up the script offers (`trap_command_override`) or a motion carries
//!   (KH2's range 20) is shown in KH1's own Triangle box, the top menu's fourth entry (record
//!   0x86, its name the follow-up's), written after KH1's context fill (`context_fill`). Triangle
//!   pressed while it shows runs it (the script's trigger 23), once per window.
//! - **KH1's limits in a form.** KH1's own limits (Sonic Blade, Ars Arcanum, Strike Raid, ...)
//!   are offered by KH1 in that same Triangle box (`0x14028AF60`, called once from the context
//!   fill); in any Drive Form that call is skipped (`specials_prompt`), so they never show. The
//!   command step's refusal (`combat::command`, s278) stays behind it.

use super::addr::Rva;
use super::reflect::{world_now, KhWorld, SORA};
use super::{combat, hook, log, pose};
use character_core::{Character, Output, WindowKind};
use glam::{Mat4, Quat, Vec3};
use kh2_formats::battle::Battle;
use kh2_magic::{Event, Magic, ObjId, SpellFile, World};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// A limit: its 03system `cmd`, KH2's name, our command record and word, KH2's MP cost, and
/// whether it runs here yet.
struct Limit {
    command: u16,
    name: &'static str,
    record: u8,
    word: u16,
    kh2_mp: u8,
    ready: bool,
}

/// KH2's Limit list, in its order. (Infinity is 03system's 0x2AB, KH2 FM's Ragnarok under its
/// PC name.) Strike Raid and Infinity are listed, greyed: their scripts need the thrown Keyblade
/// and the shots, not in yet.
const LIMITS: [Limit; 4] = [
    Limit { command: 698, name: "Sonic Rave", record: 0x82, word: 0x1FA, kh2_mp: 60, ready: true },
    Limit { command: 701, name: "Last Arcanum", record: 0x83, word: 0x1F9, kh2_mp: 75, ready: true },
    Limit { command: 704, name: "Strike Raid", record: 0x84, word: 0x1F8, kh2_mp: 65, ready: false },
    Limit { command: 683, name: "Infinity", record: 0x85, word: 0x1F7, kh2_mp: 80, ready: false },
];
/// The follow-ups (03system `cmd`, KH2's name).
const FOLLOW_UPS: [(u16, &str); 9] = [(699, "Rave"), (700, "Finish"), (702, "Bash"), (703, "The End"), (705, "Raid"), (706, "Judgment"), (684, "Shoot"), (685, "Impact"), (686, "Giga Impact")];
/// The Triangle box's record and word.
const FOLLOW_RECORD: u8 = 0x86;
const FOLLOW_WORD: u16 = 0x1F6;
/// Our records' arguments (KH1's spells 0..6, Reflect 7, the Drive forms from 0x10).
pub const LIMIT_ARG: u8 = 0x20;
pub const FOLLOW_ARG: u8 = 0x28;
/// KH1's MP cost of every limit (one bar, in the payment's hundredths; `reflect::COST`).
const COST: i32 = 100;
/// `00objentry` id of `P_EX100_KH1F` (its script checks it is Limit Form).
const ENTRY_ID: u32 = 0x95D;
/// A follow-up shown this many frames after the script last offered it (KH1's menu fill runs on
/// its own step; the script offers it every frame its window is open).
const OFFER_FRAMES: u32 = 2;

const MENU_RVA: Rva = Rva(0x529600);
const MAGIC_MENU_RVA: Rva = Rva(0x529638);
const RECORD_RVA: Rva = Rva(0x28D7D0);
const PRESSED_RVA: Rva = Rva(0x298D4A8);
/// KH1's logical Triangle press (`docs/kh1-drive.md`, Input).
const TRIANGLE: u32 = 0x400_0000;
/// The top fill's tail jump into the context fill (`0x140292090` → `jmp 0x140292760`), and the
/// context fill's call of KH1's limits' Triangle offer (`0x14028AF60`, its one caller).
const CONTEXT_JMP_RVA: Rva = Rva(0x292320);
const CONTEXT_FILL_RVA: Rva = Rva(0x292760);
const SPECIALS_CALL_RVA: Rva = Rva(0x292B12);
const SPECIALS_RVA: Rva = Rva(0x28AF60);
const SUMMON: u8 = 6;
const SUMMON_OUT: u8 = 0x4F;

static BASE: AtomicUsize = AtomicUsize::new(0);
static ON: AtomicBool = AtomicBool::new(false);
/// KH1 text for "Limit", the limits' names and the follow-ups'.
static TEXT: OnceLock<(usize, Vec<usize>, Vec<usize>)> = OnceLock::new();
/// Record 2's (Magic's) word, read at the first fill.
static MAGIC_WORD: AtomicU32 = AtomicU32::new(u32::MAX);
/// The follow-up offered now (`cmd`) and the frame count when it last was.
static OFFER: Mutex<Option<(u16, u32)>> = Mutex::new(None);
static FRAME: AtomicU32 = AtomicU32::new(0);
/// A limit chosen (index into `LIMITS`) and a follow-up taken, for the next step.
static CHOSEN: Mutex<Option<usize>> = Mutex::new(None);
static TAKEN: Mutex<Option<u16>> = Mutex::new(None);
static ORIG_SPECIALS: AtomicUsize = AtomicUsize::new(0);
static ORIG_CONTEXT: AtomicUsize = AtomicUsize::new(0);

unsafe fn rd<T: Copy>(p: usize) -> T {
    std::ptr::read_unaligned(p as *const T)
}
unsafe fn wr<T: Copy>(p: usize, v: T) {
    std::ptr::write_unaligned(p as *mut T, v)
}
fn base() -> usize {
    BASE.load(Ordering::Relaxed)
}

fn note(s: String) {
    static N: AtomicU32 = AtomicU32::new(0);
    if N.fetch_add(1, Ordering::Relaxed) < 300 {
        log(&s);
    }
}

/// A command record argument of ours (a limit or the Triangle follow-up).
pub fn ours(arg: u8) -> bool {
    ON.load(Ordering::Relaxed) && ((LIMIT_ARG..LIMIT_ARG + LIMITS.len() as u8).contains(&arg) || arg == FOLLOW_ARG)
}

fn follow_up_name(cmd: u16) -> Option<usize> {
    let (_, _, follow) = TEXT.get()?;
    FOLLOW_UPS.iter().position(|f| f.0 == cmd).map(|i| follow[i])
}

/// The follow-up shown now, if any.
fn offered() -> Option<u16> {
    let now = FRAME.load(Ordering::Relaxed);
    OFFER.try_lock().ok().and_then(|o| *o).filter(|(_, at)| now.wrapping_sub(*at) <= OFFER_FRAMES).map(|(c, _)| c)
}

// ---------------------------------------------------------------- the menu

/// Our records (spell-type, as `drive`'s forms), written over empty ones.
unsafe fn ensure_records(base: usize) -> bool {
    let record: unsafe extern "system" fn(u32) -> usize = std::mem::transmute(base + RECORD_RVA);
    let all = LIMITS.iter().enumerate().map(|(k, l)| (l.record, l.word, LIMIT_ARG + k as u8, 0x02u8)).chain(std::iter::once((FOLLOW_RECORD, FOLLOW_WORD, FOLLOW_ARG, 0x12u8)));
    for (id, word, arg, flags) in all {
        let r = record(id as u32);
        if r < 0x10000 {
            return false;
        }
        // (The Triangle record runs while Triangle is held, flag 0x10: KH1's step runs the
        // Triangle box only for such records; `not_yet` takes only a fresh press.)
        let want: [u8; 16] = [flags, 0x00, 0xFF, 0x00, word as u8, (word >> 8) as u8, 0x02, arg, 0x00, 0xFF, 0x01, 0x02, 0, 0, 0, 0];
        let have = rd::<[u8; 16]>(r);
        if have != want {
            if have != [0u8; 16] && have[4..8] != want[4..8] {
                static ONCE: AtomicBool = AtomicBool::new(false);
                if !ONCE.swap(true, Ordering::Relaxed) {
                    log(&format!("Limit Form: command record {id:#x} is in use ({have:02x?}): no Limit menu"));
                }
                return false;
            }
            wr(r, want);
        }
    }
    if MAGIC_WORD.load(Ordering::Relaxed) == u32::MAX {
        let m = record(2);
        if m > 0x10000 {
            MAGIC_WORD.store(rd::<u16>(m + 4) as u32, Ordering::Relaxed);
        }
    }
    true
}

/// Whether limit `k` can be chosen now (and why not).
unsafe fn usable(actor: usize, k: usize) -> Result<(), &'static str> {
    let l = &LIMITS[k];
    if !l.ready {
        return Err("not in yet");
    }
    if script().is_none() {
        return Err("Limit Form's script not loaded");
    }
    if super::takeover::kh2_airborne() {
        return Err("in the air (KH2's Land flag)");
    }
    if running() {
        return Err("a limit is running");
    }
    if !super::reflect::can_pay(actor, COST) {
        return Err("MP short");
    }
    Ok(())
}

/// After KH1's battle-menu fill (`drive::fill`, first): in Limit Form the Magic row is "Limit"
/// and its list our limits.
pub unsafe fn fill(actor: usize) {
    let base = base();
    if !ON.load(Ordering::Relaxed) || base == 0 || !super::form::in_limit() || !ensure_records(base) {
        return;
    }
    let magic = base + MAGIC_MENU_RVA;
    let n = rd::<i32>(magic + 0x10);
    if !(0..=15).contains(&n) {
        return;
    }
    // (KH1's last entry, Summon or the summon-out one, stays last: the fourth row, page 2's Drive.)
    let last = (n > 0).then(|| rd::<u8>(magic + 0x14 + n as usize - 1)).filter(|e| matches!(*e, SUMMON | SUMMON_OUT));
    let (old4, old8) = (rd::<u32>(magic + 4), rd::<u32>(magic + 8));
    let mut grey = 0u32;
    for (k, l) in LIMITS.iter().enumerate() {
        wr(magic + 0x14 + k, l.record);
        if usable(actor, k).is_err() {
            grey |= 1 << k;
        }
    }
    let mut count = LIMITS.len();
    let (mut g4, mut g8) = (grey, grey);
    if let Some(e) = last {
        wr(magic + 0x14 + count, e);
        let b = (n - 1) as u32;
        g4 |= (old4 >> b & 1) << count;
        g8 |= (old8 >> b & 1) << count;
        count += 1;
    }
    wr(magic + 0x10, count as i32);
    wr(magic + 4, g4);
    wr(magic + 8, g8);
    if rd::<i32>(magic + 0xC) >= LIMITS.len() as i32 {
        wr(magic + 0xC, 0i32);
    }
    // The top's Magic row there and open (KH1 leaves it out with no spell).
    let top = base + MENU_RVA;
    wr(top + 0x14 + 1, 2u8);
    for off in [4usize, 8] {
        wr(top + off, rd::<u32>(top + off) & !2);
    }
    static LOGGED: AtomicU32 = AtomicU32::new(u32::MAX);
    if LOGGED.swap(grey, Ordering::Relaxed) != grey {
        let why: Vec<String> = (0..LIMITS.len()).map(|k| format!("{} {}", LIMITS[k].name, usable(actor, k).err().unwrap_or("usable"))).collect();
        log(&format!("Limit Form: the Limit menu ({})", why.join(", ")));
    }
}

/// A command's name by its word index: "Limit" for Magic's in Limit Form, our limits', the
/// follow-up shown. `None`: someone else's.
pub fn word(index: i32) -> Option<usize> {
    if !ON.load(Ordering::Relaxed) || !super::form::in_limit() {
        return None;
    }
    let (limit, names, _) = TEXT.get()?;
    if index as u32 == MAGIC_WORD.load(Ordering::Relaxed) {
        return Some(*limit);
    }
    if index == FOLLOW_WORD as i32 {
        return offered().and_then(follow_up_name).or_else(|| follow_up_name(FOLLOW_UPS[0].0));
    }
    LIMITS.iter().position(|l| l.word as i32 == index).map(|k| names[k])
}

/// The context fill (KH1's Triangle box), then ours: the follow-up offered now in the box.
unsafe extern "system" fn context_fill(a: usize) -> u64 {
    let f: unsafe extern "system" fn(usize) -> u64 = std::mem::transmute(ORIG_CONTEXT.load(Ordering::Acquire));
    let r = f(a);
    let base = base();
    if super::form::in_limit() && running() {
        if offered().is_some() {
            let top = base + MENU_RVA;
            wr(top + 0x17, FOLLOW_RECORD);
            for off in [4usize, 8] {
                wr(top + off, rd::<u32>(top + off) & !8);
            }
        }
    }
    r
}

/// KH1's limits' Triangle offer (`0x14028AF60`): none in a Drive Form (the user: "KH1 limits
/// that are equipped shouldn't even prompt in any drive form").
unsafe extern "system" fn specials_prompt() {
    if super::form::in_form() {
        static N: AtomicU32 = AtomicU32::new(0);
        if N.fetch_add(1, Ordering::Relaxed) == 0 {
            log("KH1's limits: not offered in a Drive Form");
        }
        return;
    }
    let f: unsafe extern "system" fn() = std::mem::transmute(ORIG_SPECIALS.load(Ordering::Acquire));
    f()
}

/// KH1's menu runs one of our entries (`combat::spell_not_yet`): `true` keeps it from running.
/// A limit only once KH2's move takes a command (else the list stays open, as for a spell); the
/// follow-up only on a fresh Triangle press while it is offered.
pub unsafe fn not_yet(arg: u8) -> bool {
    if arg == FOLLOW_ARG {
        let fresh = rd::<u32>(base() + PRESSED_RVA) & TRIANGLE != 0;
        return !(fresh && offered().is_some());
    }
    super::takeover::with_kh2(|c, _| c.can_take_command()) == Some(false)
}

/// One of our commands reached KH1's command step (`combat::command`): a limit chosen, or the
/// follow-up on Triangle.
pub unsafe fn command(actor: usize, arg: u8) {
    if arg == FOLLOW_ARG {
        let Some(cmd) = offered() else { return };
        if let Ok(mut t) = TAKEN.lock() {
            *t = Some(cmd);
        }
        if let Ok(mut o) = OFFER.lock() {
            *o = None;
        }
        note(format!("Limit Form: {} on Triangle", FOLLOW_UPS.iter().find(|f| f.0 == cmd).map_or("?", |f| f.1)));
        return;
    }
    let k = (arg - LIMIT_ARG) as usize;
    let Some(l) = LIMITS.get(k) else { return };
    if let Err(why) = usable(actor, k) {
        super::reflect::menu_sound(4);
        note(format!("Limit Form: {} refused ({why})", l.name));
        return;
    }
    if let Ok(mut c) = CHOSEN.lock() {
        *c = Some(k);
    }
    note(format!("Limit Form: {} chosen", l.name));
}

/// A limit chosen and not started yet (`takeover`: an Attack meanwhile doesn't take its window).
pub fn waiting() -> bool {
    CHOSEN.try_lock().map_or(false, |c| c.is_some())
}

// ---------------------------------------------------------------- the script

fn script() -> Option<&'static Vec<u8>> {
    pose::kh2()?.limit.as_ref()?.script.as_ref()
}

/// What the script asked of Sora this frame, for the next step.
#[derive(Default)]
struct Requests {
    start: Option<u16>,
    pushes: Vec<u16>,
    face: Option<f32>,
}

struct Runtime {
    magic: Magic,
    file: Option<usize>,
    /// Pushed motions waiting (0: idle, the chain's end).
    queue: Vec<u16>,
    /// The motion handed to the controller as the next (not started yet).
    fed: Option<u16>,
    req: Requests,
    /// After a follow-up is taken: no offers until the motion starts again (motion, frame).
    taken_in: Option<(u16, u32)>,
    /// Script effect id → our drawn effect, and whether its end was passed on.
    fx: HashMap<u64, (Option<u32>, bool)>,
    started: u32,
    /// A limit paid for, started with the script's next frame (`after_step`, with the world).
    pending: Option<usize>,
}

static RT: Mutex<Option<Runtime>> = Mutex::new(None);

fn runtime(g: &mut Option<Runtime>) -> &mut Runtime {
    g.get_or_insert_with(|| {
        let mut magic = Magic::new();
        let file = pose::kh2().and_then(|k| k.limit.as_ref()).and_then(|f| {
            let script = f.script.as_ref()?;
            let made = match f.archive.as_ref().and_then(|a| SpellFile::bar_entry(a, 18)) {
                Some(pax) => SpellFile::from_parts("P_EX100_KH1F", script, &pax),
                None => SpellFile::script_only("P_EX100_KH1F", script),
            };
            match made {
                Ok(file) => Some(magic.add_spell(file)),
                Err(e) => {
                    log(&format!("Limit Form: its script didn't load ({e}): no limits"));
                    None
                }
            }
        });
        Runtime { magic, file, queue: Vec::new(), fed: None, req: Requests::default(), taken_in: None, fx: HashMap::new(), started: 0, pending: None }
    })
}

/// A limit runs.
pub fn running() -> bool {
    RT.try_lock().ok().is_some_and(|g| g.as_ref().is_some_and(|rt| rt.pending.is_some() || rt.magic.limit_running(SORA)))
}

/// Out of the form: the limit ended, its effects put out.
pub fn end_all() {
    if let Ok(mut c) = CHOSEN.lock() {
        *c = None;
    }
    if let Ok(mut t) = TAKEN.lock() {
        *t = None;
    }
    if let Ok(mut o) = OFFER.lock() {
        *o = None;
    }
    let Ok(mut g) = RT.lock() else { return };
    let Some(rt) = g.as_mut() else { return };
    rt.magic.end_objects(SORA);
    rt.queue.clear();
    rt.fed = None;
    rt.req = Requests::default();
    rt.taken_in = None;
    rt.pending = None;
    for h in rt.fx.values().filter_map(|v| v.0) {
        super::kh2fx::spell_kill(h);
    }
    rt.fx.clear();
}

/// The world as the limit's script sees it: KH1's (`reflect::world_now`), with Sora driven.
struct LimitWorld<'a> {
    kh: KhWorld,
    out: &'a Output,
    /// The motion now as the script has it (one it started this frame, else KH2's).
    motion: u16,
    frame: u32,
    clips: Option<&'a HashMap<u16, character_core::Clip>>,
    /// Pushes waiting besides this frame's (the host's queue, or a motion handed on).
    queued: bool,
    req: &'a mut Requests,
    offer: Option<u16>,
}

impl LimitWorld<'_> {
    fn clip(&self) -> Option<&character_core::Clip> {
        self.clips?.get(&self.motion)
    }
}

impl World for LimitWorld<'_> {
    fn exists(&self, o: ObjId) -> bool {
        self.kh.exists(o)
    }
    fn pos(&self, o: ObjId) -> Vec3 {
        self.kh.pos(o)
    }
    fn facing(&self, o: ObjId) -> f32 {
        match (o, self.req.face) {
            (SORA, Some(f)) => f,
            _ => self.kh.facing(o),
        }
    }
    fn motion(&self, o: ObjId) -> u16 {
        if o == SORA {
            self.motion
        } else {
            self.kh.motion(o)
        }
    }
    fn airborne(&self, o: ObjId) -> bool {
        self.kh.airborne(o)
    }
    fn jumping(&self, o: ObjId) -> bool {
        self.kh.jumping(o)
    }
    fn bone(&self, o: ObjId, code: u16) -> Option<Mat4> {
        self.kh.bone(o, code)
    }
    fn target_point(&self, o: ObjId) -> Vec3 {
        self.kh.target_point(o)
    }
    fn target_of(&self, o: ObjId) -> Option<ObjId> {
        self.kh.target_of(o)
    }
    fn party(&self, n: u32) -> Option<ObjId> {
        self.kh.party(n)
    }
    fn has_ability(&self, o: ObjId, n: u32) -> bool {
        self.kh.has_ability(o, n)
    }
    fn ground(&self, from: Vec3, depth: f32) -> Option<Vec3> {
        self.kh.ground(from, depth)
    }
    fn bg_hit(&self, from: Vec3, to: Vec3) -> bool {
        self.kh.bg_hit(from, to)
    }
    fn hitback(&mut self, o: ObjId, dir: Vec3, dist: f32) {
        self.kh.hitback(o, dir, dist)
    }
    fn random(&mut self) -> f32 {
        self.kh.random()
    }
    fn motion_start(&mut self, o: ObjId, m: u16, _blend: f32) {
        if o == SORA {
            self.req.start = Some(m);
            self.req.pushes.clear();
            self.motion = m;
            self.frame = 0;
            self.queued = false;
        }
    }
    fn motion_push(&mut self, o: ObjId, m: u16, _blend: f32) {
        if o == SORA {
            self.req.pushes.push(m);
        }
    }
    fn motion_ended(&self, o: ObjId) -> bool {
        if o != SORA || self.req.start.is_some() {
            return false;
        }
        let len = self.clip().map_or(1, |c| c.length);
        self.frame + 1 >= len && !self.queued && self.req.pushes.iter().all(|m| *m == 0)
    }
    fn motion_range(&self, o: ObjId, n: u8) -> bool {
        o == SORA && self.req.start.is_none() && self.clip().is_some_and(|c| c.has(self.frame, |k| *k == WindowKind::ScriptRange(n)))
    }
    fn motion_trigger(&self, o: ObjId, n: u8) -> bool {
        o == SORA && self.req.start.is_none() && self.out.events.iter().any(|e| *e == character_core::EventKind::Other(8 + n))
    }
    fn face(&mut self, o: ObjId, dir: Vec3) {
        if o == SORA && dir.x.abs() + dir.z.abs() > 1e-6 {
            self.req.face = Some(dir.x.atan2(dir.z));
        }
    }
    fn entry_id(&self, o: ObjId) -> u32 {
        if o == SORA {
            ENTRY_ID
        } else {
            0
        }
    }
    fn offer_command(&mut self, o: ObjId, command: u16) {
        if o == SORA {
            self.offer = Some(command);
        }
    }
}

/// Before KH2's step: a limit chosen starts, a follow-up taken goes to the script, and what the
/// script asked of Sora last frame is done (his motion started or chained, his facing).
pub unsafe fn before_step(actor: usize, ch: &mut Character) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let Ok(mut g) = RT.try_lock() else { return };
    if !super::form::in_limit() {
        if g.as_ref().is_some_and(|rt| rt.magic.limit_running(SORA)) {
            drop(g);
            end_all();
        }
        return;
    }
    let rt = runtime(&mut g);
    let chosen = CHOSEN.try_lock().ok().and_then(|mut c| c.take());
    if let (Some(k), Some(_)) = (chosen, rt.file) {
        let l = &LIMITS[k];
        if !ch.can_take_command() || usable(actor, k).is_err() {
            note(format!("Limit Form: {} dropped (KH2's move can't take it now, or it can't be used)", l.name));
        } else {
            let before = mp(actor);
            super::reflect::pay_mp(actor, COST);
            rt.pending = Some(k);
            log(&format!(
                "Limit Form: {} (cmd {}): KH2's {} MP is KH1's 1 bar (the mod's KH2 spells' 1 MP each; KH2's cost / 100 rounded up), MP {before:?} -> {:?}",
                l.name,
                l.command,
                l.kh2_mp,
                mp(actor)
            ));
        }
    }
    if let Some(cmd) = TAKEN.try_lock().ok().and_then(|mut t| t.take()) {
        if rt.magic.limit_command(SORA, cmd) {
            rt.taken_in = Some((super::takeover::KH2_MOTION.load(Ordering::Relaxed) as u16, 0));
        }
    }
    // The script's requests.
    let req = std::mem::take(&mut rt.req);
    if let Some(f) = req.face {
        ch.set_facing(f);
    }
    if let Some(m) = req.start {
        if ch.play(m, false) {
            rt.queue = req.pushes;
            rt.fed = None;
        } else {
            note(format!("Limit Form: the script's motion {m} isn't in the moveset"));
            rt.queue.clear();
        }
    } else {
        rt.queue.extend(req.pushes);
    }
    // The next pushed motion to the controller (it starts when the one playing ends).
    if !ch.scripted() {
        rt.queue.clear();
        rt.fed = None;
    } else if ch.scripted_next().is_none() && rt.fed.is_none() {
        if let Some(&m) = rt.queue.first() {
            rt.queue.remove(0);
            // (Idle and the fall end the chain: free movement from there.)
            if matches!(m, 0 | 4) {
                rt.queue.clear();
                ch.push_scripted(None);
            } else if ch.push_scripted(Some(m)) {
                rt.fed = Some(m);
            }
        }
    }
}

/// Sora's MP (stats `+0x44` of `+0x48`), for the log.
unsafe fn mp(sora: usize) -> (i32, i32) {
    let stats = pose::ptr(rd::<u32>(sora + 0x6C));
    if stats < 0x10000 {
        return (0, 0);
    }
    (rd::<i32>(stats + 0x44), rd::<i32>(stats + 0x48))
}

/// An unbound effect's root where the script has it (y up) → KH2's world (y down), turned along
/// its direction (as `formmagic`'s).
fn place(pos: Vec3, dir: Vec3) -> Mat4 {
    let r = Quat::from_rotation_arc(Vec3::Z, dir.try_normalize().unwrap_or(Vec3::Z));
    super::reflect::flip() * Mat4::from_rotation_translation(r, pos) * super::reflect::flip()
}

/// After KH2's step and the swings: the limit script's frame (its requests for the next step, the
/// follow-up offered, its effects).
pub unsafe fn after_step(actor: usize, out: &Output, airborne: bool, strikes: &mut combat::Strikes, _battle: &Battle, lock: Option<u32>) {
    FRAME.fetch_add(1, Ordering::Relaxed);
    if !ON.load(Ordering::Relaxed) || !super::form::in_limit() {
        return;
    }
    let Ok(mut g) = RT.try_lock() else { return };
    let Some(rt) = g.as_mut() else { return };
    if rt.pending.is_none() && rt.magic.casts.is_empty() && rt.magic.effects.is_empty() && rt.fx.is_empty() {
        return;
    }
    if rt.fed == Some(out.motion) {
        rt.fed = None;
    }
    let clips = pose::kh2().and_then(|k| k.limit.as_ref()).map(|f| &f.character.moveset.clips);
    let kh = world_now(actor, out, airborne, strikes, lock, 0x1A17_0000 ^ rt.started.wrapping_mul(2_654_435_761));
    let queued = rt.fed.is_some() || !rt.queue.is_empty();
    let mut req = std::mem::take(&mut rt.req);
    let mut w = LimitWorld { kh, out, motion: out.motion, frame: out.clip_frame, clips, queued, req: &mut req, offer: None };
    // A limit paid for: its script's limit trigger, its first frame now.
    if let (Some(k), Some(file)) = (rt.pending.take(), rt.file) {
        match rt.magic.limit(file, SORA, LIMITS[k].command, &mut w) {
            Ok(_) => {
                rt.started += 1;
                rt.queue.clear();
                rt.fed = None;
                rt.taken_in = None;
            }
            Err(e) => log(&format!("Limit Form: {}'s script didn't start ({e})", LIMITS[k].name)),
        }
    }
    let events = rt.magic.update(&mut w);
    let mut offer = w.offer;
    // A follow-up the motion carries (KH2's range 20, the engine's own offer): Last Arcanum's
    // Bash and The End, Infinity's.
    if offer.is_none() && rt.magic.limit_running(SORA) {
        offer = clips.and_then(|c| c.get(&out.motion)).and_then(|c| c.open(out.clip_frame).find_map(|win| if let WindowKind::OfferCommand(cmd) = win.kind { Some(cmd) } else { None }));
    }
    rt.req = req;
    // One press per window: after a follow-up, none until its motion starts again.
    if let Some((m, f)) = rt.taken_in {
        let restarted = rt.req.start.is_some() || out.motion != m || out.clip_frame < f;
        if restarted {
            rt.taken_in = None;
        } else {
            rt.taken_in = Some((m, out.clip_frame));
            offer = None;
        }
    }
    if let Some(cmd) = offer {
        if let Ok(mut o) = OFFER.lock() {
            *o = Some((cmd, FRAME.load(Ordering::Relaxed)));
        }
    }
    for e in events.into_iter().chain(rt.magic.take_events()) {
        match e {
            Event::Missing(n) => note(format!("Limit Form's script used {n} (not modelled)")),
            Event::Error(e) => log(&format!("Limit Form's script error: {e}")),
            Event::Strike { .. } => {}
        }
    }
    // Effects the script starts: from the form's archive, bound ones on Sora's bones.
    let package = super::kh2fx::LIMIT;
    let mut seen = Vec::new();
    for e in &rt.magic.effects {
        seen.push(e.id);
        let placed = e.owner.is_none().then(|| place(e.pos, e.dir));
        match rt.fx.get(&e.id) {
            None => {
                let h = if e.alive() { super::kh2fx::spell_start(package, e.element.id) } else { None };
                if let (Some(h), Some(at)) = (h, placed) {
                    super::kh2fx::spell_place(h, at);
                }
                if e.alive() {
                    super::kh2sound::spell_element(package, e.element.id);
                }
                rt.fx.insert(e.id, (h, false));
            }
            Some((Some(h), ended)) => {
                let (h, ended) = (*h, *ended);
                if let Some(at) = placed {
                    super::kh2fx::spell_place(h, at);
                }
                if e.killed {
                    super::kh2fx::spell_kill(h);
                } else if e.fade.is_some() {
                    super::kh2fx::spell_alpha(h, e.alpha());
                    if !e.alive() {
                        super::kh2fx::spell_kill(h);
                    }
                } else if (e.ended_at.is_some() || !e.alive()) && !ended {
                    super::kh2fx::spell_end(h);
                    rt.fx.insert(e.id, (Some(h), true));
                }
            }
            Some((None, _)) => {}
        }
    }
    rt.fx.retain(|id, (h, _)| seen.contains(id) || h.is_some_and(super::kh2fx::spell_running));
    if !rt.magic.limit_running(SORA) && rt.queue.is_empty() {
        if let Ok(mut o) = OFFER.lock() {
            *o = None;
        }
    }
}

// ---------------------------------------------------------------- install

/// Points the `jmp rel32` at `site` (which must go to `to`) at `ours`, through a stub near the
/// executable (`hook::redirect_call`'s, for a tail jump).
unsafe fn redirect_jump(site: usize, to: usize, ours: usize, what: &str) -> bool {
    if rd::<u8>(site) != 0xE9 {
        log(&format!("redirect {what} at {site:#x}: not a jump"));
        return false;
    }
    let rel = rd::<i32>(site + 1);
    if (site as i64 + 5 + rel as i64) as usize != to {
        log(&format!("redirect {what} at {site:#x}: goes elsewhere"));
        return false;
    }
    let Some(stub) = hook::alloc_near(site, 16) else {
        log(&format!("redirect {what}: no memory near {site:#x}"));
        return false;
    };
    let mut go = [0u8; 14];
    go[0] = 0xFF;
    go[1] = 0x25;
    go[6..].copy_from_slice(&(ours as u64).to_le_bytes());
    std::ptr::copy_nonoverlapping(go.as_ptr(), stub as *mut u8, go.len());
    let mut want = [0u8; 5];
    want[0] = 0xE9;
    want[1..].copy_from_slice(&rel.to_le_bytes());
    let mut jmp = [0u8; 5];
    jmp[0] = 0xE9;
    jmp[1..].copy_from_slice(&((stub as i64 - (site as i64 + 5)) as i32).to_le_bytes());
    hook::patch(site, &want, &jmp, what)
}

/// After `drive::install` (its word hook answers ours first).
pub unsafe fn install() {
    let base = super::sys::GetModuleHandleA(std::ptr::null()) as usize;
    BASE.store(base, Ordering::Relaxed);
    if !super::settings::get().drive {
        return;
    }
    // KH1's limits never offered in a form (the user), whatever else goes in.
    ORIG_SPECIALS.store(base + SPECIALS_RVA, Ordering::Release);
    if !hook::redirect_call(base + SPECIALS_CALL_RVA, base + SPECIALS_RVA, specials_prompt as *const () as usize, "KH1's limits' Triangle offer (none in a form)") {
        log("KH1's limits: their Triangle offer wasn't as expected: they still show in a form (refused when used)");
    }
    if !super::drive::on() {
        log("Limit Form: no Drive menu, no Limit menu");
        return;
    }
    let _ = TEXT.set((
        Box::leak(super::menu::encode("Limit").into_boxed_slice()).as_ptr() as usize,
        LIMITS.iter().map(|l| Box::leak(super::menu::encode(l.name).into_boxed_slice()).as_ptr() as usize).collect(),
        FOLLOW_UPS.iter().map(|f| Box::leak(super::menu::encode(f.1).into_boxed_slice()).as_ptr() as usize).collect(),
    ));
    ORIG_CONTEXT.store(base + CONTEXT_FILL_RVA, Ordering::Release);
    if !redirect_jump(base + CONTEXT_JMP_RVA, base + CONTEXT_FILL_RVA, context_fill as *const () as usize, "context fill (Limit Form's Triangle)") {
        log("Limit Form: the context fill wasn't as expected: no Triangle follow-ups");
    }
    ON.store(true, Ordering::Release);
    log(&format!(
        "Limit Form: the Limit menu ({}; each KH1's 1 bar of MP, KH2's {} MP)",
        LIMITS.iter().map(|l| if l.ready { l.name.to_string() } else { format!("{} (not in yet)", l.name) }).collect::<Vec<_>>().join(", "),
        LIMITS.iter().map(|l| l.kh2_mp.to_string()).collect::<Vec<_>>().join(" / ")
    ));
}
