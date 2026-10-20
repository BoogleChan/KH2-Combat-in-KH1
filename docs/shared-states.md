# Shared states (testing branch)

`shared.rs`, ini `shared = on` (default). This replaces the old model for base Sora. Under the
old model, KH2's controller had him whenever KH1 showed him standing, walking or running, and
KH1's own idle, walk, run and jump were drawn back over KH2's when a move style was KH1. Drive
Forms still use the old model: KH2's controller has them all through.

## The rule

The user (2026-10-20): "The goal is to share the animation state and logic between both
versions while allowing KH2 to handle its own unique physics and movement properties."

| Move | Owner | At KH1 | At KH2 |
|---|---|---|---|
| idle, walk, run, item use (Misc Animations) | KH1's states | KH1 unmodded | KH1's slot drawn with KH2's motion; KH2's run speed |
| jump, high jump, fall, landing (Jump) | KH1's states | KH1 unmodded | KH1's slots drawn with KH2's jump / fall / landing; KH2's jump arc |
| Aerial Dodge | KH2 borrows him | double jump with Jump at KH1 too | the same |
| Glide | KH1's glide (Square, or held jump with `glide = jump`) | KH1's | KH2 borrows him for its glide |
| Dodge Roll (Square on the ground) | KH1 at KH1; KH2 borrows at KH2 | KH1's roll and guard | KH2's roll, guard, Quick Run |
| Base / Aerial Combo | KH1's rows at KH1; KH2 borrows at KH2 | KH1's combo | KH2's combo (for now) |
| Magic / Aerial Magic | KH1 casts at KH1; KH2 borrows at KH2 | KH1's spells | KH2's spells; KH1-only spells keep KH2's cast motions |
| Reflect, Drive | KH2 borrows him | always KH2's | always KH2's |
| KH1's specials, summons | KH1 | unmodded | unmodded |

## How it runs

- **KH1 has him** (a hand-off of ours: `shared::active`). KH1's control step sees the pad as
  unmodded, with these exceptions (`shared::control`, `takeover::shared_control`):
  - Square on the ground with Dodge Roll at KH2.
  - Square in the air with Glide at KH2 (`glide = square`).
  - A fresh jump in the air when Aerial Dodge is equipped (once per air time), or with Glide at
    KH2 and `glide = jump`.
  - In the air past the rise, the held jump and KH1's action bit unless KH1's glide is on the
    jump button.

  Those presses are borrows. KH1's glide on Square is started from KH1's fall state
  (`kh1_glide_start`) and held by Square.
- **A borrow** happens only on a frame KH1 lets him act (KH1's can-act test; no buffer):
  - the Attack command with the combo style at KH2 where he is (`combat::exec` → `ATTACK_AFTER`);
  - a command of KH2's (`combat::command` → `shared::want_kh2`, then `resume_command` runs it
    once KH2 has him);
  - the button borrows above.

  `takeover::shared_frame` hands KH2's controller a fresh start:
  - in the air, with his speed;
  - the borrowing press is fresh on KH2's first frame.
- **Give-back** happens after KH2's step (`shared::give_back`):
  - **To the ground:** two frames in idle, walk, run or landing, on the ground, with no combo
    going (`Character::combo_going`) and no command waiting. KH1's ground state takes over.
  - **To the air:** two frames in KH2's plain fall (a combo over or 24 frames past its last
    swing). KH1's fall state takes over, set as an air command sets it (the same fields as
    `combat::command`).
- **KH1's own moves** (a spell, an item, a ledge, a ladder) go on in KH1's states. A command
  started from a KH2 move keeps its cut windows until KH1 lets him act.
- **A jump out of KH2's move** (`JUMP_OUT`): on the ground, where KH2's move can be left, the
  press is let through to KH1's control step. KH1 jumps him, and its states have him from there.
- **The draw** (`pose::wanted`, mode 0): KH2's controller's pose while it has him; otherwise
  KH1's slot through `pose::follow`, KH2's motion where `shared::kh2_look` says the slot's
  style is KH2's (`shared::slot_kind`), KH1's own otherwise.

## Not yet

- Base / Aerial Combo, Magic and Dodge Roll at KH2 are KH2's own moves (borrows), not yet KH1's
  states drawn with KH2's motions.
- KH2's physics on KH1's states: see `shared::physics`.
