# Clanks on KH1's Keyblade, and the Aerial Dodge's parry

Addresses are the Epic build's; the Steam build's are in `src/win/steam_map.in`.
Confidence: **Checked (exe)** read from KH1's code; **Ours** a choice of the mod's.

## KH1's swing and the Keyblade's length (Checked, exe)

- Sora's swing rows' attack data put the attack object on bone `0x30` (data `+0x10`), the
  Keyblade, at two offsets along it (data `+0x54` / `+0x58`).
- Making a row's object (`0x1402B10E0`) adds the weapon's own stats when the bone is `0x30`:
  the weapon object (actor `+0x14C`) points at them (`+0x20`): `+0x20` a radius bonus (times
  the actor's size `+0x40`), `+0x2C` a length factor on the two offsets, `+0x33` a priority
  bonus, `+0x31` / `+0x32` critical.
- A point along bone `0x30` (`0x14029C990 (actor, bone, t, out)`): `t` is first mapped into the
  weapon's own blade range, `(ws+0x24 - ws+0x28) * t + ws+0x28`, then taken between the bone's
  parent and the bone through the model instance's world matrices (`0x1401D41C0`). So each
  Keyblade's reach is its own: `t` 0..1 covers its blade from start to tip.

## Our clash test (Ours)

- The right Keyblade's swing meets guards, enemy attacks and walls with KH1's blade as drawn:
  six points `t` 0..1 along bone `0x30` (Sora's drawn matrices), swept from last frame's
  points, each sphere KH1's swing radius for this weapon (row data `+0x5C` + the weapon's radius
  bonus × his size). A shorter Keyblade reaches less far, a longer one farther.
- KH2's wide hit shapes still decide what the swing hits on bodies (KH2's hits); only clashes
  use the blade. Valor's left Keyblade keeps KH2's shapes (thinned to KH1's radius).
- Enemy melee attacks (not only free-standing shots) now meet the blade, through KH1's own
  attack-vs-attack clash (`0x1402B2890`): when KH1 marks Sora's side as clashed (`+4` 0x10 /
  0x20), the swing stops there and his recoil is KH1's kind 6 / 7 record, which `hurt` plays as
  KH2's recoil.
- Walls (ini `wall_clink`): the blade's points from last frame to now through KH1's background
  ray, not KH2's shapes (those reached far beside the blade: clinks "at random moments").
- Each clank is logged with where along the blade it touched (0% the blade's start, 100% its tip).

## The Aerial Dodge's parry (Checked + Ours)

- KH1's attack-vs-attack pass meets attack objects, not the actor: an actor flag that keeps Sora
  out of the hit pass (0x40, his dodges' untouchability) doesn't keep his own attack objects out.
- A guard is an attack object of priority 100 (`0x1402B2A30`: the guard side is unhurt, 0x10; the
  attack meeting it gets a clash record, kind 6 / 7, its owner recoils).
- While KH2's Aerial Dodge plays, Sora has such an object: KH1's guard attack data from his rows
  (the first with priority 100, logged), else his swing's data made a guard (priority 100, clash
  type 0, no power, radius at least 45). Live all through (activity 1), placed each frame on its
  bone at the data's two offsets (its swept segment), freed when the dodge ends and made again
  once spent.
