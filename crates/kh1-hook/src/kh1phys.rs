//! KH1's own physics state for an actor, as plain numbers (`docs/kh1-physics.md`), and the one
//! place KH2's controller's motion is turned into it and back.
//!
//! The user (testing, after s284): "currently, kh2 handles all of kh2s physics unless specified
//! this is causing a bug where if sora does a kh1 air move from a kh2 air move then he gets no
//! momentum during his kh1 move. Lets rewrite how kh2 can use physics so that kh2s physics gets
//! written into kh1s internal physics system instead".
//!
//! KH1 keeps an airborne actor's motion in two places, both read by its air state's step
//! (`0x1402AD510`, state 2):
//! - its sideways velocity `+0x3C0` (x, y, z, w = its length), units per 60 Hz frame, moved by
//!   that times the frame scale each game frame;
//! - its arc (`Arc`): no vertical velocity at all, but a curve of height against air time. The
//!   rise is KH2's own curve (`y = h sin(pi/2 sin(pi/2 t/T))`), the fall `g/2 (t - T)^2` with
//!   KH1's gravity, capped at 20 a frame.
//!
//! So KH2's motion goes into KH1 as a sideways velocity and an arc that goes on from where he is
//! at the speed he has: a KH2 rise exactly (the same curve), a KH2 fall at its speed now (KH1's
//! gravity from there). Nothing here touches the game: `win::shared::physics` reads and writes the
//! actor.

use character_core::Arc as Kh2Arc;

/// KH1's gravity, units per 60 Hz frame squared (`0x1403EAB54`).
pub const GRAVITY: f32 = 0.484_693_88;
/// KH1's fastest fall, units per 60 Hz frame (`0x1404308B8`; its step caps a frame's drop at
/// this times the frame scale).
pub const FALL_MAX: f32 = 20.0;
/// The fastest sideways speed KH2's controller hands KH1 (per 60 Hz frame; what the hand-overs
/// kept it to before). KH1's own walk-off carry is capped at 8 (`0x14043089C`).
pub const CARRY_MAX: f32 = 30.0;

const A: f32 = std::f32::consts::FRAC_PI_2;

/// The rise's shape, 0..1 over 0..1 of the rise (KH1's and KH2's alike).
fn sinsin(s: f32) -> f32 {
    (A * (A * s).sin()).sin()
}

/// Its slope (per unit of `s`).
fn sinsin_slope(s: f32) -> f32 {
    (A * (A * s).sin()).cos() * A * (A * s).cos() * A
}

fn len(v: [f32; 2]) -> f32 {
    v[0].hypot(v[1])
}

/// A sideways speed kept to `CARRY_MAX` (none if it isn't a number).
pub fn carried(v: [f32; 2]) -> [f32; 2] {
    if !v.iter().all(|x| x.is_finite()) {
        return [0.0; 2];
    }
    let n = len(v);
    let k = if n > CARRY_MAX { CARRY_MAX / n } else { 1.0 };
    [v[0] * k, v[1] * k]
}

/// KH1's air arc on an actor: `+0x3B0` its time `t` (60 Hz frames), `+0x3B4` the start height
/// `y0`, `+0x3B8` the time to its apex `up_t`, `+0x3BC` the apex's offset `h` (negative: up).
/// KH1's world is y-down, so every height here is KH1's (`+0x14`), down positive.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Arc {
    pub t: f32,
    pub y0: f32,
    pub up_t: f32,
    pub h: f32,
}

impl Arc {
    /// Falling from rest at `y` (what KH1's walk off an edge sets, `0x1402AD760 (actor, 0)`, and
    /// what the hand-overs used to set).
    pub fn rest(y: f32) -> Arc {
        Arc { t: 0.0, y0: y, up_t: 0.0, h: 0.0 }
    }

    /// A rise of `height` from `y` (`0x1402AD7D0`: KH1's jumps, its air attacks' hops, its
    /// launches): `T = sqrt(2 height / g)`.
    pub fn hop(y: f32, height: f32) -> Arc {
        let height = height.abs();
        Arc { t: 0.0, y0: y, up_t: (2.0 * height / GRAVITY).sqrt(), h: -height }
    }

    pub fn rising(&self) -> bool {
        self.t < self.up_t && self.up_t > 0.0
    }

    /// Its height at air time `t` (KH1 y-down).
    pub fn y_at(&self, t: f32) -> f32 {
        if t < self.up_t && self.up_t > 0.0 {
            self.y0 + self.h * sinsin(t / self.up_t)
        } else {
            let d = t - self.up_t;
            self.y0 + self.h + 0.5 * GRAVITY * d * d
        }
    }

    /// KH1's own step of it (state 2, `0x1402AD510`): the air time on by `dt` (the frame scale),
    /// and the drop this frame from where he is (`y`), a fall's capped at `FALL_MAX` a frame.
    pub fn step(&mut self, y: f32, dt: f32) -> f32 {
        self.t += dt;
        let dy = self.y_at(self.t) - y;
        if self.rising() {
            dy
        } else {
            dy.min(FALL_MAX * dt)
        }
    }

    /// His up speed on it now, per 60 Hz frame (up positive, as KH2 has it): the curve's slope.
    pub fn up_speed(&self) -> f32 {
        if self.rising() {
            -self.h * sinsin_slope(self.t / self.up_t) / self.up_t
        } else {
            -(GRAVITY * (self.t - self.up_t)).clamp(0.0, FALL_MAX)
        }
    }

    /// How much higher its apex is than where it has him (0 past it).
    pub fn rise_left(&self) -> f32 {
        if self.rising() {
            (self.y_at(self.t) - (self.y0 + self.h)).max(0.0)
        } else {
            0.0
        }
    }

    /// Falling at `down` per 60 Hz frame from `y`, `t` frames into the air: KH1's fall, its apex
    /// put `down / g` frames back (so KH1's gates that count from the apex, its grabs' 6 frames,
    /// see a fall that fast as that far past one).
    pub fn falling(y: f32, down: f32, t: f32) -> Arc {
        let tau = down.clamp(0.0, FALL_MAX) / GRAVITY;
        Arc { t, y0: y - 0.5 * GRAVITY * tau * tau, up_t: t - tau, h: 0.0 }
    }

    /// Going on at `up` per 60 Hz frame from `y` with no curve to follow (KH2 moved him outside a
    /// jump's arc: a blow, a hop's root motion): falling at that speed, or a KH1 rise that starts
    /// at that speed and tops out as high as a throw at it would under KH1's gravity.
    pub fn moving(y: f32, up: f32) -> Arc {
        if !(up > 0.0) || !up.is_finite() {
            let down = if up.is_finite() { -up } else { 0.0 };
            return Arc::falling(y, down, down.clamp(0.0, FALL_MAX) / GRAVITY);
        }
        let height = up * up / (2.0 * GRAVITY);
        // (The rise's slope at its start is `height * (pi/2)^2 / T`.)
        Arc { t: 0.0, y0: y, up_t: height * A * A / up, h: -height }
    }

    /// KH2's arc going on from `y` in KH1's fields: KH2's rise is KH1's curve, so the rest of it
    /// is exact; past KH2's apex, KH1's fall at KH2's fall speed now (`gravity`, `fall_max`:
    /// KH2's numbers), KH1's gravity from there.
    pub fn from_kh2(a: &Kh2Arc, y: f32, gravity: f32, fall_max: f32) -> Arc {
        if a.t < a.uptime && a.uptime > 0.0 && a.height > 0.0 {
            Arc { t: a.t, y0: y + a.height * sinsin(a.t / a.uptime), up_t: a.uptime, h: -a.height }
        } else {
            let down = (gravity * (a.t - a.uptime)).clamp(0.0, fall_max.max(0.0));
            Arc::falling(y, down, a.t)
        }
    }

    /// The same motion as a KH2 arc (KH2's gravity from here, `gravity`): its rise exactly, its
    /// fall at its speed now.
    pub fn to_kh2(&self, gravity: f32) -> Kh2Arc {
        if self.rising() && self.h < 0.0 {
            return Kh2Arc { height: -self.h, t: self.t, uptime: self.up_t, holdable: false };
        }
        let mut a = Kh2Arc::fall();
        if gravity > 0.0 {
            a.t = (-self.up_speed()).max(0.0) / gravity;
        }
        a
    }
}

/// An actor's motion as KH1 keeps it: sideways velocity (`+0x3C0`, per 60 Hz frame, KH1's x and
/// z, which are KH2's) and its arc.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    pub h: [f32; 2],
    pub arc: Arc,
}

impl Motion {
    /// Its velocity per 60 Hz frame: sideways, up (KH2's way).
    pub fn velocity(&self) -> ([f32; 2], f32) {
        (self.h, self.arc.up_speed())
    }
}

/// KH2's controller's motion after one of its steps, as KH1's: `air` its arc and whether that
/// step was a physics tick's first frame (`Character::air_arc`), `disp` the step's displacement
/// (KH2's world, y up: a 60 Hz step's travel, root motion included, so its velocity), `y` where
/// he is now (KH1's height), `gravity` / `fall_max` KH2's.
///
/// The arc is followed when it is what moved him this step (his drop is the arc's); otherwise
/// (root motion up or down, a blow, a glide's sink) his speed now goes on (`Arc::moving`).
pub fn from_kh2_step(air: Option<(Kh2Arc, bool)>, disp: [f32; 3], y: f32, gravity: f32, fall_max: f32) -> Motion {
    let h = carried([disp[0], disp[2]]);
    let up = if disp[1].is_finite() { disp[1] } else { 0.0 };
    if let Some((a, first)) = air {
        // (The arc moves a 2-frame tick at a time, half of it each frame.)
        let t1 = a.t;
        let t0 = (t1 - 2.0).max(0.0);
        let tick = if t1 > t0 { ((a.y(t1, gravity) - a.y(t0, gravity)).max(-fall_max * (t1 - t0))) / (t1 - t0) } else { 0.0 };
        if (tick - up).abs() < 0.05 {
            // After a tick's first frame he is one frame short of the arc's time.
            let at = Kh2Arc { t: if first { (a.t - 1.0).max(0.0) } else { a.t }, ..a };
            return Motion { h, arc: Arc::from_kh2(&at, y, gravity, fall_max) };
        }
    }
    Motion { h, arc: Arc::moving(y, up) }
}

/// One of KH1's own air moves started out of a KH2 move (KH1's command step ran it from KH1's air
/// state carrying `before`, KH2's motion; `kh1` is what the move's start left: an air attack sets
/// its own sideways lunge, 8 along its target or the stick on the first swing, 12 or 4 after
/// (`0x1402A4760`), and a hop of 40 or 20 (the attack rows' `+8`), replacing what was there).
/// What it keeps: the faster sideways speed, and KH2's rise when it goes higher than the move's
/// hop. So a KH1 air move out of a KH2 one never has less momentum than it took over, nor less
/// than KH1 gives it.
pub fn keep_momentum(before: &Motion, kh1: &Motion) -> Motion {
    let h = if len(before.h) > len(kh1.h) + 1e-3 { before.h } else { kh1.h };
    let arc = if before.arc.rise_left() > kh1.arc.rise_left() + 0.5 { before.arc } else { kh1.arc };
    Motion { h, arc }
}

#[cfg(test)]
mod tests {
    use super::*;
    use character_core::{motion, Abilities, AttackParams, Button, Character, Clip, Env, Event, EventKind, GrowthLevel, Input, MoveParams, Moveset, StateName};

    const KH2_G: f32 = 20.0 / 49.0;
    const KH2_FALL: f32 = 16.0;

    #[test]
    fn kh1_hop_tops_out_at_its_height_then_falls_at_kh1_gravity() {
        let mut a = Arc::hop(0.0, 190.0);
        let mut y = 0.0;
        while a.rising() {
            y += a.step(y, 1.0);
        }
        assert!((y + 190.0).abs() < 1.0, "apex {y}");
        let before = a.up_speed();
        y += a.step(y, 1.0);
        assert!(a.up_speed() < before && a.up_speed() < 0.0);
        // (The fall caps at 20 a frame.)
        for _ in 0..200 {
            let dy = a.step(y, 1.0);
            assert!(dy <= FALL_MAX + 1e-3);
            y += dy;
        }
    }

    /// KH2's jump handed to KH1 mid-rise: KH1's own steps go on along KH2's curve to its apex, at
    /// 60 fps (scale 1) and 30 fps (scale 2).
    #[test]
    fn a_kh2_rise_goes_on_exactly_in_kh1s_fields() {
        for dt in [1.0f32, 2.0] {
            let mut k2 = Kh2Arc::new(185.0, KH2_G);
            let mut up = 0.0;
            for _ in 0..8 {
                up += k2.advance(dt, KH2_G, KH2_FALL);
            }
            let mut y = -up;
            let mut k1 = Arc::from_kh2(&k2, y, KH2_G, KH2_FALL);
            assert!((k1.up_speed() - (k2.y(k2.t + 0.01, KH2_G) - k2.y(k2.t, KH2_G)) / 0.01).abs() < 0.05);
            while k2.rising() {
                up += k2.advance(dt, KH2_G, KH2_FALL);
                y += k1.step(y, dt);
                // (The step over the apex falls the last bit at each game's gravity.)
                let tol = if k2.rising() { 1e-2 } else { 0.2 };
                assert!((y + up).abs() < tol, "dt {dt}: KH1 {y} KH2 {up}");
            }
        }
    }

    #[test]
    fn a_kh2_fall_goes_on_at_its_speed() {
        let mut k2 = Kh2Arc::fall();
        for _ in 0..10 {
            k2.advance(1.0, KH2_G, KH2_FALL);
        }
        let speed = KH2_G * k2.t;
        let mut k1 = Arc::from_kh2(&k2, 500.0, KH2_G, KH2_FALL);
        assert!((k1.up_speed() + speed).abs() < 1e-3);
        let dy = k1.step(500.0, 1.0);
        assert!((dy - speed - GRAVITY / 2.0).abs() < 1e-3, "{dy} vs {speed}");
    }

    #[test]
    fn moving_starts_at_the_speed_and_tops_out_as_a_throw() {
        let a = Arc::moving(0.0, 6.0);
        assert!((a.up_speed() - 6.0).abs() < 1e-3);
        assert!((a.rise_left() - 36.0 / (2.0 * GRAVITY)).abs() < 1e-2);
        let f = Arc::moving(0.0, -5.0);
        assert!((f.up_speed() + 5.0).abs() < 1e-3 && !f.rising());
        assert_eq!(Arc::moving(3.0, 0.0).up_speed(), 0.0);
    }

    #[test]
    fn kh1s_rise_back_to_kh2_is_the_same_curve() {
        let mut k1 = Arc::hop(0.0, 120.0);
        let mut y = 0.0;
        for _ in 0..5 {
            y += k1.step(y, 1.0);
        }
        let mut k2 = k1.to_kh2(KH2_G);
        let mut up = -y;
        while k2.rising() {
            up += k2.advance(1.0, KH2_G, KH2_FALL);
            y += k1.step(y, 1.0);
            let tol = if k2.rising() { 1e-2 } else { 0.2 };
            assert!((y + up).abs() < tol, "KH1 {y} KH2 {up}");
        }
        // Falling: KH2 picks the fall up at KH1's speed.
        let mut k1 = Arc::falling(0.0, 7.0, 30.0);
        k1.step(0.0, 1.0);
        let k2 = k1.to_kh2(KH2_G);
        assert!((KH2_G * k2.t - (-k1.up_speed())).abs() < 1e-3);
    }

    fn growth() -> Vec<GrowthLevel> {
        (1..=4)
            .map(|l| GrowthLevel {
                high_jump_height: 200.0 + l as f32 * 50.0,
                air_dodge_height: 100.0,
                air_dodge_speed: 10.0,
                quick_run_time: 10.0,
                quick_run_speed: 30.0,
                quick_run_brake: 0.9,
                quick_run_stop_brake: 0.5,
                quick_run_star_time: 5.0,
                glide_speed: 16.0,
                glide_fall_ratio: 0.015,
                glide_fall_height: 0.0,
                glide_fall_max: 3.0,
                glide_acceleration: 0.9,
                glide_start_height: 60.0,
                glide_end_height: 10.0,
                glide_turn_speed: 0.05,
                dodge_roll_star_time: 10.0,
            })
            .collect()
    }

    fn sora() -> Character {
        let mut ms = Moveset::default();
        let mut jump = Clip::plain(motion::JUMP, 10);
        jump.events.push(Event { frame: 4, kind: EventKind::Jump });
        ms.clips.insert(motion::JUMP, jump);
        let mut ch = Character::new(ms, MoveParams::default(), growth(), AttackParams::default());
        ch.abilities = Abilities::max_growth();
        ch.abilities.glide = 0;
        ch
    }

    /// The user's bug, on KH2's controller (`character-core`) and KH1's air state as modelled here:
    /// an Aerial Dodge (KH2) going up and forward, then KH1's air attack. Before, KH1's air state was
    /// set falling from rest (its arc zeroed) and its sideways speed whatever the attack set; now it
    /// goes on with KH2's rise and speed, the hand-over at 60 fps and at 30 fps alike.
    #[test]
    fn a_kh1_air_attack_out_of_an_aerial_dodge_keeps_its_momentum() {
        for substeps in [1usize, 2] {
            let mut ch = sora();
            let mut pos = [0.0f32; 3];
            let mut prev_jump = false;
            let mut step = |ch: &mut Character, pos: &mut [f32; 3], jump: bool, stick: [f32; 2]| {
                let input = Input { stick, jump: Button::from_held(prev_jump, jump), attack: Button::default(), action: Button::default(), lock: Button::default() };
                prev_jump = jump;
                let env = Env { pos: *pos, grounded: pos[1] <= 0.0, height_above_ground: pos[1].max(0.0), targets: &[], ledges: &[] };
                let out = ch.step(&input, &env);
                for i in 0..3 {
                    pos[i] += out.displacement[i];
                }
                pos[1] = pos[1].max(0.0);
                out
            };
            step(&mut ch, &mut pos, true, [0.0, 0.0]);
            for _ in 0..12 {
                step(&mut ch, &mut pos, false, [0.0, 0.0]);
            }
            let o = step(&mut ch, &mut pos, true, [0.0, 1.0]);
            assert_eq!(o.state, StateName::AerialDodge);
            // A few frames into the dodge (an odd count at 30 fps's two substeps a game frame, too).
            let mut out = o;
            for _ in 0..(3 + substeps) {
                out = step(&mut ch, &mut pos, true, [0.0, 1.0]);
            }
            let kh2_up = out.displacement[1];
            assert!(kh2_up > 1.0, "rising: {kh2_up}");
            // KH2's motion written into KH1's fields (KH1's height is minus KH2's).
            let y = -pos[1];
            let before = from_kh2_step(ch.air_arc(), out.displacement, y, ch.params.gravity, ch.params.fall_max);
            let (vh, vu) = before.velocity();
            assert!((vu - kh2_up).abs() < 0.6, "KH1's up speed {vu} vs KH2's {kh2_up}");
            assert!((len(vh) - len([out.displacement[0], out.displacement[2]])).abs() < 1e-3);
            // KH1's air attack: its first swing's lunge (8 along the stick) and hop (40).
            let lunge = Motion { h: [0.0, 8.0], arc: Arc::hop(y, 40.0) };
            let kept = keep_momentum(&before, &lunge);
            // (Before: falling from rest, with whatever speed the swing set.)
            let old = Motion { h: lunge.h, arc: Arc::rest(y) };
            assert_eq!(old.arc.up_speed(), 0.0);
            assert!(kept.arc.rise_left() + 0.5 >= before.arc.rise_left().max(lunge.arc.rise_left()));
            assert!(len(kept.h) >= len(before.h).max(8.0) - 1e-3);
            // KH1's own state-2 steps from there rise on (its frame scale `substeps`).
            let mut arc = kept.arc;
            let mut yy = y;
            let dy = arc.step(yy, substeps as f32);
            yy += dy;
            assert!(dy < -1.0, "still going up: {dy}");
            assert!(yy < y);
        }
    }
}
