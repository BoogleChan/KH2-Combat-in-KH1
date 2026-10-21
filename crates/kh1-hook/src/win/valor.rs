//! Valor Form's second Keyblade (the user: "add the slot in menu for valor form, and
//! automatically put a keyblade that isnt equipped into the slot. If no keyblade is eligible to
//! be in that slot, dont let players go into valor form").
//!
//! - The slot: a Keyblade id kept in the save at `equip::VALOR_OFF` (`docs/kh1-equipment.md`).
//! - Eligible: a Keyblade Sora owns with a copy not equipped (`equip::valor_candidates`). The
//!   slot's Keyblade is kept while it stays eligible; otherwise the eligible one with the most
//!   Strength is put in it (`equip::valor_keyblade`); with none, the slot is emptied and Valor
//!   can't be chosen (`form::usable` / `form::choose`).
//! - In the form, its swings' second-hand hitboxes are KH2's (the rig's second weapon); its model
//!   in his left hand: `started` / `ended`.
//! - Final Form has a slot of its own (the user: "add a section in the equipment menu for the
//!   second keyblade in final form"): `equip::FINAL_OFF`, the same Keyblades to choose from, but
//!   the Kingdom Key when it holds none of them (`final_keyblade`), so Final is never refused.
//! - Each slot holds its copy as equipping does (`equip::form_held`): one copy can't be in both
//!   slots, nor in a slot and Sora's hand. Valor's slot keeps its copy first; Final's Kingdom Key
//!   fallback holds none (its slot is emptied).

use super::log;
use std::sync::atomic::{AtomicU32, Ordering};

/// The Keyblade in Valor's slot, as last decided (0 none), for the log.
static CHOSEN: AtomicU32 = AtomicU32::new(0);

/// The Keyblade Valor uses now (its item id), the slot filled or fixed as it goes; `None` when
/// Sora owns no Keyblade with a copy free.
pub unsafe fn keyblade() -> Option<u8> {
    let base = super::sys::GetModuleHandleA(std::ptr::null()) as usize;
    let save = super::menu::save_block(base)?;
    let pick = super::equip::valor_keyblade(base);
    let slot = (save + super::equip::VALOR_OFF) as *mut u8;
    let stored = std::ptr::read_unaligned(slot);
    let want = pick.unwrap_or(0);
    if stored != want {
        std::ptr::write_unaligned(slot, want);
    }
    // While he is Valor, the slot's Keyblade in his hand, changed or not (the Equipment row, the
    // user: "when switching the second keyblade out while in valor form it doesnt swap
    // properly"; `leftblade::show` does nothing more when it's the one loaded, and asks again
    // while another load is still under way).
    if super::form::in_valor() {
        super::leftblade::show(pick);
    }
    if CHOSEN.swap(want as u32, Ordering::Relaxed) != want as u32 {
        let name = pick.and_then(|k| super::equip::item_name(base, k)).unwrap_or_else(|| "none".into());
        log(&format!(
            "Valor's Keyblade: {name} ({want:#x}; Sora's {:?}, Keyblades owned {:02x?})",
            super::equip::sora_weapon(base),
            super::equip::owned_keyblades(base)
        ));
    }
    pick
}

/// The Keyblade in Final's slot, as last decided (0 not yet), for the log.
static FINAL_CHOSEN: AtomicU32 = AtomicU32::new(0);

/// The Keyblade Final uses now (its item id): the slot's if Sora may give it (a copy no member
/// and not Valor's slot holds), else the Kingdom Key (`equip::final_keyblade`). The slot is
/// emptied for the Kingdom Key fallback, so the fallback holds no copy (`equip::final_held`);
/// a Kingdom Key picked in the menu stays and holds one.
pub unsafe fn final_keyblade() -> u8 {
    let base = super::sys::GetModuleHandleA(std::ptr::null()) as usize;
    // (While a form row's pick may have the hovered Keyblade in Sora's weapon byte, the copies
    // can't be counted: the slot as stored, nothing written.)
    let picking = super::equipmenu::pick_own().is_some();
    let k = if picking { super::equip::final_stored(base) } else { super::equip::final_keyblade(base) };
    if let (Some(save), false) = (super::menu::save_block(base), picking) {
        let slot = (save + super::equip::FINAL_OFF) as *mut u8;
        let stored = std::ptr::read_unaligned(slot);
        if stored != 0 && !super::equip::final_may_hold(base, stored) {
            // (An old save with both slots on Sora's one free copy: Valor keeps it.)
            let why = if super::equip::valor_held(base) == stored {
                "its copy is Valor's (Valor's slot keeps it)"
            } else {
                "no copy of it is free"
            };
            let name = super::equip::item_name(base, stored).unwrap_or_else(|| format!("{stored:#x}"));
            log(&format!("Final's Keyblade: {name} ({stored:#x}) {why}: Final falls back to the Kingdom Key, slot emptied"));
            std::ptr::write_unaligned(slot, 0);
        }
    }
    // (In Final, a change made in the menu reaches his left hand, as Valor's.)
    if super::form::in_final() {
        super::leftblade::show(Some(k));
    }
    if FINAL_CHOSEN.swap(k as u32, Ordering::Relaxed) != k as u32 {
        let name = super::equip::item_name(base, k).unwrap_or_else(|| format!("{k:#x}"));
        log(&format!("Final's Keyblade: {name} ({k:#x}; Sora's {:?})", super::equip::sora_weapon(base)));
    }
    k
}

/// Sora became Valor (or Final): his second Keyblade comes out.
pub unsafe fn started() {
    if super::form::in_final() {
        let k = final_keyblade();
        log(&format!("Final Form: second Keyblade {k:#x}"));
        super::leftblade::show(Some(k));
        return;
    }
    let k = keyblade();
    log(&format!("Valor Form: second Keyblade {:?}", k.map(|k| format!("{k:#x}"))));
    super::leftblade::show(k);
}

/// Back from Valor.
pub unsafe fn ended() {
    super::leftblade::hide();
    // Sora's own Strength and max MP again (`valorstats`).
    super::valorstats::step(super::takeover::sora_actor());
}
