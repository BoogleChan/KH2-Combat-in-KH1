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
run out when you're out of MP, so sora will revert when he is out of MP".

- **How he gets it (ours).** Only by the secret button code (`form::pad`, read in `probe::pad`).
  It is never in the Drive list. The code is taken when KH2 could change him (free, jumping,
  falling, landing or at a move's end) and he isn't in a form or changing. KH2's Anti roll (the
  counter above) isn't used.
- **Its data (Data, checked on extracted files).** `P_EX100_HTLF`, moveset 6:
  - the same 256-bone skeleton as Sora's (`pose::load_form` checks this);
  - no weapon sets (`wmst` has none for moveset 6), so it holds no Keyblade. Its attack windows
    are on its body's own shapes (groups 1, 10, 13);
  - `prty`: run 16, jump 185, turn 0.26;
  - 13 combo rows. Rows 11 (motion 173) and 12 (motion 161) are kind 0, reached only by id;
  - motion 40 (Aerial Recovery) but no motion 42 (the guard's), so no guard;
  - its own sounds (bank 503, frame-trigger sounds) and frame-trigger effects from its own
    archive (`obj/P_EX100_HTLF.a.us`, `kh2fx::ANTI`).
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
  - whether the HTLF effect archive exists on the install;
  - Square standing: KH2's exe tries row 11 for it, but our controller ties that to the guard
    ability, so motion 173 (a short invulnerable move with no attack or guard window) isn't
    played;
  - what reaches row 12 (motion 161) in KH2;
  - whether `+8` in KH1's hit record is the final damage.

## Not modelled

- Form keyblade stats (forms deal Sora's base damage).
- Limit Form's limit commands, summons, party members.
- The form change's full-screen effect.
- Drive Recovery items.
