//! Valor Form's second Keyblade in Sora's left hand (`docs/kh1-weapon-second.md`).
//!
//! KH1 gives a party member one weapon: its `.wpn` model is skinned to the last 13 bones of his
//! skeleton (280..292, the grip under the right hand and the chain), and KH1's model queue
//! (`0x1401D1F10`) queues it with the body, copying the bone matrices at that moment. So:
//!
//! 1. **Load** the Valor Keyblade's `.wpn` once, with KH1's own loader (`0x140289A80`) into a
//!    buffer of ours, and set its model up with KH1's setup (`0x1401D45F0`) in the callback.
//!    The loader's file record carries the remastered assets, so the HD mesh is hooked up as
//!    for KH1's own Keyblade. It stays loaded for the session; a different Keyblade chosen
//!    later is loaded in a new buffer and the old one released (KH1's own order: the model,
//!    then the file record with its textures, with the `.wpn` release handler kept out of it).
//! 2. **Pose**: `pose` hands over the left hand's 13 weapon matrices each frame (`note_pose`),
//!    from KH2's own left Keyblade (the Valor rig's second weapon, `kh2_left` from `takeover`).
//! 3. **Draw**: after KH1 queues Sora with his Keyblade out, queue him a second time with every
//!    body mesh hidden, our model as the weapon and the left-hand matrices put in bones
//!    280..292 for that call only (put back at once).
//!
//! Everything is off with `valor_left_blade = off` (nothing installed). Every step is logged a
//! few times.

use super::addr::Rva;
use super::log;
use glam::Mat4;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// KH1's model queue `(instance, view struct, draw params, actor)`.
const QUEUE_RVA: Rva = Rva(0x1D1F10);
/// `push rbp; push rbx; push rsi; push rdi` (both builds).
const QUEUE_PROLOGUE: [u8; 5] = [0x40, 0x55, 0x53, 0x56, 0x57];
/// The file loader `(name, buffer, callback(size, id, buffer), delay)`: queued on KH1's loader
/// task; the callback runs on the game thread once the file is in.
const LOAD_RVA: Rva = Rva(0x289A80);
/// A `jmp` to the model setup `0x1401D5AD0 (container)` (relocation, the HD table).
const SETUP_RVA: Rva = Rva(0x1D45F0);
/// The model release `(container)` (KH1's `.wpn` release handler calls it).
const RELEASE_MODEL_RVA: Rva = Rva(0x1D49D0);
/// The file record holding an address `(addr, unlink)`, and dropping one `(record, textures)`.
const FIND_RECORD_RVA: Rva = Rva(0xE2B10);
const DROP_RECORD_RVA: Rva = Rva(0xE2DF0);
/// KH1's `.wpn` release handler and the place in the file-type table that points at it (in
/// writable data): replaced by `wpn_release`, which keeps KH1's handler off our buffers (it
/// looks a buffer up among the three party slots and would use garbage for ours).
const WPN_RELEASE_RVA: Rva = Rva(0x2851E0);
const WPN_RELEASE_SLOT_RVA: Rva = Rva(0x4DAF48);
/// `btltbl.bin` and its item table pointer; each member's weapon table offset (Sora's first).
const ITEM_TABLE_PTR_RVA: Rva = Rva(0x2D23738);
const BTLTBL_RVA: Rva = Rva(0x2D23740);
const WEAPON_TABLE_OFFSETS_RVA: Rva = Rva(0x528D60);

/// A file record's buffer, size, and its file-type row (-1: no handler is called on release).
const REC_BUFFER: usize = 0x658;
const REC_ROW: usize = 0x64C;
/// The instance's world matrices (handle).
const INST_WORLD: usize = 0x1B4;
/// The draw params: the weapon model, the hidden-mesh mask; their size.
const P_WEAPON: usize = 0x18;
const P_MASK: usize = 0x38;
const P_SIZE: usize = 0x60;
/// The weapon bones: the first, how many, and Sora's bone count.
const GRIP: usize = 280;
const WEAPON_BONES: usize = 13;
const SORA_BONES: usize = 293;
/// KH1 Sora's hands (`pose::HANDS`).
const RIGHT_HAND: usize = 101;
const LEFT_HAND: usize = 206;
/// Our load buffer (Sora's own weapon slot is 0x40000; twice that to be safe).
const BUF_SIZE: usize = 0x80000;
const MENV: u32 = u32::from_le_bytes(*b"MENV");
const HD_MARK: u32 = 0x9696_9696;
/// A load that hasn't answered by then is given up (KH1 never calls back on a failure).
const LOAD_TIMEOUT_S: f32 = 15.0;

static INSTALLED: AtomicBool = AtomicBool::new(false);
static BASE: AtomicUsize = AtomicUsize::new(0);
static ORIG_QUEUE: AtomicUsize = AtomicUsize::new(0);
static IN_SECOND: AtomicBool = AtomicBool::new(false);
/// Valor's form is out (`show` / `hide`).
static SHOWN: AtomicBool = AtomicBool::new(false);
/// The mesh model drawn in the left hand (0: none ready).
static MESH: AtomicUsize = AtomicUsize::new(0);
/// The buffer of the load under way (0: none).
static PENDING_BUF: AtomicUsize = AtomicUsize::new(0);

struct Loaded {
    item: u8,
    buf: usize,
    container: usize,
    mesh: usize,
}

struct State {
    loaded: Option<Loaded>,
    /// The load under way: item, buffer, since.
    pending: Option<(u8, usize, Instant)>,
    /// Items whose load failed this session (not asked again).
    failed: Vec<u8>,
    /// Every buffer we ever gave KH1's loader (never freed: a late answer may still land).
    bufs: Vec<usize>,
}

static STATE: Mutex<State> = Mutex::new(State { loaded: None, pending: None, failed: Vec::new(), bufs: Vec::new() });
/// KH2's left Keyblade this frame (KH2 model space), from `takeover`.
static KH2_LEFT: Mutex<Option<(Vec<Mat4>, Instant)>> = Mutex::new(None);
/// The left hand's weapon bones in KH1's world, and the game frame they were made on.
static LEFT: Mutex<Option<([Mat4; WEAPON_BONES], u64)>> = Mutex::new(None);
/// KH2's bind pose of the Valor skeleton (for the hand offsets), made once.
static KH2_BIND: Mutex<Option<Vec<Mat4>>> = Mutex::new(None);

unsafe fn rd<T: Copy>(p: usize) -> T {
    std::ptr::read_unaligned(p as *const T)
}

fn readable(at: usize, len: usize) -> bool {
    at >= 0x10000 && unsafe { super::crash::readable(at) && super::crash::readable(at + len.max(1) - 1) }
}

/// Logs `line` the first `max` times its counter is bumped.
fn log_n(counter: &AtomicU32, max: u32, line: impl FnOnce() -> String) {
    if counter.fetch_add(1, Ordering::Relaxed) < max {
        log(&format!("left Keyblade: {}", line()));
    }
}

pub unsafe fn install() {
    if !super::settings::get().valor_left_blade {
        log("left Keyblade: off (valor_left_blade = off)");
        return;
    }
    let base = super::sys::GetModuleHandleA(std::ptr::null()) as usize;
    BASE.store(base, Ordering::Relaxed);
    // KH1's .wpn release handler, pointed at ours (data: the pointer itself is checked).
    let slot = base + WPN_RELEASE_SLOT_RVA;
    let want = ((base + WPN_RELEASE_RVA) as u64).to_le_bytes();
    let ours = (wpn_release as *const () as usize as u64).to_le_bytes();
    if !super::crash::readable(slot) || !super::hook::patch(slot, &want, &ours, "the .wpn release handler (left Keyblade)") {
        log("left Keyblade: KH1's .wpn release handler not where expected: not installed");
        return;
    }
    if super::hook::install(base + QUEUE_RVA, &QUEUE_PROLOGUE, queue as *const () as usize, &ORIG_QUEUE, "model queue (left Keyblade)").is_none() {
        // (Put KH1's handler back: nothing of ours will be loaded.)
        super::hook::patch(slot, &ours, &want, "the .wpn release handler (back)");
        return;
    }
    INSTALLED.store(true, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------------------------
// Loading

/// The `.wpn` file name of a Sora weapon (KH1 item id), from `btltbl.bin`'s weapon table.
unsafe fn file_name(base: usize, item: u8) -> Option<String> {
    if item == 0 {
        return None;
    }
    let table = rd::<u64>(base + ITEM_TABLE_PTR_RVA) as usize;
    let rec = table.checked_add((item as usize - 1) * 0x14)?;
    if !readable(rec, 0x14) || rd::<u8>(rec + 2) != 2 {
        return None;
    }
    let row = rd::<i16>(rec + 6);
    if row <= 0 {
        return None;
    }
    let off = rd::<i32>(base + WEAPON_TABLE_OFFSETS_RVA);
    let at = (base + BTLTBL_RVA).checked_add(off as usize)? + (row as usize - 1) * 0x58;
    if off <= 0 || !readable(at, 16) {
        return None;
    }
    let stem: Vec<u8> = std::slice::from_raw_parts(at as *const u8, 16).iter().copied().take_while(|&c| c != 0).collect();
    if stem.is_empty() || !stem.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_') {
        return None;
    }
    Some(format!("{}.wpn", String::from_utf8_lossy(&stem)))
}

/// Valor began (`valor::started`), its Keyblade `item`: drawn once its model is in; loaded now
/// if it isn't (game thread).
pub unsafe fn show(item: Option<u8>) {
    if !INSTALLED.load(Ordering::Relaxed) {
        return;
    }
    SHOWN.store(true, Ordering::Relaxed);
    let Some(item) = item else {
        MESH.store(0, Ordering::Relaxed);
        return;
    };
    let base = BASE.load(Ordering::Relaxed);
    let Ok(mut st) = STATE.try_lock() else { return };
    // A load that never answered: given up (its buffer is kept, never reused).
    if let Some((it, buf, since)) = st.pending {
        if since.elapsed().as_secs_f32() > LOAD_TIMEOUT_S {
            log(&format!("left Keyblade: the load of {it:#x} never answered ({:.0} s): given up", since.elapsed().as_secs_f32()));
            st.pending = None;
            st.failed.push(it);
            if PENDING_BUF.load(Ordering::Relaxed) == buf {
                PENDING_BUF.store(0, Ordering::Relaxed);
            }
        }
    }
    if let Some(l) = st.loaded.as_ref() {
        if l.item == item {
            MESH.store(l.mesh, Ordering::Relaxed);
            return;
        }
    }
    if st.pending.is_some_and(|p| p.0 == item) || st.failed.contains(&item) {
        return;
    }
    if st.pending.is_some() {
        // (Another Keyblade's load is under way: asked again at the next Valor.)
        return;
    }
    // Another Keyblade was loaded: released (not drawn from now on).
    if let Some(old) = st.loaded.take() {
        MESH.store(0, Ordering::Relaxed);
        release(&old);
    }
    let Some(name) = file_name(base, item) else {
        log(&format!("left Keyblade: no weapon file for item {item:#x}"));
        st.failed.push(item);
        return;
    };
    let buf = super::sys::VirtualAlloc(std::ptr::null_mut(), BUF_SIZE, 0x3000, 0x04) as usize;
    if buf == 0 {
        log("left Keyblade: no memory for the model");
        st.failed.push(item);
        return;
    }
    st.bufs.push(buf);
    st.pending = Some((item, buf, Instant::now()));
    PENDING_BUF.store(buf, Ordering::Relaxed);
    drop(st);
    let mut cname = name.clone().into_bytes();
    cname.push(0);
    let load: unsafe extern "system" fn(*const u8, usize, usize, u32) -> u32 = std::mem::transmute(base + LOAD_RVA);
    let id = load(cname.as_ptr(), buf, loaded as *const () as usize, 0);
    log(&format!("left Keyblade: load of {name} (item {item:#x}) requested into {buf:#x} (request {id})"));
}

/// Back from Valor (`valor::ended`): not drawn; the model stays loaded.
pub fn hide() {
    SHOWN.store(false, Ordering::Relaxed);
}

/// KH1's loader is done with our file (game thread, KH1's loader task).
unsafe extern "system" fn loaded(size: i32, id: u32, buf: usize) {
    static N: AtomicU32 = AtomicU32::new(0);
    log_n(&N, 20, || format!("load answered: request {id}, {size} bytes into {buf:#x}"));
    if buf == 0 || PENDING_BUF.load(Ordering::Relaxed) != buf {
        log_n(&N, 20, || "(not the load we wait for: ignored)".into());
        return;
    }
    PENDING_BUF.store(0, Ordering::Relaxed);
    let Ok(mut st) = STATE.try_lock() else {
        log("left Keyblade: busy when the load answered: model not used");
        return;
    };
    let Some((item, _, _)) = st.pending.take() else { return };
    match set_up(size, buf) {
        Ok((container, mesh)) => {
            st.loaded = Some(Loaded { item, buf, container, mesh });
            if SHOWN.load(Ordering::Relaxed) {
                MESH.store(mesh, Ordering::Relaxed);
            }
        }
        Err(e) => {
            log(&format!("left Keyblade: item {item:#x}'s model not used: {e}"));
            st.failed.push(item);
        }
    }
}

/// The `.wpn` in `buf`: its model container set up by KH1; returns it and its mesh model.
unsafe fn set_up(size: i32, buf: usize) -> Result<(usize, usize), String> {
    if size <= 0x10 || size as usize > BUF_SIZE {
        return Err(format!("size {size}"));
    }
    let size = size as usize;
    let (o4, o8, oc) = (rd::<u32>(buf + 4) as usize, rd::<u32>(buf + 8) as usize, rd::<u32>(buf + 0xC) as usize);
    // (`+0xC` may run past the bytes loaded: the user's Divine Rose said 0x1F700 of 0x1F690, its
    // end padding. KH1's own `.wpn` handler `0x140285120` reads only `+4` and `+8`.)
    let _ = oc;
    if !(o4 <= o8 && o4 < size && o8 % 4 == 0 && o8 + 0x30 <= size) {
        return Err(format!("header {o4:#x} {o8:#x} {oc:#x} (size {size:#x})"));
    }
    let container = buf + o8;
    let magic = rd::<u32>(container);
    if magic != MENV {
        return Err(format!("container magic {magic:#010x}, not MENV"));
    }
    let setup: unsafe extern "system" fn(usize) = std::mem::transmute(BASE.load(Ordering::Relaxed) + SETUP_RVA);
    setup(container);
    let mesh = super::pose::ptr(rd::<u32>(container + 0x20));
    if !readable(mesh, 0x20) {
        return Err(format!("no mesh model after setup ({mesh:#x})"));
    }
    let bones = rd::<i16>(mesh);
    if bones < 1 || bones as usize > WEAPON_BONES {
        return Err(format!("{bones} bones (at most {WEAPON_BONES} fit Sora's weapon bones)"));
    }
    let hd = rd::<u32>(mesh + 0x10) == HD_MARK && rd::<u32>(mesh + 0x14) != 0;
    log(&format!(
        "left Keyblade: model set up: container {container:#x}, mesh {mesh:#x}, {bones} bones, {} meshes, {}",
        rd::<i32>(mesh + 0xC),
        if hd { "remastered (HD) table hooked up" } else { "no remastered table (PS2 mesh)" }
    ));
    Ok((container, mesh))
}

/// Releases a loaded model as KH1 releases a `.wpn` (its model, then its file record with the
/// textures, deferred by KH1), with the record's handler row cleared so KH1's handler isn't
/// called on it. The buffer is kept.
unsafe fn release(old: &Loaded) {
    let base = BASE.load(Ordering::Relaxed);
    let find: unsafe extern "system" fn(usize, i32) -> usize = std::mem::transmute(base + FIND_RECORD_RVA);
    let record = find(old.buf, 0);
    if record != 0 && readable(record, REC_BUFFER + 8) && rd::<u64>(record + REC_BUFFER) as usize == old.buf {
        std::ptr::write_unaligned((record + REC_ROW) as *mut i32, -1);
        let free_model: unsafe extern "system" fn(usize) = std::mem::transmute(base + RELEASE_MODEL_RVA);
        free_model(old.container);
        let drop_record: unsafe extern "system" fn(usize, u8) = std::mem::transmute(base + DROP_RECORD_RVA);
        drop_record(record, 1);
        log(&format!("left Keyblade: item {:#x}'s model released (record {record:#x})", old.item));
    } else {
        log(&format!("left Keyblade: item {:#x}'s file record not found ({record:#x}): left loaded", old.item));
    }
}

/// KH1's `.wpn` release handler (`record`), but never for a buffer of ours (it would look our
/// buffer up among the three party slots, find none and release garbage).
unsafe extern "system" fn wpn_release(record: usize) -> u64 {
    let buf = if readable(record, REC_BUFFER + 8) { rd::<u64>(record + REC_BUFFER) as usize } else { 0 };
    let ours = STATE.try_lock().map(|s| s.bufs.contains(&buf)).unwrap_or(false) || buf == PENDING_BUF.load(Ordering::Relaxed);
    if ours {
        static N: AtomicU32 = AtomicU32::new(0);
        log_n(&N, 10, || format!("KH1 released our file record {record:#x} (buffer {buf:#x}): not drawn from now on"));
        MESH.store(0, Ordering::Relaxed);
        if let Ok(mut st) = STATE.try_lock() {
            if st.loaded.as_ref().is_some_and(|l| l.buf == buf) {
                st.loaded = None;
            }
        }
        return 0;
    }
    let f: unsafe extern "system" fn(usize) -> u64 = std::mem::transmute(BASE.load(Ordering::Relaxed) + WPN_RELEASE_RVA);
    f(record)
}

// ---------------------------------------------------------------------------------------------
// Posing

/// `takeover`: KH2's left Keyblade this frame (the rig's second weapon), if there is one.
pub fn kh2_left(bones: Option<&[Mat4]>) {
    if !INSTALLED.load(Ordering::Relaxed) || !super::form::in_valor() {
        return;
    }
    if let Ok(mut g) = KH2_LEFT.try_lock() {
        *g = bones.filter(|b| !b.is_empty()).map(|b| (b.to_vec(), Instant::now()));
    }
}

/// A matrix with its translation times `k` (KH2 units to KH1's).
fn scaled(m: Mat4, k: f32) -> Mat4 {
    let mut m = m;
    m.w_axis = (m.w_axis.truncate() * k).extend(1.0);
    m
}

/// `pose`, after Sora's KH1 bones are drawn from KH2's pose: the left hand's weapon bones.
///
/// - `kh1`: KH1's bones as drawn (KH1 model space), `kh1_bind` KH1's bind; `kh2_body` and
///   `kh2_right` KH2's body and right Keyblade (KH2 model space); `to_world` KH1's placement;
///   `ratio` KH1's size against KH2's (`Retarget::hip_ratio`).
/// - KH2's two Keyblades are one model on two hand bones (`sklt`): each weapon root relative to
///   its hand bone is known for both. Each KH2 hand bone is put on KH1's hand by the bind poses'
///   relation (both T-poses facing +z, same axes). The turn from KH2's right weapon root to KH1's
///   grip (280, as `pose` drew it: KH1's grip, `flip_weapons`, teeth up) is then the same for the
///   left, and the chain keeps the right one's shape.
pub fn note_pose(kh1: &[Mat4], kh1_bind: &[Mat4], kh2_body: &[Mat4], kh2_right: &[Mat4], to_world: &Mat4, ratio: f32) {
    if !INSTALLED.load(Ordering::Relaxed) || !SHOWN.load(Ordering::Relaxed) || !super::form::in_valor() || MESH.load(Ordering::Relaxed) == 0 {
        return;
    }
    static N: AtomicU32 = AtomicU32::new(0);
    let Some(left) = KH2_LEFT.try_lock().ok().and_then(|g| g.as_ref().filter(|(_, t)| t.elapsed().as_secs_f32() < 0.15).map(|(b, _)| b[0])) else { return };
    let Some(rig) = super::pose::kh2().and_then(|k| k.valor.as_ref()).map(|f| f.rig.clone()) else { return };
    let (Some(sr), Some(sl)) = (rig.weapons.first().and_then(|w| w.bone), rig.weapons.get(1).and_then(|w| w.bone)) else {
        log_n(&N, 5, || "the Valor rig has no second weapon bone".into());
        return;
    };
    if kh1.len() < SORA_BONES || kh1_bind.len() < SORA_BONES || kh2_body.len() <= sr.max(sl) || kh2_right.is_empty() {
        return;
    }
    let Ok(mut bind) = KH2_BIND.try_lock() else { return };
    let b2 = bind.get_or_insert_with(|| kh2_anim::Pose::bind(&rig.skeleton).bones);
    if b2.len() <= sr.max(sl) {
        return;
    }
    let k = if ratio.is_finite() && ratio > 0.1 { ratio } else { 1.0 };
    // KH2's hand bone in KH1's hand frame (bind poses).
    let offset = |kh1_hand: usize, kh2_hand: usize| kh1_bind[kh1_hand].inverse() * scaled(b2[kh2_hand], k);
    // Weapon root relative to its KH2 hand bone now.
    let rel = |kh2_hand: usize, root: Mat4| scaled(kh2_body[kh2_hand].inverse() * root, k);
    let right_root = kh1[RIGHT_HAND] * offset(RIGHT_HAND, sr) * rel(sr, kh2_right[0]);
    let turn = right_root.inverse() * kh1[GRIP];
    let grip = kh1[LEFT_HAND] * offset(LEFT_HAND, sl) * rel(sl, left) * turn;
    let from_hand = (grip.w_axis - kh1[LEFT_HAND].w_axis).truncate().length();
    if !grip.is_finite() || from_hand > 40.0 {
        log_n(&N, 5, || format!("left grip {from_hand:.0} units from the left hand: not drawn this frame"));
        return;
    }
    let inv_grip = kh1[GRIP].inverse();
    let mut out = [Mat4::IDENTITY; WEAPON_BONES];
    for (i, m) in out.iter_mut().enumerate() {
        *m = *to_world * grip * inv_grip * kh1[GRIP + i];
    }
    log_n(&N, 3, || format!("left grip posed, {from_hand:.1} units from the left hand"));
    if let Ok(mut g) = LEFT.try_lock() {
        *g = Some((out, super::takeover::GAME_FRAME.load(Ordering::Relaxed)));
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing

type Queue = unsafe extern "system" fn(usize, usize, usize, usize) -> usize;

unsafe extern "system" fn queue(instance: usize, view: usize, params: usize, actor: usize) -> usize {
    let f: Queue = std::mem::transmute(ORIG_QUEUE.load(Ordering::Acquire));
    let r = f(instance, view, params, actor);
    if !IN_SECOND.load(Ordering::Relaxed) {
        second(f, instance, view, params, actor);
    }
    r
}

/// The left Keyblade: Sora queued again, body hidden, our model in the left hand's bones.
unsafe fn second(f: Queue, instance: usize, view: usize, params: usize, actor: usize) {
    let mesh = MESH.load(Ordering::Relaxed);
    if mesh == 0 || !SHOWN.load(Ordering::Relaxed) || !super::form::in_valor() || actor == 0 || actor != super::takeover::sora_actor() {
        return;
    }
    if instance != super::pose::SORA_INSTANCE.load(Ordering::Relaxed) || !readable(params, P_SIZE) {
        return;
    }
    // KH1 drew his own Keyblade (out, and its model there).
    if rd::<u64>(params + P_WEAPON) == 0 {
        return;
    }
    let frame = super::takeover::GAME_FRAME.load(Ordering::Relaxed);
    let Some(left) = LEFT.try_lock().ok().and_then(|g| g.as_ref().filter(|(_, at)| frame.saturating_sub(*at) <= 2).map(|(m, _)| *m)) else { return };
    if !readable(instance + INST_WORLD, 4) {
        return;
    }
    let world = super::pose::ptr(rd::<u32>(instance + INST_WORLD));
    if !readable(world, SORA_BONES * 0x40) {
        return;
    }
    let at = world + GRIP * 0x40;
    let mut saved = [0u8; WEAPON_BONES * 0x40];
    std::ptr::copy_nonoverlapping(at as *const u8, saved.as_mut_ptr(), saved.len());
    for (i, m) in left.iter().enumerate() {
        std::ptr::write_unaligned((at + i * 0x40) as *mut [f32; 16], m.to_cols_array());
    }
    // KH1's own params for this draw, but every body mesh hidden and our model as the weapon.
    #[repr(C, align(16))]
    struct Params([u8; 0x100]);
    let mut p = Params([0; 0x100]);
    std::ptr::copy_nonoverlapping(params as *const u8, p.0.as_mut_ptr(), P_SIZE);
    std::ptr::write_unaligned(p.0.as_mut_ptr().add(P_MASK) as *mut u32, 0xFFFF_FFFF);
    std::ptr::write_unaligned(p.0.as_mut_ptr().add(P_WEAPON) as *mut u64, mesh as u64);
    IN_SECOND.store(true, Ordering::Relaxed);
    f(instance, view, p.0.as_ptr() as usize, actor);
    IN_SECOND.store(false, Ordering::Relaxed);
    std::ptr::copy_nonoverlapping(saved.as_ptr(), at as *mut u8, saved.len());
    static N: AtomicU32 = AtomicU32::new(0);
    log_n(&N, 3, || format!("drawn in the left hand (mesh {mesh:#x}, grip at {:.0?})", left[0].w_axis.truncate()));
}
