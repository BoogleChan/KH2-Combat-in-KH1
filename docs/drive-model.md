# Drive Form model

How Drive Forms work in this project. The rules are in `character_core::drive`; the
playground's form change is in `kh2-bevy/src/drive.rs`.

Confidence labels:
- **Checked (exe)**: read from the executable's code.
- **Data**: read from your install.
- **Ours**: our choice.

## The forms are separate player objects (Checked, exe + Data)

Each form is its own object in `00objentry.bin`, with:
- its own model and motion set (`obj/P_EX100_<suffix>`)
- a moveset id, which picks:
  - its `prty`/`plyr` records (speeds, jump, auto-target range)
  - its PTYA combo table
  - its weapon motion sets in 03system `wmst` (row `moveset × 2 + hand`)

| Form | id | Object | Moveset | Weapons (wmst) | Run speed |
|---|---|---|---|---|---|
| Valor | 1 | P_EX100_BTLF | 2 | W_EX010_BTLF_R / _L | 12 |
| Wisdom | 2 | P_EX100_MAGF | 3 | W_EX010_MAGF | 12 |
| Limit | 3 | P_EX100_KH1F | 0x42 | W_EX010_KH1F | 8 |
| Master | 4 | P_EX100_TRIF | 4 | W_EX010_TRIF_R / _L | 10 |
| Final | 5 | P_EX100_ULTF | 5 | W_EX010_ULTF_R / _L | 16 |
| Anti | 6 | P_EX100_HTLF | 6 | none | 16 |

**Weapons.**
- Both hands' weapons attach to the bones in `sklt` (right 178, left 86).
- Their motions move them, so Master's and Final's floating keyblades come out of the data.
- Attack windows name the weapon they belong to: body, first weapon or second weapon.

## Drive gauge (Checked, exe)

- **Bars.** One bar is 100 points, up to the maximum (9 at most).
  - When full, the gauge holds at 100 points in the last bar.
  - Paying a cost keeps a part-filled bar unless the gauge was full or short.
- **Sora's hits** add the ATKP row's drive value (byte 0x29):
  - ×0.75 against normal enemies
  - ×2 while MP charges (+0.2 per Drive Boost)
  - truncated, so a value-1 hit on a normal enemy adds nothing
- **Ours:** spell hits add drive too (the game's handler runs for Sora's attacks; that it
  covers spells is inferred).
- **Not modelled:** orbs (the rule is in `Drive::orb`), Damage Drive, items.

## Driving (Checked, exe)

1. **Costs** come from 03system `cmd` byte 0x1c: Valor 3, Wisdom 3, Limit 4, Master 4,
   Final 5, Anti 9.
   - A command is unavailable when the gauge is short, or Sora is already in a form or
     changing.
2. **The Anti roll.**
   - A counter rises by 1 with every drive that isn't Final.
   - The Anti chance is 0% below 5, 10% from 5, and 25% from 10 (fmlv Anti rows).
   - A Final drive lowers the counter by 10; becoming Anti lowers it by 6.
   - Anti happens only in battle. **Ours:** "in battle" means an enemy is up.
3. **The cost of the resulting form is paid.** Anti costs 9, which empties the gauge.
4. **Sora's Drive motion** (0x34, 160 frames) plays, invulnerable. The screen tints grey:
   12 frames in, then 15 frames out after the swap.
5. **The swap.** The form object replaces Sora, keeping his position, facing and lock-on.
   - It plays its arrival motion (0x35).
   - HP and MP go to full, and MP Charge is cancelled.
6. **Form gauge** = `(form level + 3) × 600` frames. Anti uses 6 bars.

## In a form (Checked, exe)

- **Draining.** The form gauge drains 1 per frame in battle, 0.5 outside (×0.8 per Form
  Boost).
- **Hits drain it too:** each of the form's hits takes `15 × drive value` frames, ×0.2 on
  normal enemies.
- **Magic.**
  - The `sub` gets +10 in Wisdom, +20 in Master and +30 in Final, which picks the spells'
    form variants and `magi` records.
  - Valor, Limit and Anti can't cast (the command's form mask).
- **Abilities.**
  - Sora's support abilities carry over (Combo Plus and the like). His action and growth
    abilities don't.
  - Each form brings its growth ability at its level: Valor High Jump, Wisdom Quick Run,
    Limit Dodge Roll, Master Aerial Dodge, Final Glide. Form Lv1/3/5/7 give LV1/2/3/MAX.
  - **Ours:** every row of the form's combo table is unlocked, and the form guards if its
    motion set has the guard motion.
- **Anti** takes 1.5× damage.
- **Reverting** happens when the gauge empties, on the Revert command (the Drive row reads
  Revert in a form), or on a KO.
  - Sora comes back at once and plays motion 4.
  - There is no refill and no cooldown.
- **Form EXP** (fmlv table, levels capped at 7):
  - Valor: +1 per hit
  - Wisdom: +1 per Heartless defeated (status ids 1000..2011, not 1047..1059)
  - Final: +1 per Nobody defeated (3000..4014)
  - Master's rule (drive orbs) and Limit's (limit finishers) aren't modelled.

## Wisdom Form's shots (Checked, exe + Data)

Wisdom's attacks have no hitbox windows. Their motions fire frame trigger 1 with an effect id
(25..43).
- **Where the logic lives.** Wisdom's model file has a script (`wisd`). The game runs its
  trigger 0x10 with the object, the id, and the effect started from the form's effect archive
  (`obj/P_EX100_MAGF.a.us`).
- **What the script does.** It fires one homing shot per id:
  - ATKP 859, sub 0, 3 or 5
  - a sphere of radius 50
  - speed 20 rising by 2 per frame to 80
  - turns 1.8° per frame toward Sora's target, and stops homing within 100
  - lives 24 frames plus an 8-frame fade
  - stops on a hit or a wall
- **How we run it.** `kh2-magic` runs this script like a spell (`Magic::object_effect`). One
  trap was added for it: `obj_effect_start`.

## Final Form's floating Keyblades (Checked, Data)

The user: "sora's keyblades dont spin properly during final form glide. also his keyblades in
final form glide and in final form jump should do damage".

- **Where the spin is.** KH2's Keyblade model (`W_EX010.mdlx`) has its blade on bone 2: the mesh
  and every hit shape are there. Bones 0 and 1 place it; 3 and on are the keychain. Final's
  weapon motions spin bone 2, not the root:
  - glide (205..207): bone 2 turns about its z by −12π over the 60-frame loop (6 turns), while
    the root holds the Keyblade still beside him;
  - idle (0): bone 2 rolls once about its x over 160 frames.
- **Why ours didn't spin.** `leftblade::final_float` and `note_pose` moved KH1's Keyblades by
  KH2's weapon root only, so the blade's own turn was lost. They now follow bone 2
  (`KH2_BLADE`), matched against bone 2 of base Sora's held idle. Bone 2 is not animated in the
  other motions, so those look as before. Valor still follows the root.
- **Where the left one hangs from.** The user: "when sora hovers on the ground, the
  second keyblade follows him but it jitters a little. also, when gliding and turning, the
  second keyblade goes to the same position as the first keyblade".
  - The data is not the cause. `W_EX010_ULTF_L` has its own clip for every motion
    `W_EX010_ULTF_R` has (0..19, 40, 53, the casts, 151..154, 181..183, 205..208), and no two
    are the same. The body's motion 139 has no clip in either hand's set; both then play
    motion 0, as `kh2_rig` does for any weapon (what KH2 does there is not checked).
  - KH2's own spacing (`kh2-anim/examples/final_blades`, blade bone 2 in the body's space):

    | Motion | Apart | Left blade from the left hand |
    |---|---|---|
    | Idle (0) | 64..80 | about 38 |
    | Glide (205) | 140, one on each side | about 95 |
    | Banked turns (206, 207) | about 35, both on one side of him | about 115, across his body |
  - The cause: each Keyblade was put on KH1's hand of its side, by its KH2 hand bone. KH1's
    hands are rebuilt from KH2's bones, not copied, so each turns a little differently from
    KH2's. A Keyblade floating 40 to 120 units from its hand swings by that difference. The
    left hand's wobble showed as the jitter, and in a turn the two hands' differences came to
    the 35 units between the blades.
  - Now, in Final Form, `note_pose` hangs the left Keyblade from KH2's right-hand bone, on
    KH1's right hand, as `final_float` hangs the right one. The two keep KH2's spacing exactly,
    and the left is as steady as the right. Valor's Keyblades are held, so each still hangs from
    its own hand.
- **The attacks.** Final's weapon motions carry their own attack triggers (range trigger 10,
  group 2 on bone 2, three spheres of radius 60):

  | Motion | Right / left ATKP | Frames | Power | Re-hit interval |
  |---|---|---|---|---|
  | Jump (3) | 1273 / 1274 | 2..12 | 20 | none (once) |
  | Jump (3) | 1286 / 1287 | 12..end | 10 | 10 |
  | Fall (4) | 1275 / 1276 | 2..20 | 20 | none |
  | Land (5) | 1277 / 1278 | 0..20 | 20 | none |
  | Glide (205..207) | 1281 / 1282 | all | 100 | 30 |
  | Hover (208) | 162 (a guard, kind 2) | 1..end | 0 | — |

  KH2's weapon object plays its motion whatever Sora's action, so these hurt in the jump's rise,
  the fall and the glide. Our controller runs windows only for actions (the jump's push, the
  landing). `kh2_rig::MotionHits` reads the windows of the clip the rig shows at its frame
  (weapons only, attacks only) while KH2 has him jumping, in the air or gliding. `takeover`
  hands them to `combat::Strikes` with the controller's own, replacing the jump action's copies.
  A window open from frame to frame keeps its swing id, across the glide's banked turns too. So
  a foe is hit once per window, or again after the row's interval. They land through KH1's
  contact code with the row's power, as the swings do. The hover's guard is not modelled.
  No other form has attack windows in these motions.

## Final Form's magic (Data; the rules ours, as Wisdom's)

The user: "lets try to incorporate our rules, logic, and process from wisdom forms' magic, and
bring it to final form, with final form's animations".

- **No spell files of its own.** MAGC has rows for worlds 0 and 10 only (10: the `…lk.mag`
  variants), none per form. KH2's Final casts Sora's `magic/<SPELL>_<level>.mag` with its sub
  + 30. 03system `magi` has records 30..35, and the spells' ATKP rows (248..251, 573.., 708..)
  have subs 30..35, as Wisdom's have 10..15.
- **Its cast motions.** MAGC's row gives the same motion ids in every form. Each moveset fills
  them with its own clips. In `P_EX100_ULTF.mset` these are 60 fps clips; their lengths below are
  in 60 Hz frames.

  | Spell | Ground | Combo finisher | Air |
  |---|---|---|---|
  | Fire | 56 `E362` (76) | 57 `E363` (92) | 58 `E372` (68) |
  | Blizzard | 59 `E360` (50) | 60 `E361` (104) | 61 `E370` (42) |
  | Thunder | 62 `E364` (50) | 63 `E365` (100) | 64 `E374` (42) |
  | Cure | 65 `E362`, as Fire's | none | 67 `E372`, as Fire's |
  | Reflect | 71 `E362` | 72 `E363` | 73 `E372` |

  `E362` carries the magic-cast event for 56, 65 and 71, and `E372` for 58, 67 and 73. So Final's
  Fire, Cure and Reflect share one cast. Every Final cast has an allow-movement range all through.
- **How we cast them** (`form::Kind::magic_sub`, `form::casting_form`):
  - Fire, Blizzard and Thunder are KH2's. They take Wisdom's path through
    `formmagic::spell_chosen` and `before_step`: the MAGC motions, the controller's combo slots
    and cast windows, the script at sub level × 2 − 2 (+ 1 for the finisher) + 30, its effects
    and sounds, and hits with KH1's own data for the spell at its level. Level and cost are
    KH1's.
  - Cure is KH1's own, drawn with Final's Cure motion.
  - Gravity, Stop and Aero are KH1's. Gravity is drawn on Final's Thunder motion; Stop and Aero
    on its Cure motion. The air ones use the air motions. Each runs on that motion's clock,
    input window and movement (`takeover::command_started`'s `FormCast`, `pose::follow`).
  - Reflect runs at Final's sub (+ 30) now, as Wisdom's runs at + 10.
  - In the air, the controller's casts use KH2's own physics for Final (`takeover`'s
    `in_final()` air-magic block). KH1's spells in the air are handed off as they are in
    Wisdom: KH1's air state moves him while Final's air cast motion is drawn.
  - Final has no shots. Its frame triggers 1 stay its own elements (`kh2fx::form_event`).
- **Unchecked in game:** all of it. In particular, whether Final's 60 fps cast clips line up
  with KH1's cast timing for the KH1 spells.

## Anti Form in the KH1 port (Data + exe notes above; the rules ours where marked)

The user: "I want you to add anti form. I want 1:1 combat parity. I dont want it to cost any MP
to enter, and I dont want sora to be able to revert out of it." Then: "Drive forms in this mod
run out when you're out of MP, so sora will revert when he is out of MP". Since s285 it takes
half his MP to enter, rounded down (the user: "Antiform should take half of your MP"), and still
starts at any MP.

- **How he gets it (ours).** Only by the secret button code (`form::pad`, read in `probe::pad`).
  It is never in the Drive list. The code is taken when KH2 could change him (free, jumping,
  falling, landing or at a move's end) and he isn't in a form or changing. KH2's Anti roll (the
  counter above) isn't used.
- **Its data (Data, checked on extracted files).** `P_EX100_HTLF`, moveset 6:
  - the same 256-bone skeleton as Sora's (`pose::load_form` checks this);
  - no weapon sets (`wmst` has none for moveset 6), so it holds no Keyblade. Its attack windows
    are on its body's own shapes: see "Its attack shapes" below;
  - `prty`: run 16, jump 185, turn 0.26;
  - 13 combo rows. Rows 11 (motion 173) and 12 (motion 161) are kind 0, reached only by id
    (its Square moves, below);
  - motion 40 (Aerial Recovery) but no motion 42 (the guard's), so no guard;
  - its own sounds (bank 503, frame-trigger sounds) and frame-trigger effects from its own
    archive (`obj/P_EX100_HTLF.a.us`, `kh2fx::ANTI`).
- **Its attack shapes (Data, the Steam install's `obj/P_EX100_HTLF.mdlx`).** Anti has its own
  collision table (34 entries, BAR entry 23), not Sora's (36). Its hands and legs are its weapons:
  every hit window is in the body's motion set and switches on the body's shapes of its group.
  Bones are Sora's skeleton's (`kh2-bevy` `rig.rs`: arms 76..99 and 168..191, legs 11..17 and
  40..46). Attack (type 6) entries, by group:
  - 1: sphere r150 on bone 67 (the spine). Windows 154:477, 155:474, Side Spin 161:542 and 161:544;
  - 2: sphere r400 on bone 67 (154:478, 184:555, 184:556);
  - 3: sphere r80 on bone 2 (40:454, 151:476, 183:454 / 455, 184:455);
  - 4: right hand, spheres r55 on bone 85 and r35 on bone 110 (151:475 / 530, 152:459, 182:548);
  - 6: left hand, r55 on 177 and r35 on 202 (53:124, 151:456, 153:534, 181:549);
  - 7: right leg, r60 on 18 and r40 on 11 (152:457 / 531, 181:550);
  - 8: left leg, r60 on 47 and r40 on 40 (152:531..533, 153:460, 181:551, 182:545 / 547);
  - 10: sphere r120 on bone 2 (153:458, Side Spin's spin 161:543, 181:552);
  - 11: a column on the root, r100 (183:553); 12: r80 on 41 and r50 on 67 (182:479 / 480 / 546);
  - 13: a column on bone 2, r260 (155:538, 183:554). No group 5 or 9.

  (motion:ATKP.) Every group Anti's moves use has a shape, so all its windows can land. The
  Square-move helper read Anti's table as Sora's: `local/re`'s `.mdlx` files came from a modded
  install (the Epic one's live archives, whose form models carry Sora's 784-byte table; its
  `backup` and the Steam install have the originals). The port reads the Steam install (Mods
  Manager backup first), so this was never wrong in game. At load, `pose::log_shape_groups`
  logs each form's groups, `KH2 Anti Form attack groups b1 b2 ...: each has a shape`, or
  `NO SHAPE for` a group with its windows (`Rig::shape_groups`; a modded model would show it).
  The other forms' groups all have shapes (Data, same install): Valor body 6, both Keyblades
  2, 3, 5, 6, right also 4; Wisdom body 6 (its shots are its script's); Limit body 3, 6, 20,
  Keyblade 1, 2, 4, 6; Final body 2, 3, 6, 19, both Keyblades 1, 2, 4, 6; Sora body 2, 3, 6,
  18, 19, 21, 22, 23, Keyblade 1, 2, 5, 6. `kh2-data` test `every_attack_group_has_a_shape`
  checks it on files in `KH2_LOCAL_DATA`.
- **Square (Data + KH Wiki; the user: "lets add anti-form's square move").** Anti's PTYA rows 11
  and 12 sit where Sora's guard (11, motion 173) and Upper Slash (12, motion 161) are, with
  ability 0 (Sora's need 1 and 18). In Anti's motion set they are its own moves. The KH Wiki's
  Antiform page names them: "By pressing Square on the ground, Sora jumps backwards, spins around
  and flies forwards. If there are enemies he will home in on one and move into a position behind
  them" (Anti Glide), and "By pressing Square in the middle of a ground combo, Sora blazes in a
  circle around the enemy" (Side Spin).
  - **Anti Glide** (row 11, motion 173, F311; 108 frames at 60 fps). No hit and no guard window.
    Invulnerable on frames 0..8; no gravity from frame 2. Its root motion goes back about 270 and
    up about 95 by frame 24, then forward about 1250 by frame 80 (from the start). Turn-to-lock
    15° / 30° / 15° per frame on 52..62 / 62..84 / 84..100, so with a target he turns onto it as
    he passes it. Frame 0: trigger 21 (360°, the start's snap); frame 2: effect 27 and a footstep;
    sounds 503 038 (frame 4) and 503 039 (frame 56). Range 2 (the free state) from frame 92.
  - **Side Spin** (row 12, motion 161, F310; 120 frames). Three hit windows on his body's
    shapes: ATKP 542 (frames 9..19, group 1: power 50, physical, hit sound 503 003), 543 (26..94,
    group 10: power 20, element 4, re-hit every 8 frames) and 544 (94..120, group 1: power 20,
    element 4). All three have refact self 2 (they go on through a clash). Combo window 78..94,
    turn-to-lock 22° per frame on 0..90, no gravity from 94 (it ends on a hop, root up 67). Effect
    17 at frame 24. Its root motion circles (about 250 forward, swinging ±100 sideways).
  - **Ours, in the port.** Square on the ground, standing or moving, plays Anti Glide: the
    controller's guard-row path takes row 11 when its motion has no guard window (as Valor's,
    whose row 11 is an attack). Square in a ground combo's window plays Side Spin
    (`Character::square_swing`, set by `form::arrive`, `ANTI_SIDE_SPIN_ROW`), one swing of the
    combo as Upper Slash is: only while a swing is left before the finisher. Otherwise Square goes
    to Anti Glide, as Sora's Square goes to his guard. In the air Square does nothing (Anti has no
    air Square row). Both play through the attack path: their motions, root motion,
    invulnerability, turns, hits, effects and sounds come from the data. Neither guards: Anti
    Glide has no guard window, and s283's rule stands.
- **Growth (Data).** 00battle `fmlv`'s Anti rows give no ability at any level, so it has no
  growth ability. None of Sora's growth abilities carry over, as in every form, and the
  one-level-up rule (`form::growth_up`) doesn't apply. His support abilities carry over as in
  the other forms.
- **What it can't do (exe: the command's form mask, as Valor's magic).** No magic, items,
  summons or Drive / Revert. In the port, KH1's specials (Sonic Blade, Strike Raid ...) are
  refused too, as KH2's Limits and reaction commands are. All of them get KH1's "can't" sound:
  - `combat::command` and `exec` refuse the command;
  - `combat::spell_not_yet` refuses the menu entry and the menu stays open;
  - `drive::fill` greys Magic, Items, Summon and the Drive row.
  **Ours:** the field's context commands (talking, examining) stay, as in every form (the
  user's earlier rule for Drive Forms).
- **Damage taken (exe notes, `DriveParams::anti_damage`).** 1.5 times. This is a constant in
  the executable, not in the data. `hurt` raises each attack's damage in KH1's hit record
  (`+8`) by 1.5, rounded up, before KH1 applies it. Drains and heals are left alone.
- **Entering and its end (ours, the user's rules).**
  - It costs no MP and needs none (`form::cost_mp` 0, `mp_enough`).
  - Otherwise it changes as the other forms do: Sora's Drive motion, HP to full, "Give me
    strength!", and the arrival.
  - It runs on Sora's MP as every form here does: the same yellow-MP drain (`form::after_step`,
    one unit every `DRAIN_FRAMES` frames of battle, half as fast out of it) and the same
    `valorstats::DRIVE_MP` bars added while it lasts.
  - Out of MP, it reverts. A KO reverts at once.
  - Revert can't end it: the Drive row is greyed, and `form::choose` and `drive::revert_row`
    refuse it.
  - KH2's own end (its 6-bar form gauge, hits draining it) isn't modelled. KH1 has no Drive
    gauge.
- **Look.** Shadow Sora's look comes from `tint` (separate work). KH1's Keyblade is shrunk
  away on his draw (`leftblade::queue`, `model::hide`). With KH2's player model, the HTLF body
  is drawn.
- **Unchecked in game:** all of it. In particular:
  - Strike Raid's thrown Keyblade as drawn (its turn and spin on KH1's Keyblade, KH1 drawing it
    far from him), its trail, the catch's flash; how far it flies (about 1300 for Raid: KH2's
    30 a frame for the rest of the throw motion);
  - Infinity's ring of shots and their hits, its push;
  - Last Arcanum's glow now on the Keyblade;
  - whether the HTLF effect archive exists on the install;
  - Anti Glide while moving: the exe notes have row 11 tried by the action button "when not
    moving"; it is taken moving too here (no Dodge Roll in Anti; Valor's rule);
  - Anti Glide's homing: how KH2 puts him behind the target (only its root motion and turns
    here, so how far past the target he ends depends on the distance), and its free state from
    frame 92 (range 2 isn't read, so it plays to its end at 108);
  - that Anti's claw and kick hits land and their reach looks right (the shapes above, read
    from the install; the load log line says whether every group found one);
  - the wiki's "can also deflect many attacks automatically": not traced (no guard window in
    Anti Glide or Side Spin);
  - whether `+8` in KH1's hit record is the final damage.

## Limit Form in the KH1 port (Data; the rules ours where marked)

The user: "lets begin working on limit form". Its cost and unlock are the user's: 1 MP to enter,
as Valor and Wisdom (`form::LIMIT_COST_MP`), and open from level 97 (`form::LIMIT_LEVEL`, the same
level check as Final's 99 and the second form's 98). It is the Drive list's third entry, after
Wisdom.

Read from the extracted `00battle.bin`, `03system.bin`, `00objentry.bin`, `msg/us/sys.bar`,
`obj/P_EX100_KH1F.mdlx` / `.mset` and `obj/W_EX010_KH1F.mset`, and from the install's file
lists (`.hed` names). Nothing of it is in the repository.

### Its data

- **Object.** `P_EX100_KH1F` (objentry 0x95D; the `_NM` / `_XM` / `_TR` / `_WI` variants
  0x95E..0x961 share its motion set), weapon joint 1 (right hand bone 178, as Sora's).
- **Moveset 0x42.** `wmst` row 0x42 × 2 gives `W_EX010_KH1F`, right hand only (rows 0x43..0x45
  name it too). One Keyblade: Sora's own, as in base form.
- **Skeleton.** 256 bones, the same as Sora's, so the pose's retarget is unchanged.
- **Movement (`prty` 0x42).** Walk 2, run 8 (Sora's), jump 285 (Sora's 185), turn 0.26, no
  acceleration / brake. `plyr` has only 59 pointers, so there is no record for 0x42. **Ours:**
  `kh2_data::sora::attack_params_for` takes Sora's (auto lock-on 300). Which record KH2 reads
  isn't traced.
- **Model.** BAR entries `p_ex` (model), `tim_` (textures), `limi` (type 3, an object script of
  12148 bytes: its Limit commands, below), and a type-23 entry. It looks like KH1 Sora.
- **Effects and sounds.** `obj/P_EX100_KH1F.a.us` is on the install (in `kh2_sixth`'s list). It
  isn't extracted here, so whether it has a "p_ex" PAX isn't checked; `kh2fx::form_library` falls
  back to the archive's first. The motions' sounds are the Keyblade bank's (500 0xx), plus
  footsteps and voices.
- **Motions.** Body clips 0..19, 40..43, 53, 137, 139, 151..155, 162, 164, 173, 181..183, 191,
  209, 211..213, 221..227, 231..234, 241..248. The tags of the form's own motions start with K
  (K303 = 154, K304 = 155, K322 = 173, K332 = 183, K400.. = 211.., K410.. = 221.., K420.. =
  231.., K430.. = 241..). There is no motion 52 (the Drive change is Sora's), but there is 53
  (its arrival: elements 1..12 and 100..102 from its archive). Motion 42 (the guard's repel, a
  guard window, ATKP 162) and 209 (Dodge Roll) are there. `kh2_data`'s moveset now reads motions
  up to 255, as posing does (it stopped at 230, so Strike Raid's and Infinity's had no clips); no
  other moveset has motions past 230, so the others are unchanged. The Keyblade's motion 235 (no
  body motion) is the thrown Keyblade's own, which its script starts on the weapon.
- **Combo rows (PTYA set 0x42, 21 rows).** No row needs an ability.

  | Row | Kind | Flags | Motion | ATKP (power) | What it is |
  |---|---|---|---|---|---|
  | 0 | 1 | 0xA | 191 | 909 (65, 25) | jumping slash into the air (target 0..260), then the fall |
  | 28, 43 | 1, 2 | | 164 | 968, 975 (100, 25) | the far dash (180 / 300..650) |
  | 6 | 1 | | 153 | 128, 147 | 180..300 |
  | 4 | 2 | | 152 | 127 | 0..180 |
  | 35 | 1 | | 162 | 890 (125) | 0..120 |
  | 5 | 3 | | 151 | 126 | the ground default |
  | 2 | 2 | finisher | 155 | 1870 (200, Magic, element 5, body group 20) | **Ripple Drive** ("when surrounded", "relative to magic"): any target within 500 |
  | 3 | 3 | finisher | 154 | 1877 (600) | **Zantetsuken** ("on a single target ... instantly eliminate weaker enemies"): the default finisher |
  | 10 | 1 | air | 182 | 132 | |
  | 9 | 3 | air | 181 | 131 | the air default |
  | 8 | 3 | air, finisher | 183 | 1871 ×3 (100, 25, 25; re-hit 10), 1872 (100) | **Hurricane Period** ("an aerial barrage") |
  | 11, 12 | 0 | | 173 | guard window 161 | the guard's press (Square standing) |
  | 39 | 0 | | 154 | 1877 | the **Zantetsu Counter** ("Unleashes Zantetsuken by selecting Attack while guarding"): KH2's counterguard row |
  | 1 | 0 | | 181 | | |
  | 52..56 | 0 | air | 242..246 | | Infinity's (by id only) |

  The finisher names come from the ability items 573 Zantetsuken, 574 Ripple Drive and 575
  Hurricane Period, matched to the rows by their help text and ATKP. KH2 gates none of them by
  ability.
- **Growth: Dodge Roll (Data).** Items 564..567 (Dodge Roll LV1..MAX, ability numbers 181..184).
  00battle `fmlv` form 3 (Limit) gives the growth level 1, 1, 2, 2, 3, 3, 4 at form levels 1..7.
  Its other level-up abilities are Lv2 Auto Limit (568), Lv4 Draw (405) and Lv6 Lucky Lucky (407).
  Its level EXP column reads 3, 6, 12, 19, 23, 27. Its experience rule (sys.bar): "For each limit
  command finishing move unleashed in Limit Form, 1 point of experience is gained". Dodge Roll's
  invulnerable frames by level (`fmab`): 10, 16, 24, 32.
- **Drive cost in KH2 (for reference).** 4 bars: 03system `cmd` 0x2A1 "Limit Form" (execute 4,
  argument 3, the form id), and 0x2A2 "Limit" (Auto Limit's reaction command), both cost 4.

### Its commands in KH2 (Data, and its script `limi` read)

- **Magic is replaced by "Limit"** (`cmd` 0x2C3, execute 707, submenu 12, icon 7). Its list:

  | Limit | `cmd` | MP | Follow-ups (`cmd`, no cost) | Help (sys.bar) | Motions (from `limi`'s motion calls) |
  |---|---|---|---|---|---|
  | Sonic Rave | 0x2BA (698) | 60 | Rave 0x2BB, Finish 0x2BC | "attack on an enemy as you pass by. Press [it] at the right tempo for a combo" | 211 (K400) into 212 (K401), 212 again per Rave, 213 (K402) the Finish |
  | Last Arcanum | 0x2BD (701) | 75 | Bash 0x2BE, The End 0x2BF | "a powerful combo attack. Press [it] for a consecutive combo" | 221 (K410), 222..226 one per Bash, 227 (K416) The End |
  | Strike Raid | 0x2C0 (704) | 65 | Raid 0x2C1, Judgment 0x2C2 | "throw the Keyblade at the target. Press [it] for a combo" | 231 (K420), the throw 232 (Raid) / 233 (Judgment), the catch 234; the Keyblade's own 235 |
  | Infinity | 0x2AB (683) | 80 | Shoot 0x2AC, Impact 0x2AD, Giga Impact 0x2AE | "can be unleashed by pressing [it] after an aerial combo" | 241 (K430), combo rows 52..56 (242..246), 247 (Shoot, Impact) / 248 (Giga Impact) |

  `00battle` `limt` rows 27..30 name these limits' files (`sonic_rave.bar`, `last_alcanam.bar`,
  `strike_raid.bar`, `ragnarok.bar`). On the install those are `limit/us/*.bar`, all four 72480
  bytes. That is likely their menu banners: they aren't extracted, so this isn't checked.
  Infinity's file is Ragnarok's, and its `cmd` 0x2AB is KH2FM's Ragnarok under its PC name.
- **Their MP.** The costs are plain costs (not 255, "all remaining MP"). All of KH2's limits
  that cost MP (these four, and the party limits 0x21E / 0x220 at 50) have flag 0x10000 and no
  spell has it; what it does to a cost above Sora's MP isn't traced (no KH2 executable here). The
  flags also have 0x2, the Land flag (`reaction-commands.md`: a Land command can't be used in
  the air), on the limits and their follow-ups.
- **Its script** (`limi`, BAR entry type 3 of the model, as Wisdom's `wisd`; work 128 bytes).
  Read from its code:
  - **Trigger 22** starts a limit with a limit record. It asks `trap_limit_start_command` for
    the command, keeps the limit's control block in work memory, starts a thread that offers
    the party member's commands while `trap_limit_time` lasts (none in a solo limit), and runs
    the command's code: that sets the light (`trap_limit_light`, `trap_light_set_color`; other
    colours in world 13), the follow-up pair and count (Sonic Rave: Rave, Finish, 5; Strike
    Raid: Raid, Judgment, 3), and Sora's hook.
  - **The hook** (`trap_obj_hook`, one per object, a new one replacing the old) drives his
    motions: `trap_sysobj_motion_start` / `_push` (a queue; pushing 0 ends the chain in idle),
    waits on `trap_sysobj_motion_is_end` / `_id`, turns him (`trap_obj_wish_dir`: Rave and
    Finish turn him round to pass back through), and pushes him back (`trap_obj_hitback`,
    Infinity's shots). The script checks his object is Limit Form's (`trap_obj_get_entry_id`
    0x95D..0x961) before it moves him.
  - **The main loop** runs while his motion is the one the hook last started (or one of
    Infinity's 241..248). While the count is above 0 it offers the first follow-up during the
    motion's script range 1 (`trap_obj_motion_check_range (obj, 1)`), at 0 the second during
    range 2 (`trap_command_override`, every frame the range is open). When the motion is no
    longer the hook's, the limit ends (`trap_limit_reference` waited on, its targets freed,
    `trap_obj_stop_end_all (17)`); Strike Raid then catches the Keyblade.
  - **Trigger 23** runs a follow-up command chosen (its count down by one): Rave starts 212
    again, Finish 213; Bash 222..226 in turn (a counter in work memory), The End 227; Raid and
    Judgment throw the Keyblade (232 / 233, then the catch 234); Shoot, Impact and Giga Impact
    push him back 50 / 100 / 200 (`trap_obj_hitback`, away from his facing) and start 247 / 247
    / 248 with 10 / 17 / 25 shots.
  - **Trigger 17** with 100 (frame trigger 7 [100, 0] of 247 / 248) fires Infinity's shots (ATKP
    1862, radius 80, homing): below.
  - **Last Arcanum** also starts its own glow on the Keyblade for the whole limit (a thread at its
    start: elements 35 on the weapon root 0x4005 and 36 on the tip 0x4003, 6 and 28 along the
    blade's x), ended when his motion is no longer the hook's.
  - **Strike Raid** (`cmd` 704): the hook plays 231 (K420; it has no follow-up range), then the
    first throw (232, Raid's) by itself. The throw motion's frame trigger 9 (232 frame 8, 233
    frame 27) starts a thread that takes the Keyblade from his hand as an object of its own
    (`trap_party_get_weapon`, `trap_obj_detach`, `trap_obj_motion_unsync`), starts its own motion
    235 on it (its spin) and its trail (element 32 on its blade bone,
    `trap_obj_effect_start_bind_other`), and makes it an attack: **ATKP 1866 radius 160 for
    Raid, 1867 radius 190 for Judgment** (power 200 and 500; s291's notes had the two the other
    way round). Each frame it turns toward the lock-on target at most 5° (`trap_vector_homing`;
    no more once within 120), moves 30 along its heading (`trap_vector_addf`), is kept 120 (Raid)
    / 150 (Judgment) above a floor found within that much below, and its heading becomes 0.8 of
    itself and 0.2 of the step it made (so the homing is weak: the throw goes about where he
    faced). Judgment wobbles it up and down about that path (amplitude 30, a 20-frame period).
    Its turn is `trap_strike_raid_calc_xyzrot` (heading, up) then `trap_obj_set_xyzrot`, the up
    easing from straight up to the throw's own over 33 frames; it moves by
    `trap_obj_move_request`. It flies until it is attached again: 232 / 233 end into the catch
    (234, K423), which puts it back in his hand (`trap_obj_attach`, `trap_party_hand_to_bone`)
    with a flash where it was (element 34, `trap_obj_effect_start_pos`) and ends the trail.
    234's script ranges 1 and 2 (frames 0..38) offer Raid (count 3) then Judgment; the limit
    ends with 234.
  - **`trap_strike_raid_calc_xyzrot (dir, up)`** (exe `0x14053EF90`, read): both normalized
    (`up` in place); an `up` equal to `dir` becomes (0, -1, 0); x = normalize(up × dir), z =
    -dir, `up` is written back as dir × x (square to `dir`), y = normalize(z × x); the rows x, y,
    z are turned into Euler angles (`0x1401AB290`). The script's first call (its result dropped)
    is for the up it writes back, which it then makes point up.
  - **Infinity** (`cmd` 683) has no follow-up pair: the hook plays 241 (K430), then combo rows
    52..55 (242..245) by `trap_obj_party_attack`, each started again on its first frame with the
    next pushed after it, then row 56 (246), whose frame trigger 9 starts its charge (element 20
    on the tip). 246's range 20 offers Shoot (frames 42..72 and 92..100), Impact (72..82) and
    Giga Impact (82..92). The shots (trigger 17): from the Keyblade tip, one per `n` (10, 17,
    25), each sent along his "up" turned about his facing by 2π·i / n (`trap_vector_rot`: a ring
    round his forward axis); three of them, picked at random, go for the lock-on target, the
    others for an enemy the script's target search finds (up to 10 found within 1000, one picked
    at random). Each waits 6 frames, then each frame turns at most 5° toward a point a random
    0.2..0.8 of the way from its target to the side (`trap_vector_inter` with the cross of its
    heading: they spiral in), moves 16, and ends when it hits or after 150..180 frames (a target
    reached within 64 without a hit is dropped for another).
- **The motions' own parts.** Range 16 and 17 are the script ranges 1 and 2 (Sonic Rave's 212:
  frames 16..46; Strike Raid's 234: 0..38). Range 20 (command, 0) offers a command on the
  reaction button while open: Last Arcanum's Bash (221..225) and The End (226), Infinity's Shoot,
  Impact and Giga Impact (246). Range 26 names the limit the motion belongs to. The hits are the
  Keyblade's (`W_EX010_KH1F`): Rave ATKP 1878, Finish 1879, Last Arcanum 1883..1895, Infinity's
  combo 1857..1861, each with the guard row 162. The body's ranges 27 make him invulnerable
  (211 all through, 212 frames 0..12, 213 0..48, 221 0..120, ...). Frame trigger 1 starts the
  form's effect elements (211 [40], 212 [26, 27, 28], 213 [29, 30, 31], ...), 14 its voices,
  the Keyblade's 8 its sounds.
- **Ours (inferred from the script and the data together):** `trap_obj_motion_check_range (obj,
  n)` reads range 15 + n, and `trap_obj_motion_check_trigger (obj, n)` frame trigger 8 + n (the
  throw frames of 232 / 233 and Infinity's 246 carry trigger 9 and no trigger 1). A command the
  motions offer by range 20 goes, when chosen, to the limit's trigger 23 as the script's own
  offers do (they are all `cmd` execute 707). Frame trigger 7 of a limit motion is the script's
  trigger 17 with the object and the trigger's parameters. Not checked against the executable.
- **Read from the executable** (KH2 FM PC, `KINGDOM HEARTS II FINAL MIX.exe`; the trap tables are
  16-byte records of handler and flags, table 0 at RVA `0x752E00`, 1 `0x755370`, 2 `0x756B60`,
  6 `0x787E80`, 8 `0x787530`): `trap_vector_rot (v, axis, angle)` turns `v` about `axis`
  (`0x14041D650`); `trap_vector_inter (a, b, t)` is a + (b − a)·t (`0x14041D490`);
  `trap_vector_homing (v, to, max)` writes `v` turned toward `to` by at most `max` (no
  frametime), a unit vector (`0x14041D3D0`); `trap_random_get (n)` / `_getf (x)` / `_range (a,
  b)` are an integer below n, x · [0, 1), a + (b − a) · [0, 1); `trap_obj_hitback` sets the
  object's push (dist × 0.1 a frame, × 0.9 each frame, `0x1403D4960`, as `character_core`'s
  hit-back).
- **Not read:** ranges 23 (parameters 1, 3, 78, 92), 29, 31, 44 and 49, and frame triggers 16,
  19 and 31 of these motions. Nothing in the script moves the camera.
- **Square.** Standing, it is the guard (row 11, motion 173; KH2's Limit Form keeps the guard).
  Moving, it is Dodge Roll (motion 209). Attack during the guard's repel (motion 42) is the
  Zantetsu Counter (row 39). All three are the controller's own rules (`character_core`), which
  apply to this data as they are.
- **Triangle** is the limits' follow-ups (above) and Auto Limit's reaction command.

### The port (ours, on the rules every form has here)

- `form::Kind::Limit`, `pose::Kh2::limit` (`P_EX100_KH1F` through `load_form`), `drive::FORMS`
  third entry: record 0x81, word 0x1FB. The Drive records' arguments follow the list's order, so
  Final's is now 0x13.
- The change, the revert, the MP drain and its two extra bars (`valorstats::DRIVE_MP`), the
  revert out of MP, on a KO and in a new world, and full HP on entering are all as for every form.
- **Moveset.** KH2's controller runs its moves and combo rows, its movement numbers, its guard,
  Aerial Recovery (motion 40), the Zantetsu Counter and its finishers, as above.
- **Growth.** Dodge Roll at base Sora's level + 1, MAX at most (`form::growth_up`), the others
  none. It follows KH1's menu changes mid-form (`form::abilities`).
- **Effects and sounds.** Its archive is `kh2fx::LIMIT` (frame trigger 1 → `kh2fx::form_event`,
  weapon anchors on its one Keyblade, `form_bone`). Its elements' sounds are in
  `formmagic::element_sounds`, and its swings' sounds and voices in `kh2sound`, as Valor's.
- **Look.** No tint (`tint::refresh(None)`): Sora's drawn model is KH1's, which Limit Form is
  modelled on. With KH2's player model (off since s259) its own body `P_EX100_KH1F.mdlx` is
  drawn (`model.rs`).
- **No magic.** KH1's spells (a shortcut's too) are refused in the form (`combat::run_entry`):
  KH2's Limit Form can't cast.
- **The Limit menu (the user's pick: "KH2's own limits").** `limits.rs`:
  - The top's Magic row reads "Limit" (record 2's word answered by `limits::word`) and opens our
    list in the magic list's place: records 0x82..0x85 (words 0x1FA..0x1F7, spell-type records
    with arguments 0x20..0x23, written over empty ones as the Drive forms are), named Sonic
    Rave, Last Arcanum, Strike Raid, Infinity. KH1's magic list has no cost column, so no cost
    is shown. A row is greyed in the air (the Land flag), with the MP short, or while a limit
    runs (Infinity's shots still flying count).
  - **MP (ours).** Each limit costs one of KH1's bars (KH1's payment `0x1402A39D0` with 100). The
    mod's KH2 spells (Wisdom's, Final's, base Sora's, Reflect) cost KH1's own 1 MP each whatever
    KH2's cost (`reflect::COST`, `kh1_spell_cost`), and KH2's cost / 100 rounded up gives the same
    bar for 60..80, so both rules agree; it is logged when a limit is paid. Short of a bar the
    limit can't be used (KH2's own rule for a cost above his MP isn't traced, above). The form's
    own drain goes on; a limit that spends the last bar runs to its end before the out-of-MP
    revert (that waits for `can_change_form`, never true in a scripted motion).
  - **Running.** Chosen, a limit waits in the list until KH2's move takes a command
    (`can_take_command`, as a spell waits for `can_cast`). Then its bar is paid and `limi` is
    started at trigger 22 with the command (`kh2_magic::Magic::limit`). Each frame after KH2's
    step the script runs (`limits::after_step`, Sora as `LimitWorld`); what it asks is done
    before the next step (`limits::before_step`): `Character::play` for a start, the controller's
    one-slot queue (`push_scripted`) fed from ours for the pushes, `set_facing` for a turn. A
    motion's hook runs before the script's main loop each frame (`kh2_magic`: an object's hook
    runs in its own update, before the scripts that run after it; the other order ended Sonic
    Rave as 212 began).
  - **Triangle.** A follow-up the script offers, or the one the motion's range 20 offers, is put
    in KH1's own Triangle box: the top menu's fourth entry, record 0x86 (word 0x1F6, its name the
    follow-up's), written after KH1's context fill (the fill's tail jump `0x140292320` pointed
    at `limits::context_fill`). Its record has KH1's "run while held" flag (0x10), the only kind
    KH1's step runs from that box; we take only a fresh press (KH1's logical Triangle
    `0x4000000` pressed that frame), once per window: after a follow-up, none is offered until
    its motion starts.
  - **All four run**, with all their follow-ups (`kh2-magic`'s `tests/limits.rs` runs them from
    the real script against the form's moveset: Rave five times, then Finish; Bash five times,
    then The End; Strike Raid's first throw, three Raids and Judgment, the Keyblade off and back
    each time; Infinity's rows 52..56, Shoot, its push and its ten shots).
  - **The script's objects (`kh2-magic`).** Sora's Keyblade is an object of its own to the script
    (`World::weapon`). Detached, `Magic` keeps it (`Loose`: where, how turned, its motion and
    frame) and answers for it itself, so the script moves it (`trap_obj_set_pos`,
    `_move_request`, `_set_xyzrot`) until it is attached again; the host only draws it. A limit
    that ends early (a revert) puts it back. The target traps are ours on the host's enemies
    (`World::foes`): the lock-on target is the lock's (nothing locked: the nearest enemy in
    front, as Wisdom's shots); a search takes the nearest enemy in its range it hasn't found yet
    (KH2's own pick isn't traced).
  - **The thrown Keyblade drawn.** KH1's own Keyblade (its weapon bones, the grip and its chain)
    is moved where the script has it (`limits::pose_hook`, `leftblade::float_grip`): KH2's blade
    bone of the Keyblade's motion 235 at its frame, on the object, put to KH1's grip by the same
    relation that holds it in the hand (`leftblade::held_blade`), as Final Form's floating
    Keyblades are. His hand is empty meanwhile.
  - **Its hits.** The script's attacks (the thrown Keyblade's sphere on its path, Infinity's
    shots) hit KH1's enemies through the spell hit records (`Strikes::spell_strike`, keys
    `1 << 58`), with KH2's ATKP rows; a hit is passed back (a shot ends there).
  - **The push.** `trap_obj_hitback` on Sora is `Character::hit_back` (KH2's: dist × 0.1 a frame,
    decaying × 0.9).
  - **A combo row** (`trap_obj_party_attack`) plays the row's motion as the script's own start;
    its hits and windows are the motion's. A start of the motion that has just begun, on its
    first frame (the script taking over a motion its push started: Sonic Rave's 212, Infinity's
    rows), is not started again (it played its first frame twice, its voice too).
  - **Weapon anchors.** Limit Form's are KH2's own, as Final's: the blade bone where KH1 draws the
    Keyblade (in his hand, or thrown; `limits::drawn_blade`), its turn included, the tip 78 along
    its −x (`kh2fx::form_bone`). They had Valor's rule (the point, Sora's facing for the turn),
    and the script's elements on them started on his hand's tip: Last Arcanum's glow on the
    Keyblade (elements 35 / 36, 6 and 28 along the blade) pointed along his facing, off the
    Keyblade (the user, s291: "the last arcanum vfx being desyncd from the keyblade"). An element
    a spell script starts now starts on its package's anchors (`kh2fx::spell_start`).
- **No KH1 limits in any form** (the user: "KH1 limits that are equipped shouldn't even prompt
  in any drive form. No drive form should be able to use KH1 Limits"). KH1 offers its limits
  (Sonic Blade, Ars Arcanum, Strike Raid, Ragnarok, ...) in its Triangle box: `0x14028AF60`,
  called once, from the end of the context fill (`0x140292B12`), checks up to five equipped ones
  (`0x14052904C`, the bits at `0x14299CC40`) against the target's distance and their MP
  (`0x1402A2A80`) and puts the best in the box (`0x140290E70 (3, id)`, the id from
  `0x140529041`; greyed by `0x140290E20` when the MP is short). In any Drive Form that call is
  skipped (`limits::specials_prompt`): they never show. The refusal of every KH1 move but Attack,
  magic and items in a form (`combat::command`, `exec`, s278) stays as the backstop. Limit
  Form's KH2 limits are the only limits in a form.
- **Unchecked in game:** all of it. In particular:
  - whether the KH1F archive loads as a PAX;
  - the Limit menu's rows and names, and the Triangle box with our record (KH1's step runs it
    only at the menu's top level);
  - the follow-ups' timing against KH2's (the press reaches the script one or two frames later);
  - the limits' hits (KH2's ATKP powers on KH1's enemies) and their invulnerability;
  - KH1's limits gone from the Triangle box in every form;
  - Ripple Drive's body group 20 shapes on KH1's contact code;
  - Zantetsuken's power 600 against KH1's enemies;
  - Sora's `plyr` stand-in.

## Not modelled

- Form keyblade stats (forms deal Sora's base damage).
- The limits' light (`trap_limit_light`); summons, party members.
- The form change's full-screen effect.
- Drive Recovery items.
