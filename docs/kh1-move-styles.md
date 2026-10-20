# Move styles (KH1 or KH2, per kind of move)

`styles.rs`: Ground Combo, Aerial Combo, Magic, Aerial Magic, each KH1 or KH2, for base Sora
only (Drive Forms keep their own KH2 movesets). Read from `kh1-kh2.ini` (`ground_combo`,
`air_combo`, `magic`, `air_magic`), switched in game from the Config menu. Defaults: combos KH2,
magic KH1 (as before the switches).

## Combos

`stylecombo.rs`. With the combo where Sora is set to KH1 (on the ground: Ground Combo; in KH2's
air: Aerial Combo), the Attack command is not KH2's attack button:

- **Start.** KH1's command step (`0x1402A5380`, hooked as `combat::command`) runs it as KH1's
  other commands: KH2 must be able to take a command (`can_take_command`, else it waits as they
  do); in KH2's air Sora is first put in KH1's air state 2 (the air-time fields, KH2's sideways
  speed), so KH1 picks its air row. Our call of the step marks `STARTING`, so the attack row it
  runs (`0x1402A4760`, hooked as `combat::exec`) is KH1's own instead of KH2's press. KH1 then
  has him (`hand_to_kh1`, `FROM_COMMAND`), and `stylecombo::started` marks the hand-off as a
  combo's.
- **During.** Attack presses stay in KH1's command record (`take_chosen_command` leaves them) and
  its rows run (`exec`), so KH1 chains its combo, finishers and abilities as unmodded. None of the
  spell cuts (`takeover`'s `out_any`): no jump / Square / spell out of a swing; a spell chosen
  meanwhile waits for him to be free (`MAGIC_AFTER`), then runs as after any KH1 move. The draw
  is KH1's own swing (`pose`: no KH2 swing over KH1's combo slots 98..104, no stale KH2 pose at
  the start).
- **End.** As after a KH1 special: once KH1 lets him act, the stick, a jump, Square or (in the
  air) his fall hand him back to KH2 (KH2's fall and landing after an air combo). KH2's combo
  starts over afterwards (no `COMBO_ON` carry). Attack while KH1 has him free is KH1's again if
  the style where he is (KH1's air state 1 / 2 or the ground) is KH1's; else it is KH2's press
  and KH2 takes him (mixed styles: a KH1 ground combo then a KH2 air combo, or the other way).
- Any other hand-off (`hand_to_kh1`) clears the combo mark; it holds only while `HANDOFF` and
  `FROM_COMMAND` do.

Unverified in game: that KH1's command step needs nothing more than its state to pick the air
row; that KH1's can-act test opens only after a swing's recovery (so the stick doesn't hand him
back between swings); KH1's own hit reactions mid-combo.

## Magic

Which style applies is picked when the spell is chosen (or KH1 starts it): Aerial Magic if KH2
has him in the air then, else Magic (`formmagic::base_kh2`).

- **KH1** (default): as before. KH1 casts every spell; Reflect is KH2's.
- **KH2**:
  - Fire, Blizzard, Thunder, Cure are KH2's base-form spells (`formmagic::spell_chosen`, the
    path Wisdom's spells take): the spell's MAGC row (world 0) gives base Sora's own cast
    motions (ground, air, combo finisher) in his KH2 moveset, cast through the controller (its
    combo slots, its cast windows); KH2's script `magic/<SPELL>_<level>.mag` at base Sora's magic
    sub (level × 2 − 2, + 1 for the finisher; Wisdom's are + 10, `form::magic_sub`), its effects
    and sounds, its attacks through the spell hit records with KH1's own data for the spell at
    its level (element). Level: KH1's spell level; cost: KH1's (1 MP). Cure heals a share of
    KH1's max HP by the ATKP row's power (as Wisdom's did).
  - Gravity, Stop, Aero (no KH2 version): KH1 casts the spell, drawn with base Sora's KH2 cast
    motion (Gravity on Thunder's, Stop and Aero on Cure's, the air ones in the air) and run on
    that motion's clock, input window and movement, as Wisdom's KH1-only spells
    (`takeover::command_started`'s `FormCast`, `pose::follow`, `formmagic::BASE_KH1_CAST`).
  - Aerial Magic at KH2: air casts (Reflect too) use KH2's own air physics (the cast motions'
    no-gravity frames, KH2's gravity), not KH1's fall, as Final Form's.
- The spells' effect packages are loaded with `drive = off` too (they were only with Drive on).
- If a spell's KH2 file or MAGC row is missing, KH1 casts its own.
