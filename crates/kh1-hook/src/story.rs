//! Story and battle flags from KH1's save block (`win::menu::save_block`), and what they open:
//! the Drive forms (`win::form::unlocked`) and Reflect's level (`win::reflect`). The user:
//!
//! - first Drive form after Guard Armor, second after Opposite Armor, third after the Pegasus Cup
//!   (its final is Leon & Yuffie), each by the Dive to the Heart's choice (`FORM_TABLE`);
//! - Master Form after Anti-Sora (the dark Sora of Neverland's ship hold), Final Form after
//!   Ansem-Riku (Riku 2, Hollow Bastion);
//! - Reflect found after Pot Centipede, the Hercules Cup (its final is Hercules) and Ice Titan
//!   (the Gold Match): the first gives Reflect, the second Reflera, the third Reflega.
//!
//! How each flag was found (`docs/drive-model.md`, "When the forms open", has the details): KH1's
//! event scripts store a world's story byte with an opcode whose target `0xB04 + world` the
//! script VM (`0x14019DF40`, store-byte class) maps to `save + 0xA00 + target`, so to `save +
//! 0x1504 + world`; the Heartless journal's bits are set by script call 0x206 (`0x1401B3830`) at
//! `save + 0xC00 + 0xAE3 + id / 8`, the high bit first; the Coliseum's cup states are bytes the
//! cup scripts step from 0 to 3. Each value was read from the user's own install (the scripts of
//! the room each fight is in) and checked against the user's saves and KH1's own per-event save
//! snapshots (`event_data/*`), which hold the save as it is before and after each fight.

/// The worlds' story bytes (12), in KH1's world order.
pub const PROGRESS: usize = 0x1504;
pub const TRAVERSE_TOWN: usize = 0;
pub const AGRABAH: usize = 4;
pub const NEVERLAND: usize = 9;
pub const HOLLOW_BASTION: usize = 10;

/// Guard Armor beaten: Traverse Town's byte goes 0x28 → 0x2B in the Third District's script
/// once the fight is over (its enemies cleared, its prize given), the same value the bosses'
/// gifts use (`growth::BOSS_GIFTS`).
pub const GUARD_ARMOR_TT: u8 = 0x2B;
/// Pot Centipede beaten: Agrabah's byte set to 0x35 right after its prize (`growth::BOSS_GIFTS`).
pub const POT_CENTIPEDE_AG: u8 = 0x35;
/// Anti-Sora beaten: the hold's event at Neverland 0x1E sets 0x28 as it starts (the fight is in
/// it), and the next room's sets 0x32 as Sora walks in; 0x32 is the first byte only reached
/// after the fight.
pub const ANTI_SORA_NL: u8 = 0x32;
/// Ansem-Riku (Riku 2) beaten: after Dragon Maleficent (0x6E) the Grand Hall's script runs the
/// fight and its scenes and ends by setting Hollow Bastion's byte to 0x82.
pub const RIKU_2_HB: u8 = 0x82;

/// The Heartless journal (`save + 0x16E3`, one bit per entry, the high bit first).
pub const JOURNAL: usize = 0x16E3;
/// Opposite Armor's journal entry: added by the Second District's script once the fight is won
/// (no story byte changes then: Traverse Town stays at 0x4E from the bell to the fight).
pub const OPPOSITE_ARMOR_ENTRY: u8 = 0x9F;
/// Ice Titan's journal entry: added as the Gold Match starts.
pub const ICE_TITAN_ENTRY: u8 = 0xA6;

/// The Coliseum's cups' states (`save + 0xF36 + cup`: Phil, Pegasus, Hercules, Hades): 0 not
/// won, 1 won, 2 and 3 its other two rounds won after that.
pub const CUPS: usize = 0xF36;
pub const PEGASUS_CUP: usize = 1;
pub const HERCULES_CUP: usize = 2;

/// The inventory (`save + 0x499 + id`) and Diamond Dust, the Gold Match's prize.
pub const INVENTORY: usize = 0x499;
pub const DIAMOND_DUST: u8 = 0x65;

/// The Dive to the Heart's choice (`save + 0x48C`): 0 the Sword, 1 the Shield, 2 the Rod (Wand).
pub const DIVE_CHOICE: usize = 0x48C;

/// The bytes `Flags::read` needs from the save block's start.
pub const SPAN: usize = JOURNAL + 0x20;

/// The flags (each true once that fight is won).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub guard_armor: bool,
    pub opposite_armor: bool,
    pub pegasus_cup: bool,
    pub anti_sora: bool,
    pub riku_2: bool,
    pub pot_centipede: bool,
    pub hercules_cup: bool,
    pub ice_titan: bool,
}

fn journal(save: &[u8], entry: u8) -> bool {
    save[JOURNAL + (entry >> 3) as usize] & (0x80 >> (entry & 7)) != 0
}

impl Flags {
    /// From the save block's first `SPAN` bytes (`None` if shorter).
    pub fn read(save: &[u8]) -> Option<Flags> {
        if save.len() < SPAN {
            return None;
        }
        let progress = |world: usize| save[PROGRESS + world];
        Some(Flags {
            guard_armor: progress(TRAVERSE_TOWN) >= GUARD_ARMOR_TT,
            opposite_armor: journal(save, OPPOSITE_ARMOR_ENTRY),
            pegasus_cup: save[CUPS + PEGASUS_CUP] >= 1,
            anti_sora: progress(NEVERLAND) >= ANTI_SORA_NL,
            riku_2: progress(HOLLOW_BASTION) >= RIKU_2_HB,
            pot_centipede: progress(AGRABAH) >= POT_CENTIPEDE_AG,
            hercules_cup: save[CUPS + HERCULES_CUP] >= 1,
            // (The journal entry comes as the match starts and Diamond Dust only from winning it:
            // both, so neither a lost match nor a Diamond Dust from elsewhere counts.)
            ice_titan: journal(save, ICE_TITAN_ENTRY) && save[INVENTORY + DIAMOND_DUST as usize] >= 1,
        })
    }

    /// How many of Reflect's three fights are won: 0 none (no Reflect yet), 1 Reflect, 2 Reflera,
    /// 3 Reflega.
    pub fn reflect_level(&self) -> u8 {
        [self.pot_centipede, self.hercules_cup, self.ice_titan].iter().filter(|b| **b).count() as u8
    }

    /// The flags as bits (for "logged when it changes").
    pub fn bits(&self) -> u8 {
        [self.guard_armor, self.opposite_armor, self.pegasus_cup, self.anti_sora, self.riku_2, self.pot_centipede, self.hercules_cup, self.ice_titan]
            .iter()
            .enumerate()
            .fold(0, |b, (i, on)| b | (*on as u8) << i)
    }

    /// One line for the log.
    pub fn describe(&self) -> String {
        let s = |b: bool| if b { "beaten" } else { "not yet" };
        let w = |b: bool| if b { "won" } else { "not yet" };
        format!(
            "Guard Armor {}, Opposite Armor {}, Pegasus Cup {}, Anti-Sora {}, Riku 2 {}; Pot Centipede {}, Hercules Cup {}, Ice Titan {}",
            s(self.guard_armor),
            s(self.opposite_armor),
            w(self.pegasus_cup),
            s(self.anti_sora),
            s(self.riku_2),
            s(self.pot_centipede),
            w(self.hercules_cup),
            s(self.ice_titan)
        )
    }
}

/// The Drive list's forms (Anti is the secret code's, always open: not here).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Valor,
    Wisdom,
    Limit,
    Master,
    Final,
}

/// The Dive's choice as `FORM_TABLE`'s column: 0 Sword, 1 Shield, 2 Rod; anything else the
/// Sword's (KH1 writes only 0..2).
pub fn choice_column(choice: u8) -> usize {
    if choice <= 2 {
        choice as usize
    } else {
        0
    }
}

pub fn choice_name(choice: u8) -> &'static str {
    ["Sword", "Shield", "Rod"][choice_column(choice)]
}

/// The user's table: the first, second and third forms (after Guard Armor, Opposite Armor and the
/// Pegasus Cup) for each Dive choice (Sword, Shield, Rod).
pub const FORM_TABLE: [[Form; 3]; 3] = [
    // Sword
    [Form::Valor, Form::Limit, Form::Wisdom],
    // Shield
    [Form::Limit, Form::Wisdom, Form::Valor],
    // Rod (Wand)
    [Form::Wisdom, Form::Valor, Form::Limit],
];

/// Whether `form` is open with this Dive choice and these flags.
pub fn unlocked(form: Form, choice: u8, f: &Flags) -> bool {
    match form {
        Form::Master => f.anti_sora,
        Form::Final => f.riku_2,
        _ => {
            let tiers = FORM_TABLE[choice_column(choice)];
            [f.guard_armor, f.opposite_armor, f.pegasus_cup].iter().zip(tiers).any(|(beaten, tier)| *beaten && tier == form)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save() -> Vec<u8> {
        vec![0; SPAN]
    }

    fn with_journal(s: &mut [u8], entry: u8) {
        s[JOURNAL + (entry >> 3) as usize] |= 0x80 >> (entry & 7);
    }

    fn flags(ga: bool, oa: bool, peg: bool) -> Flags {
        Flags { guard_armor: ga, opposite_armor: oa, pegasus_cup: peg, ..Default::default() }
    }

    const MIDDLE: [Form; 3] = [Form::Valor, Form::Wisdom, Form::Limit];

    fn open(choice: u8, f: &Flags) -> Vec<Form> {
        MIDDLE.iter().copied().filter(|&m| unlocked(m, choice, f)).collect()
    }

    #[test]
    fn nothing_before_guard_armor() {
        for choice in 0..3 {
            assert!(open(choice, &flags(false, false, false)).is_empty());
            assert!(!unlocked(Form::Master, choice, &Flags::default()));
            assert!(!unlocked(Form::Final, choice, &Flags::default()));
        }
    }

    #[test]
    fn sword() {
        assert_eq!(open(0, &flags(true, false, false)), [Form::Valor]);
        assert_eq!(open(0, &flags(true, true, false)), [Form::Valor, Form::Limit]);
        assert_eq!(open(0, &flags(true, true, true)), [Form::Valor, Form::Wisdom, Form::Limit]);
    }

    #[test]
    fn shield() {
        assert_eq!(open(1, &flags(true, false, false)), [Form::Limit]);
        assert_eq!(open(1, &flags(true, true, false)), [Form::Wisdom, Form::Limit]);
        assert_eq!(open(1, &flags(true, true, true)), [Form::Valor, Form::Wisdom, Form::Limit]);
    }

    #[test]
    fn rod() {
        assert_eq!(open(2, &flags(true, false, false)), [Form::Wisdom]);
        assert_eq!(open(2, &flags(true, true, false)), [Form::Valor, Form::Wisdom]);
        assert_eq!(open(2, &flags(true, true, true)), [Form::Valor, Form::Wisdom, Form::Limit]);
    }

    #[test]
    fn each_tier_alone() {
        // Out of order (a randomizer's): each fight opens its own column's form only.
        for choice in 0..3u8 {
            let col = FORM_TABLE[choice as usize];
            assert_eq!(open(choice, &flags(false, true, false)), [col[1]]);
            assert_eq!(open(choice, &flags(false, false, true)), [col[2]]);
        }
    }

    #[test]
    fn every_choice_gets_every_form_once() {
        for col in FORM_TABLE {
            for m in MIDDLE {
                assert_eq!(col.iter().filter(|&&c| c == m).count(), 1);
            }
        }
    }

    #[test]
    fn unknown_choice_is_the_swords() {
        assert_eq!(open(7, &flags(true, false, false)), [Form::Valor]);
        assert_eq!(choice_name(7), "Sword");
    }

    #[test]
    fn master_and_final() {
        let f = Flags { anti_sora: true, ..Default::default() };
        assert!(unlocked(Form::Master, 0, &f) && !unlocked(Form::Final, 0, &f));
        let f = Flags { riku_2: true, ..Default::default() };
        assert!(unlocked(Form::Final, 2, &f) && !unlocked(Form::Master, 2, &f));
    }

    #[test]
    fn read_thresholds() {
        let mut s = save();
        assert_eq!(Flags::read(&s), Some(Flags::default()));
        s[PROGRESS + TRAVERSE_TOWN] = GUARD_ARMOR_TT - 3;
        assert!(!Flags::read(&s).unwrap().guard_armor);
        s[PROGRESS + TRAVERSE_TOWN] = GUARD_ARMOR_TT;
        assert!(Flags::read(&s).unwrap().guard_armor);
        // Traverse Town at 0x4E (the bell rung) isn't Opposite Armor: its journal entry is.
        s[PROGRESS + TRAVERSE_TOWN] = 0x4E;
        assert!(!Flags::read(&s).unwrap().opposite_armor);
        with_journal(&mut s, OPPOSITE_ARMOR_ENTRY);
        assert!(Flags::read(&s).unwrap().opposite_armor);
        assert_eq!(s[0x16F6], 0x01);
        s[PROGRESS + NEVERLAND] = 0x28;
        assert!(!Flags::read(&s).unwrap().anti_sora);
        s[PROGRESS + NEVERLAND] = ANTI_SORA_NL;
        assert!(Flags::read(&s).unwrap().anti_sora);
        s[PROGRESS + HOLLOW_BASTION] = 0x6E;
        assert!(!Flags::read(&s).unwrap().riku_2);
        s[PROGRESS + HOLLOW_BASTION] = RIKU_2_HB;
        assert!(Flags::read(&s).unwrap().riku_2);
        s[CUPS + PEGASUS_CUP] = 1;
        assert!(Flags::read(&s).unwrap().pegasus_cup);
        assert!(Flags::read(&s[..SPAN - 1]).is_none());
    }

    #[test]
    fn reflect_levels() {
        let mut s = save();
        assert_eq!(Flags::read(&s).unwrap().reflect_level(), 0);
        s[PROGRESS + AGRABAH] = POT_CENTIPEDE_AG;
        assert_eq!(Flags::read(&s).unwrap().reflect_level(), 1);
        s[CUPS + HERCULES_CUP] = 1;
        assert_eq!(Flags::read(&s).unwrap().reflect_level(), 2);
        // The Gold Match: its journal entry alone (started) isn't a win, nor Diamond Dust alone.
        with_journal(&mut s, ICE_TITAN_ENTRY);
        assert_eq!(Flags::read(&s).unwrap().reflect_level(), 2);
        s[INVENTORY + DIAMOND_DUST as usize] = 1;
        assert_eq!(Flags::read(&s).unwrap().reflect_level(), 3);
        // Any one of the three is the first.
        let f = Flags { ice_titan: true, ..Default::default() };
        assert_eq!(f.reflect_level(), 1);
    }

    #[test]
    fn bits_change_with_each_flag() {
        let mut seen = vec![Flags::default().bits()];
        let all = [
            Flags { guard_armor: true, ..Default::default() },
            Flags { opposite_armor: true, ..Default::default() },
            Flags { pegasus_cup: true, ..Default::default() },
            Flags { anti_sora: true, ..Default::default() },
            Flags { riku_2: true, ..Default::default() },
            Flags { pot_centipede: true, ..Default::default() },
            Flags { hercules_cup: true, ..Default::default() },
            Flags { ice_titan: true, ..Default::default() },
        ];
        for f in all {
            assert!(!seen.contains(&f.bits()));
            seen.push(f.bits());
        }
    }
}
