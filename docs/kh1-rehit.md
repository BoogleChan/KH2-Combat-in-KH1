# KH1: how one attack hits an actor again

Found in Ghidra (the user's Epic exe) while looking at the user's report that Sephiroth's flame
pillar killed Sora "instantly", even with Second Chance-style protection. Confidence: medium
(the static reading is solid; the pillar's own interval hasn't been measured in game).

## Once per attack, re-armed on an interval

- Each attack has a bit. Queuing a hit record on a target (`0x1402BD480(attack, bit, target)`)
  does nothing if the target's `+0x3E0` already has the bit, and sets the bit otherwise. So an
  attack hits each actor once.
- An attack with an interval (`+0x50`, a float, > 0) adds the frame time to `+0xA4` each frame
  (`0x1402B11F0`). Each time `+0xA4` reaches the interval, the attack's bit (`+0x54`) is cleared
  from every actor's `+0x3E0` and from every hitbox's `+0x9C` mask, and the attack can hit again.
  This is how a lasting attack (a pillar, a stream) hits more than once.
- `0x1402B22D0` masks every actor's `+0x3E0` with `0x142DB8110` (the bits still in use).
- The hit check `0x1402B1A50` also skips a hitbox pair when the target's hurtbox has flag
  `0x200000` or `0x400` set, or when the target actor's `+0x3E0` has the attack's bit (`[0x15]`).

## Taking hits

- KH1's hit pass `0x140299A40` ignores a record when the record has `0x20` (no effect), the
  actor's flags `+0` has `0x200` (set while carried or grabbed: `0x1402AAC90`, `0x1402C3BA0`), or
  the stats' `+0` has `4` (dying).
- Second Chance (ability `0x40` in the equipped list at `0x1428695A0 + 0x44`): with HP full and
  the damage at least the HP, the damage becomes HP − 1. Only the first hit from full HP is
  spared.

## What the mod does

KH1's knock-back reactions move Sora out of a lasting attack. KH2's reactions, played in their
place (`hurt.rs`), had no invulnerable frames (Sora's KH2 hurt and blow clips, motions 10–19,
carry no hurtbox-off ranges). So every re-arm of the pillar landed and its damage was taken.
KH2's engine protects a downed Sora, and `hurt.rs` now does the same:
- while KH2 has him in a blow (launched, landed, getting up), no hit lands;
- while he flinches, the same attacker's hits within 20 game frames of the last one don't land.
