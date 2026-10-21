//! Limit Form's limits from its model's own script (`limi`, BAR entry 3 of
//! `P_EX100_KH1F.mdlx`) against a fake Sora whose motions come from the form's
//! moveset (`P_EX100_KH1F.mset`: lengths and script ranges). Skipped unless the
//! files are extracted (`local/re`, or `KH2_RE_DIR`).

use std::collections::HashMap;
use std::path::PathBuf;

use glam::{Mat4, Vec3};
use kh2_formats::motion::Mset;
use kh2_magic::{Event, Magic, ObjId, SpellFile, World};

const SORA: ObjId = 1;
const FOE: ObjId = 2;
/// `00objentry` id of `P_EX100_KH1F` (the script checks for it).
const LIMIT_FORM_ENTRY: u32 = 0x95D;

/// A motion: its length (60 Hz frames) and its ranges (id, start, end, first parameter).
struct Motion {
    len: u32,
    ranges: Vec<(u8, u32, Option<u32>, i16)>,
}

struct Fake {
    motions: HashMap<u16, Motion>,
    motion: u16,
    frame: u32,
    /// Pushed motions, in order (0: idle, the chain's end).
    queued: Vec<u16>,
    facing: f32,
    offers: Vec<u16>,
    started: Vec<u16>,
}

impl Fake {
    fn new(mset: &Mset) -> Fake {
        let mut motions = HashMap::new();
        for m in 0..=255u16 {
            let Some(anb) = mset.slot_for(m as usize, true, true).and_then(|s| s.anb.as_ref()) else { continue };
            let Some(info) = &anb.motion else { continue };
            let len = ((info.frame_end - info.frame_start) / info.fps * 60.0).round().max(1.0) as u32;
            let ranges = anb.triggers.ranges.iter().map(|r| (r.id, r.start.max(0) as u32, (r.end >= 0).then_some(r.end as u32), r.params.first().copied().unwrap_or(0))).collect();
            motions.insert(m, Motion { len, ranges });
        }
        Fake { motions, motion: 0, frame: 0, queued: Vec::new(), facing: 0.0, offers: Vec::new(), started: Vec::new() }
    }

    /// A command the motion itself offers now (range 20: the engine's reaction command).
    fn motion_offer(&self) -> Option<u16> {
        let f = self.frame;
        self.motions.get(&self.motion)?.ranges.iter().find(|&&(id, s, e, _)| id == 20 && f >= s && e.map_or(true, |e| f < e)).map(|r| r.3 as u16)
    }

    fn len(&self) -> u32 {
        self.motions.get(&self.motion).map_or(1, |m| m.len)
    }

    /// One frame of the motion (after the scripts ran, as the host's controller steps).
    fn advance(&mut self) {
        self.frame += 1;
        if self.frame >= self.len() {
            self.frame = 0;
            self.motion = if self.queued.is_empty() { 0 } else { self.queued.remove(0) };
        }
    }
}

impl World for Fake {
    fn exists(&self, _: ObjId) -> bool {
        true
    }
    fn pos(&self, o: ObjId) -> Vec3 {
        if o == FOE {
            Vec3::new(0.0, 0.0, 300.0)
        } else {
            Vec3::ZERO
        }
    }
    fn facing(&self, _: ObjId) -> f32 {
        self.facing
    }
    fn motion(&self, o: ObjId) -> u16 {
        if o == SORA {
            self.motion
        } else {
            0
        }
    }
    fn airborne(&self, _: ObjId) -> bool {
        false
    }
    fn jumping(&self, _: ObjId) -> bool {
        false
    }
    fn bone(&self, o: ObjId, _code: u16) -> Option<Mat4> {
        Some(Mat4::from_translation(self.pos(o)))
    }
    fn target_point(&self, o: ObjId) -> Vec3 {
        self.pos(o) + Vec3::Y * 60.0
    }
    fn target_of(&self, o: ObjId) -> Option<ObjId> {
        (o == SORA).then_some(FOE)
    }
    fn party(&self, n: u32) -> Option<ObjId> {
        (n == 0).then_some(SORA)
    }
    fn has_ability(&self, _: ObjId, _: u32) -> bool {
        false
    }
    fn ground(&self, from: Vec3, depth: f32) -> Option<Vec3> {
        (from.y >= 0.0 && from.y - depth <= 0.0).then(|| Vec3::new(from.x, 0.0, from.z))
    }
    fn bg_hit(&self, _: Vec3, to: Vec3) -> bool {
        to.y < 0.0
    }
    fn hitback(&mut self, _: ObjId, _: Vec3, _: f32) {}
    fn random(&mut self) -> f32 {
        0.5
    }
    fn motion_start(&mut self, o: ObjId, m: u16, _blend: f32) {
        if o == SORA {
            self.motion = m;
            self.frame = 0;
            self.queued.clear();
            self.started.push(m);
        }
    }
    fn motion_push(&mut self, o: ObjId, m: u16, _blend: f32) {
        if o == SORA {
            self.queued.push(m);
        }
    }
    fn motion_ended(&self, _: ObjId) -> bool {
        self.frame + 1 >= self.len() && self.queued.is_empty()
    }
    fn motion_range(&self, _: ObjId, n: u8) -> bool {
        let f = self.frame;
        self.motions.get(&self.motion).is_some_and(|m| m.ranges.iter().any(|&(id, s, e, _)| id == 15 + n && f >= s && e.map_or(true, |e| f < e)))
    }
    fn face(&mut self, _: ObjId, dir: Vec3) {
        self.facing = dir.x.atan2(dir.z);
    }
    fn entry_id(&self, _: ObjId) -> u32 {
        LIMIT_FORM_ENTRY
    }
    fn offer_command(&mut self, _: ObjId, command: u16) {
        self.offers.push(command);
    }
}

fn dir() -> PathBuf {
    std::env::var_os("KH2_RE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/re"))
}

/// Sonic Rave (03system `cmd` 698): K400 (211) into K401 (212); Rave (699) offered in 212's
/// script range 1, each press turning him round into 212 again; after five, Finish (700) in its
/// range 2 instead, into K402 (213); then the limit ends with the motion.
#[test]
fn sonic_rave_runs_with_its_follow_ups() {
    let d = dir();
    let (Ok(mdlx), Ok(mset)) = (std::fs::read(d.join("P_EX100_KH1F.mdlx")), std::fs::read(d.join("P_EX100_KH1F.mset"))) else {
        eprintln!("skipped: no Limit Form files in {}", d.display());
        return;
    };
    let script = SpellFile::bar_entry(&mdlx, 3).expect("limi");
    let mut magic = Magic::new();
    let file = magic.add_spell(SpellFile::script_only("P_EX100_KH1F", &script).unwrap());
    let mset = Mset::parse(&mset).unwrap();
    let mut w = Fake::new(&mset);
    magic.limit(file, SORA, 698, &mut w).unwrap();
    let mut seen: Vec<u16> = Vec::new();
    let mut raves = 0;
    let mut finish = false;
    let mut offered = Vec::new();
    let mut flips = 0;
    let mut taken_at = 0;
    for f in 0..2000 {
        w.offers.clear();
        let facing = w.facing;
        for e in magic.update(&mut w) {
            match e {
                Event::Error(s) => panic!("frame {f}: {s}"),
                Event::Missing(n) => eprintln!("missing {n}"),
                Event::Strike { .. } => {}
            }
        }
        if (w.facing - facing).abs() > 3.0 {
            flips += 1;
        }
        for &c in &w.offers {
            offered.push((w.motion, w.frame, c));
        }
        // Each follow-up taken as soon as it is offered, once per motion start (the host's
        // rule: one press per window; the script starts the next motion on the next frame).
        if let Some(&c) = w.offers.first().filter(|_| w.started.len() > taken_at) {
            taken_at = w.started.len();
            assert!(magic.limit_command(SORA, c));
            if c == 699 {
                raves += 1;
            } else if c == 700 {
                finish = true;
            }
        }
        if seen.last() != Some(&w.motion) {
            seen.push(w.motion);
        }
        if !magic.limit_running(SORA) {
            eprintln!("Sonic Rave over at frame {f}: motions {seen:?}, started {:?}, {raves} Raves, Finish {finish}, flips {flips}", w.started);
            break;
        }
        w.advance();
    }
    assert!(!magic.limit_running(SORA), "the limit never ended: {seen:?}");
    assert_eq!(&w.started[..2], &[211, 212], "{:?}", w.started);
    assert_eq!(raves, 5, "offers {offered:?}");
    assert!(finish, "no Finish: offers {offered:?}");
    assert!(w.started.contains(&213));
    // The Raves in 212's range 1 (16..46), the Finish in its range 2 (the same frames).
    assert!(offered.iter().all(|&(m, f, _)| m == 212 && (16..46).contains(&f)), "{offered:?}");
    assert!(offered.iter().any(|&(_, f, _)| f == 16), "offered from the window's first frame: {offered:?}");
    assert_eq!(flips, 6, "each Rave and the Finish turn him round");
    assert_eq!(w.motion, 0);
}

/// Last Arcanum (`cmd` 701): K410 (221); Bash (702) offered by the motions themselves (range
/// 20), each press the next of K411..K415 (222..226); The End (703) from 226's, into K416 (227).
#[test]
fn last_arcanum_runs_its_combo() {
    let d = dir();
    let (Ok(mdlx), Ok(mset)) = (std::fs::read(d.join("P_EX100_KH1F.mdlx")), std::fs::read(d.join("P_EX100_KH1F.mset"))) else {
        eprintln!("skipped: no Limit Form files in {}", d.display());
        return;
    };
    let script = SpellFile::bar_entry(&mdlx, 3).expect("limi");
    let mut magic = Magic::new();
    let file = magic.add_spell(SpellFile::script_only("P_EX100_KH1F", &script).unwrap());
    let mut w = Fake::new(&Mset::parse(&mset).unwrap());
    magic.limit(file, SORA, 701, &mut w).unwrap();
    let mut taken = Vec::new();
    let mut taken_at = 0;
    for f in 0..3000 {
        w.offers.clear();
        for e in magic.update(&mut w) {
            if let Event::Error(s) = e {
                panic!("frame {f}: {s}");
            }
        }
        let offer = w.offers.first().copied().or_else(|| w.motion_offer());
        if let Some(c) = offer.filter(|_| w.started.len() > taken_at) {
            taken_at = w.started.len();
            assert!(magic.limit_command(SORA, c));
            taken.push(c);
        }
        if !magic.limit_running(SORA) {
            break;
        }
        w.advance();
    }
    eprintln!("Last Arcanum: started {:?}, took {taken:?}", w.started);
    assert!(!magic.limit_running(SORA));
    assert_eq!(w.started, vec![221, 222, 223, 224, 225, 226, 227]);
    assert_eq!(taken, vec![702, 702, 702, 702, 702, 703]);
}
