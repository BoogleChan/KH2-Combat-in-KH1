//! KH2 motions as KH1 slots (`kh1_data::slot`) on the user's own `xa_ex_0010.mset`: the request
//! table maps the glide and the combo to the slots `docs/kh1-anim-slots.md` names, and a retimed
//! template keeps its keys within its new length. Skipped without the file.

use std::path::PathBuf;

fn mset() -> Option<Vec<u8>> {
    let re = std::env::var_os("KH2_RE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/re"));
    let p = std::env::var_os("KH1_MSET").map(PathBuf::from).unwrap_or_else(|| re.join("kh1/xa_ex_0010.mset"));
    match std::fs::read(&p) {
        Ok(b) => Some(b),
        Err(_) => {
            eprintln!("skipped: no {}", p.display());
            None
        }
    }
}

#[test]
fn request_table_slots() {
    let Some(b) = mset() else { return };
    let t = kh1_data::slot::request_table(&b).unwrap();
    let slot = |r: usize| t[r] & 0xFFF;
    assert_eq!([slot(0x71), slot(0x72), slot(0x73), slot(0x74)], [75, 76, 77, 78]);
    assert_eq!([slot(0x76), slot(0x79)], [75, 78]);
    assert_eq!([slot(0xC8), slot(0xC9), slot(0xCA), slot(0xCB)], [98, 99, 100, 101]);
    assert_eq!(slot(0xDC), 117);
    assert_eq!(slot(9), 12);
}

#[test]
fn retimed_second_swing() {
    let Some(b) = mset() else { return };
    let m = kh1_data::slot::motion_bytes(&b, 99).unwrap();
    let ev = kh1_data::slot::events(&m).unwrap();
    let hit = ev.iter().find(|t| t.kind == kh1_data::slot::HIT).unwrap();
    assert_eq!((hit.keys[0].time, hit.keys[1].time), (18.0, 32.0));
    let new = kh1_data::slot::scaled(&ev, 56.0 / 48.0);
    let r = kh1_data::slot::retimed(&m, 56, &new).unwrap();
    assert_eq!(kh1_data::slot::events(&r).unwrap(), new);
    // Every key within the new length (key times in 1/256 frames, the head's high 24 bits).
    let keys = u32::from_le_bytes(r[0x28..0x2C].try_into().unwrap()) as usize;
    let (mut old_last, mut last) = (0, 0);
    for k in 0..200 {
        let p = keys + k * 16;
        old_last = old_last.max(i32::from_le_bytes(m[p..p + 4].try_into().unwrap()) >> 8);
        last = last.max(i32::from_le_bytes(r[p..p + 4].try_into().unwrap()) >> 8);
    }
    eprintln!("last key: {} -> {} (1/256 frames)", old_last, last);
    assert!(old_last <= 48 * 256 && old_last > 40 * 256, "template's last key {old_last}");
    assert!(last <= 56 * 256 && last > 47 * 256, "last key {last}");
}
