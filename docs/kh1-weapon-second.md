# A second Keyblade on KH1 Sora (Valor Form's left hand)

Goal: while Valor Form is active, KH1 Sora holds a second Keyblade in his left hand, as KH2's
Valor does. It is a different Keyblade from the equipped one (the "Valor Keyblade", any
Keyblade he owns). KH1 itself only ever gives a party member one weapon.

Static reading of the Epic exe (image base `0x140000000`, add the ASLR slide), Steam RVAs from
`/opt/re/steam/resolve.py`. Data facts were checked against the user's `btltbl.bin`,
`xa_ex_0010.mdls` and `kh1_first.hed`. Nothing here has been checked at runtime yet; the
checks to make first are in section 6.

Confidence: **[high]** read directly from code or data; **[med]** several pieces agree but
nothing was run; **[low]** a guess.

---

## 1. How KH1 loads a party member's weapon

### 1.1 Which file [high]

- The equipped weapon is the item byte at `save + member * 0x74 + 0x36`. The save pointer is
  at `0x1428695A0`, the same one `pose.rs` reads; Sora is member 0.
- `btltbl.bin` is loaded once, synchronously, at **`0x142D23740`** (`0x14028D840`; Steam
  `0x2D22D40`).
  - Item table: the pointer at `0x142D23738` is `btltbl + 0x1A58`, with 0x14-byte records
    (`0x14028D7E0(id)` returns `table + (id - 1) * 0x14`). Byte `+2` is the owner (member + 2,
    so 2 for Sora). The short at `+6` is the **weapon model number** (1-based).
  - Weapon records: `btltbl + table[member] + (n - 1) * 0x58`, where `table` is the u32 array
    at `0x140528D60`: Sora `0x94F8`, Donald `0x9C88`, Goofy `0xA208`, then `0xA788`, `0`,
    `0xA7E0` (guests). The record starts with the file stem (16 bytes). The rest holds trail
    and effect parameters (not decoded).
- File names are `"%s%s"` (`0x1403B2E0C`) of the stem plus `".se"` (pointer at `0x140528D90`),
  then plus `".wpn"` (pointer at `0x140528D88`). Sora's 22 weapons are items `0x51..0x66`,
  model numbers 1..22, in order:

| item | file | item | file | item | file |
|---|---|---|---|---|---|
| 0x51 | xw_ex_5010 (Kingdom Key) | 0x59 | xw_ex_5050 | 0x61 | xw_ex_5130 |
| 0x52 | xw_dh_5000 (dream sword) | 0x5A | xw_ex_5060 | 0x62 | xw_ex_5140 |
| 0x53 | xw_dh_5010 (dream shield) | 0x5B | xw_ex_5070 | 0x63 | xw_ex_5150 |
| 0x54 | xw_dh_5020 (dream rod) | 0x5C | xw_ex_5080 | 0x64 | xw_ex_5160 |
| 0x55 | xw_di_5000 (wooden sword) | 0x5D | xw_ex_5090 | 0x65 | xw_ex_5190 |
| 0x56 | xw_ex_5020 | 0x5E | xw_ex_5100 | 0x66 | xw_ex_5200 |
| 0x57 | xw_ex_5030 | 0x5F | xw_ex_5110 | | |
| 0x58 | xw_ex_5040 | 0x60 | xw_ex_5120 | | |

  (`0x52..0x55` being the dream weapons and the wooden sword matches `pose.rs`'s existing
  exclusion.)

### 1.2 The weapon slots and the loader [high]

Three **slots** of 0x30 bytes at **`0x14296BF40`** (Steam `0x296B540`). Slot 0 is always
Sora's. Slots 1 and 2 are for any other member (Donald, Goofy, guests).

| slot + | what |
|---|---|
| `0x00` | flags: bit 0 in use; bits 1-2 state (2 = an object uses it, 4 = loading) |
| `0x04` | member |
| `0x08` | weapon model number |
| `0x0C` | buffer size: `0x40000` for Sora, `0x20000` for the others (from `0x140528D98..`) |
| `0x10` | buffer: a static pool at `0x14286BBE0` (Sora first, then the two others) |
| `0x18` | the `.wpn`'s model container (file + `[file+8]`), after setup |
| `0x20` | the `.wpn`'s effect section (file + `[file+4]`), after setup |
| `0x28` | completion callback (gets the member) |

Functions (Epic → Steam):

| Epic | Steam | what |
|---|---|---|
| `0x1402849C0` | `0x286B40` | reset the slots (area start) |
| `0x140284A30(cb)` | `0x286BB0` | load every party member's weapon (members from `save+0x48E..0x490`), chained through `0x140284EE0` |
| `0x140284A60(member, cb)` | `0x286BE0` | load one member's weapon into its slot; returns 1 if a load started, 0 if it was already there or there is none |
| `0x140285080` | `0x287200` | `.se` loaded: register it as sound bank `-3 - slot`, then load the `.wpn` (`0x140285270`) into the same buffer |
| `0x140285120` | `0x2872A0` | `.wpn` loaded: container → `0x1401D45F0` (a `jmp` to the model setup **`0x1401D5AD0`**); effects → `0x1401DDF20`; then the slot's callback |
| `0x140284BE0(actor)` / `0x140284D40(member)` | `0x286D60` / `0x286EC0` | create the **weapon object** for that member's slot; the second also stores it at `actor+0x14C` and runs the pending equip callback |
| `0x140284760(actor)` | `0x2868E0` | free the actor's weapon object |
| `0x1402845A0(item, cb)` | `0x286720` | equip an item (menu: `0x140302AF0` / `0x140302BA0`): free the object, write the save byte, reload, recreate |


All loads go through **`0x140289A80(name, buffer, callback, delay)`** (Steam `0x28BC10`).
It queues a request on KH1's loader task, and the read runs as a job (`0x14028A070` →
`0x14028A200`). The task (`0x14028A140`) calls `callback(size, request id, buffer)` **on the
game thread** once the size is known (≥ 0). A failed load never calls back. Requests are
serialised; the job queue at `0x14234B9A8` has only two entries, so never post jobs directly,
only through `0x140289A80`.

### 1.3 The weapon object [high for the fields named, med for the rest]

A pool of 9 objects of 0x60 bytes at **`0x14296BBE0`** (Steam `0x296B1E0`), with an in-use
bitmask at `0x14296BFD8`. `actor+0x14C` is a handle to one of them (`pose::ptr` resolves it).
Creating one also sets `actor+0x374 |= 0x10`.

| obj + | what |
|---|---|
| `0x00` | handle of the model container (slot `+0x18`) |
| `0x04` | handle of the **mesh model**: `0x1401D4600(container, obj+4, 0, 0)` copies the container's `+0x20` |
| `0x0C..0x0F` | RGBA bytes; draw-weapon (`0x1402A0CA0`) sets `ff ff ff 80` |
| `0x18` | pointer to the `.wpn`'s effect section (slot `+0x20`); the swing effect's source in `combat.rs` |
| `0x20` | pointer to the `btltbl` weapon record |
| `0x28` | trail object (from `combat.rs`) |
| `0x40` / `0x48` | cleared at creation |

The `.wpn` container is a `MENV`, not a `MOBJ`: `0x1401D4600` would write through its null
arguments for a `MOBJ`. The container has no instance and no skeleton of its own.

### 1.4 How it is attached to the hand: Sora's own skeleton [high]

The Keyblade has **no matrix of its own**. KH1 Sora's skeleton (293 bones) includes the weapon
rig: 280 is the grip under the right hand (101), and 281 to 292 are the blade and chain (stage
15). The weapon's mesh model has 13 bones and is skinned onto **the last 13 bones of the body's
matrices**: the queue below gives the weapon record a bone offset of
`body bone count − weapon bone count` = 293 − 13 = 280. KH1's motions animate 280..292, and
`pose.rs` writes them when KH2 has Sora. Swapping Keyblades only swaps the mesh. Donald's and
Goofy's weapons work the same way on their own skeletons.

In Sora's bind, bones 282..291 carry the skeleton flag `0x40000000` (bits 30-31 of the links
word = 1); 280, 281 and 292 don't. The queue only copies matrices for bone 0 and flagged bones,
so the mesh is skinned to 282..291.

---

## 2. How the weapon is drawn each frame [high]

1. **`0x14029F4A0(actor, hidden-mesh mask, p3, colour float[4], p5 float)`** (Steam `0x2A1630`)
   is the party actor's model draw. It is reached from four per-actor draw variants
   (`0x14029EE00`, `0x14029F360`, `0x14029F9E0`, `0x14029FBC0`), which pass `actor+0x38C` as
   the mask and `actor+0x394` as p3. It fills a 0x60-byte parameter block:

   | param + | what |
   |---|---|
   | `0x00` | flags (bit 0: use the remastered body table; 0x100 / 0x200 clip and scissor) |
   | `0x04` | `actor+0x420` (lit) |
   | `0x0C` | body RGBA (from the colour floats) |
   | `0x18` | **weapon mesh model** = `ptr(obj+4)`, only if the object, its model and actor flag `0x4000` are all there; else 0 |
   | `0x20` | body RGBA again; `0x24..0x27` = the weapon object's `+0xC..+0xF` |
   | `0x28` / `0x30` | per-character callback and its argument (the actor for Sora) |
   | `0x38` | **the hidden-mesh mask** (bit n hides mesh group n) |
   | `0x40` / `0x48` | lighting callbacks `0x14029BB70`, `0x14029BAA0` |
   | `0x50` / `0x58` | `0x1000070`, `0x800D0` |

   It then calls `0x1401D43B0` (a `jmp` to **`0x1401D1F10`**) with (instance =
   `ptr(actor+0x15C)`, view struct `0x14299C530`, params, actor).
2. **`0x1401D1F10(instance, view, params, actor)`** (Steam `0x1D4070`, prologue
   `40 55 53 56 57`) is the model queue:
   - It counts the body meshes not hidden by `+0x38`. With a weapon present it goes on even if
     that count is 0.
   - Body model record: 0x68 bytes at `0x1423FE658 + (half * 0x4E + n) * 0x68`, with n from
     `0x1423FE1C4`.
     - Its matrix block comes from the frame scratch allocator `0x14028B890`.
     - It **copies the instance's world matrices** (`instance+0x1B4`, the array `pose.rs`
       writes) for bone 0 and every flagged bone. In lit mode it copies two matrices per bone.
     - One 200-byte mesh record per visible mesh goes to `0x142402600 + (half * 0x1E0 + m) * 200`.
   - **Weapon record**: a second model record whose matrix block is the body's (same pointer),
     with the bone offset above, plus one mesh record per weapon mesh. Its colours come from
     params `+0x20..0x27`.
   - The matrices are copied **at queue time**. A second queue call made after changing the
     world array therefore gets its own copy, and the first call's copy stays as it was.
3. The world pass (task 160000, `0x1401EADD0`) draws the mesh records through
   `0x1401DC5A0`, the same as the body.

### 2.1 HD (remastered) Keyblades [high mechanism, med that every `.wpn` has them]

- A mesh model may carry a remastered table: `model+0x10 == 0x96969696` and `model+0x14` is a
  handle to a pointer to the table. The mesh count and entry accessors (`0x1401DB570`,
  `0x1401DB4F0`) use that table when it is there. The draw (`0x1401DC5A0`) takes the remastered
  branch (`0x1401DB690` / `0x1401DB730`) by the same per-mesh flag as Sora's body (see
  `kh1-tint.md`). Weapons go through exactly the same records and draw.
- The marker is set at **setup time**: `0x1401D5AD0` → `0x1401DA690` (Steam `0x1DC7F0`).
  1. It finds the file record whose buffer holds the model (`0x1400E2B10`; record list
     `0x1421ABB30`, record `+0x658` buffer, `+0x640` size).
  2. In the record's remastered-asset list (`+0x670`) it looks for the asset tagged with the
     model's offset in the file (asset `+0x54`) whose data (`+0x60`) starts with `0x4126`.
  3. It builds the table with `0x1401D9FA0` and writes the marker and handle.
- So **an HD Keyblade needs its `.wpn` loaded by KH1's own loader** (which creates the file
  record and its remastered assets, textures included) **and set up with `0x1401D5AD0` while
  that record exists**.
- Evidence that `.wpn` entries carry remastered assets: in `kh1_first.hed`,
  `xw_ex_5010.wpn` is 0x1A490 bytes but takes 0x50E10 in the PKG, and `xw_dh_5000.wpn` is
  0x1CC90 against 0xC22B0. The extra bytes are the appended assets, as with
  `xa_ex_0010.mdls` (0xA69C0 against 0xB2AF00). Their kinds (a `.cvbl` mesh plus DDS
  textures, as for Sora) are not yet read.

### 2.2 A second weapon object through KH1's functions? [high: no]

- `0x140284BE0` / `0x140284D40` always build the object from **the member's own slot**
  (found by member id), so they can't make an object for another item.
- There are three slots for three party members, and in a normal party all three are in use.
- The other loader callers (`0x1401AFB00`, an event command; the menu equip; the area-start
  load-all) all reload a member's own slot.
- KH1 has no code path that loads a weapon model by item id into a spare place, and no code
  that draws two weapons on one actor.
- The weapon object pool (9) and the draw would accept a hand-made object; only the loading
  has to be ours.

---

## 3. Design

### 3.1 Loading the Valor Keyblade [med]

Use KH1's own loader into **our own buffer**, outside every range KH1 loads into:

1. **When:** preload once per session, the first frame KH2 has Sora in the field, and again
   only when the configured Valor Keyblade changes.
   - This takes timing out of battle. The load is a small PKG read on KH1's loader, over a few
     frames, queued behind KH1's own loads (more during area changes).
   - Don't issue it while KH1's pause or event state is up; wait for the callback.
2. **What:** item id (ini, default for example Kingdom Key `0x51`, or "same as equipped") →
   model number `*(i16*)(item record + 6)` → stem = the `btltbl` record's first 16 bytes →
   `"<stem>.wpn"`. Skip the `.se`: the left blade needs no sound bank.
3. **Buffer:** `VirtualAlloc` 0x40000 (Sora's slot size; `.wpn` files are about 0x1A000 to
   0x1D000). Keep 0x60 bytes elsewhere, zeroed, for our weapon object.
4. **Load:** call `0x140289A80(name, buffer, our_callback, 0)` from the game thread.
5. **`our_callback(size: i32, id: u32, buffer: usize)`** (Win64), on the game thread:
   1. Check `size > 0`, and that `[buf+8] < [buf+0xC] <= size` and `[buf+4] <= [buf+8]`.
   2. `container = buf + [buf+8]`; check that its magic is `MENV`.
   3. Call `0x1401D45F0(container)` (Steam `0x1D6750`; or `0x1401D5AD0` directly). This
      relocates it and hooks up the HD table, because our file record exists now.
   4. Build our object:
      - `obj+0 = 0x14038B080(container)` (pointer → handle; registers the 32 MB region in the
        64-entry table at `0x142EE4730` if new)
      - `obj+4 = *(u32*)(container + 0x20)`
      - `obj+0xC = ff ff ff 80`
   5. Log the mesh model `m = ptr(obj+4)`: bone count `*(i16*)m` (expect 13 or fewer), and
      whether `m+0x10 == 0x96969696`. Mark it ready.
   6. Don't call `0x1401DDF20` (effects): the left blade gets no KH1 trail or swing effect.
6. **Never let KH1 release our record through the `.wpn` handler.**
   - KH1 drops file records only by address range: `0x1400E38C0(start, end, flag)` when a
     loader reuses a buffer, `0x1400E3830(addr)`, and a few fixed areas. None of them overlaps
     a buffer of ours.
   - Its release handler for `.wpn` (`0x1402851E0`, row pointer at **`0x1404DAF48`**, Steam
     `0x4DA268`, in writable `.data`) looks the buffer up among the three slots. For ours it
     finds none and then calls `0x1401D49D0` on garbage (slot −1 = bytes `0x14296BF28` /
     `0x14296BF30`).
   - To change or free our Keyblade:
     1. Set our record's `+0x64C` (the handler row) to `-1`.
     2. Call `0x1400E2DF0(record, 1)` (Steam `0xE2E00`). This marks its textures for deferred
        deletion and frees the record without calling the handler. Find the record with
        `0x1400E2B10(buffer, 0)`.
     3. Release the model with `0x1401D49D0(container)`, whose texture frees are also deferred.
   - For safety, also replace the row's pointer with a wrapper that ignores our buffer and
     calls `0x1402851E0` for everything else.
7. **Not ready yet** (first seconds, failed load): no left blade, or the equipped Keyblade's own
   mesh as a stand-in. A stand-in needs no load: use `obj+4` = the right object's `+4`.
   Time out a load that hasn't called back after a few seconds; KH1 never calls back on
   failure.

### 3.2 The left hand's matrices (`pose.rs`) [med]

KH2's Valor rig already has the left weapon: `rig.weapons[1]` (`W_EX010_BTLF_L`, attached to
the `sklt` left bone; `kh2_data::sora::load_rig`). Generalise `kh2_weapon` to a weapon index
and pose `weapons[1]` with the same motion. Then, each frame KH2 poses Sora in Valor:

- Calibrate on the right hand.
  - `C = inverse(KH2 right weapon root, mapped onto KH1's right hand) * pose[280]`, where
    "mapped onto KH1's hand" is `drawn hand * inverse(hand offset) * (inverse(KH2 hand) * KH2
    weapon root)`, with the translation scaled to KH1's size.
  - The hand offset is the fixed bind relation between KH2's and KH1's hand bones (both binds
    are T-poses facing +z with the same axes; the retarget already holds them).
  - KH2's two weapons are the same model, so C (KH2 weapon root → KH1 grip 280, including
    `kh1_wrist`, `flip_weapons` and `keyblade_teeth_up`) is the same for both hands.
- `G_L = drawn left hand (206) * inverse(left hand offset) * (inverse(KH2 left hand) * KH2
  left weapon root) * C`.
- Bones `280 + i` for i in 0..13: `L[i] = G_L * inverse(pose[280]) * pose[280 + i]` (the right
  chain's shape). A later step can pose the chain from `weapons[1]`'s chain bones with
  `secondary`.
- Store `to_world * L[i]` (KH1 world, as written to the world array) with the game frame
  number, for the draw hook.
- Fallback when no second KH2 weapon is posed: none (no left blade).

### 3.3 Drawing it: a second queue call [med-high]

Hook **`0x1401D1F10`** (Steam `0x1D4070`). The prologue `40 55 53 56 57` is push rbp, rbx,
rsi and rdi: 5 bytes, position independent.

```
hook(instance, view, params, actor):
    r = original(instance, view, params, actor)
    if !IN_SECOND && actor == Sora's actor && valor active && ready
       && *(u64*)(params+0x18) != 0          // KH1 drew his right Keyblade (weapon out)
       && left matrices are from this game frame:
        p2 = copy of the 0x60 bytes at params
        *(u32*)(p2+0x38) = 0xFFFFFFFF        // hide every body mesh group
        *(u64*)(p2+0x18) = ptr(obj+4)        // our mesh model
        p2[0x24..0x28] = right object's +0xC..+0xF  // KH1's fade / alpha follow
        w = ptr(*(u32*)(instance+0x1B4)) + 280*0x40
        save 13*0x40 bytes at w; write L; IN_SECOND = true
        original(instance, view, &p2, actor)
        IN_SECOND = false; restore the 13 matrices
    return r
```

- KH1 draws the right Keyblade first, copying the right-hand matrices. Our call copies the
  left-hand ones into its own block and queues only the weapon. The array is restored before
  anything else reads it.
- Everything else comes from KH1's own parameters: lighting callbacks, colours, the HD-body
  flag, and the lit path.
- Cost: one more model record (78 per half frame), 2 to 10 mesh records (480), and 18 or
  37 KB of frame scratch.
- The second call also re-runs the instance's texture-animation bookkeeping (counters reset per
  call) [med: harmless as read]. On the non-HD path (`params` bit 0 clear), it would see
  `actor+0x374`'s bit 31 after the first call cleared bit 30 [low impact].
- Alternative: hook `0x14029F4A0` (Steam `0x2A1630`, prologue `40 55 53 56 57`). Call it a
  second time with the mask `0xFFFFFFFF` and `actor+0x14C` briefly pointing at our object
  (that needs our object's handle). This is less direct, because the actor's draw also touches
  shadow and clip state.
- Skip the pause menu (`0x14232E9B4`), whose party models are drawn through the same
  functions.

### 3.4 Lifetime summary

| moment | action |
|---|---|
| first field frame with KH2 in control, or the ini choice changes | load our `.wpn` (3.1) |
| Valor starts | start drawing (3.3) as soon as it is ready |
| every frame in Valor | `pose.rs` stores L; the draw hook queues the left blade while KH1's right one is out |
| revert | stop drawing; keep the model loaded |
| choice changes | drop the old record and model (3.1 step 6), load the new one |

---

## 4. What the left blade does not get (yet)

- No KH1 trail, swing effect or sparkle: those come from the weapon object's effect section
  (`+0x18`) and trail object (`+0x28`), set up by `0x1401DDF20` and driven per actor. Hits are
  ours anyway: KH2's `weapons[1]` shapes.
- No `.se` sound bank: KH2's own sounds play for Valor.

## 5. Address list

| what | Epic | Steam RVA |
|---|---|---|
| Sora's actor pointer | `0x142D37C80` | `0x2D37280` |
| actor's weapon object handle | `actor+0x14C` | same |
| actor's model instance handle | `actor+0x15C` | same |
| instance's world matrices handle | `instance+0x1B4` | same |
| party actor model draw | `0x14029F4A0` | `0x2A1630` |
| model queue | `0x1401D1F10` (via `0x1401D43B0`) | `0x1D4070` (`0x1D6510`) |
| model record table / mesh record table | `0x1423FE658` / `0x142402600` | `0x23FDC58` / `0x2401C00` |
| frame scratch allocator | `0x14028B890` | `0x28DA20` |
| mesh draw | `0x1401DC5A0` | `0x1DE700` |
| HD hookup at setup | `0x1401DA690` | `0x1DC7F0` |
| model setup (relocate) | `0x1401D5AD0` (thunk `0x1401D45F0`) | `0x1D7C30` (`0x1D6750`) |
| object from container | `0x1401D4600` | `0x1D6760` |
| model release | `0x1401D49D0` | `0x1D6B30` |
| pointer → handle / handle → pointer | `0x14038B080` / `0x14038B0B0` | `0x38AD90` / `0x38ADC0` |
| file load (async) | `0x140289A80` | `0x28BC10` |
| file record list | `0x1421ABB30` | `0x21AB020` |
| find / drop a file record | `0x1400E2B10` / `0x1400E2DF0` | `0xE2B20` / `0xE2E00` |
| `.wpn` release handler / its row pointer | `0x1402851E0` / `0x1404DAF48` | `0x287360` / `0x4DA268` |
| weapon slots / object pool / bitmask | `0x14296BF40` / `0x14296BBE0` / `0x14296BFD8` | `0x296B540` / `0x296B1E0` / `0x296B5D8` |
| slot buffers (static) | `0x14286BBE0` | `0x286B1E0` |
| `btltbl.bin` / item table pointer | `0x142D23740` / `0x142D23738` | `0x2D22D40` / `0x2D22D38` |
| per-member weapon table offsets / `.wpn` / `.se` suffix pointers | `0x140528D60` / `0x140528D88` / `0x140528D90` | `0x528060` / `0x528088` / `0x528090` |
| equip item / load member weapon / create object | `0x1402845A0` / `0x140284A60` / `0x140284D40` | `0x286720` / `0x286BE0` / `0x286EC0` |
| draw weapon (flag 0x4000) / put away | `0x1402A0CA0` / `0x1402A10F0` | `0x2A2E30` / `0x2A3280` |

## 6. Runtime checks, in order

1. Log Sora's weapon object (`ptr(actor+0x14C)`) and its mesh model (`ptr(obj+4)`): bone count
   (expect 13), and `+0x10 == 0x96969696` with `+0x14` set (KH1's own Keyblade is HD).
2. Load `xw_ex_5020.wpn` into our buffer and check the same on ours (HD hooked up).
3. First draw test without `pose.rs` changes: L = the right matrices moved 30 units along
   Sora's +x. Expect a second Keyblade beside the first.
4. Change areas with it loaded, and check that it still draws (that no global texture purge
   takes our record's textures).
5. Swap the Valor Keyblade twice, and check that nothing crashes (the record drop of 3.1
   step 6).
