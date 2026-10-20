//! KH1 Sora's poses on KH2 Sora's skeleton: the reverse of [`crate::retarget`], so KH2's model
//! can be drawn while KH1 animates him (its own moves, cut scenes).
//!
//! Both skeletons bind in a T-pose facing +z with the same axes (`retarget`'s notes). Each KH2
//! bone is paired with the KH1 bone that starts at the same joint and runs the same way in the
//! two binds (KH2's scaled to KH1's size by the hip heights): the nearest origin, ties broken by
//! the direction to the first child, among the KH1 bones a caller allows (the body, hair and
//! hood; not the weapon chain). A paired KH2 bone turns as its KH1 bone turned from its bind;
//! an unpaired one turns with its parent. Joints are placed from the parent along KH2's own bind
//! offsets, so the mesh keeps KH2's proportions; the hips go where KH1's are, scaled back to
//! KH2's size.
//!
//! The pairing is ours, worked out from the two bind poses, not from either game's code.

use glam::{Mat4, Quat, Vec3};
use kh2_formats::anim::Bone;

pub struct Inverse {
    /// Per KH2 bone: its KH1 bone.
    map: Vec<Option<usize>>,
    parent: Vec<Option<usize>>,
    /// KH2 bind: scale, rotation, position (model space, KH2 units).
    scale: Vec<Vec3>,
    rot: Vec<Quat>,
    pos: Vec<Vec3>,
    /// KH1 bind rotations, inverted.
    rot1_inv: Vec<Quat>,
    /// KH1 distances over KH2's (`Retarget::hip_ratio`).
    ratio: f32,
}

/// The direction from each bone to its first child (zero for a leaf or a child at the same place).
fn child_dirs(parents: &[Option<usize>], pos: &[Vec3]) -> Vec<Vec3> {
    let mut out = vec![Vec3::ZERO; pos.len()];
    let mut done = vec![false; pos.len()];
    for (c, p) in parents.iter().enumerate() {
        if let Some(p) = *p {
            if !done[p] {
                let d = pos[c] - pos[p];
                if d.length() > 1e-3 {
                    out[p] = d.normalize();
                    done[p] = true;
                }
            }
        }
    }
    out
}

fn parents(bones: &[Bone]) -> Vec<Option<usize>> {
    bones.iter().enumerate().map(|(i, b)| (b.parent >= 0 && (b.parent as usize) < i).then_some(b.parent as usize)).collect()
}

impl Inverse {
    /// `ratio`: KH1's hip height over KH2's; `usable(kh1_bone)`: the KH1 bones a KH2 bone may
    /// follow.
    pub fn new(kh1: &[Bone], kh2: &[Bone], ratio: f32, usable: impl Fn(usize) -> bool) -> Inverse {
        let b1 = kh2_anim::Pose::bind(kh1).bones;
        let b2 = kh2_anim::Pose::bind(kh2).bones;
        let p1: Vec<Vec3> = b1.iter().map(|m| m.w_axis.truncate()).collect();
        let (mut scale, mut rot, mut pos) = (Vec::new(), Vec::new(), Vec::new());
        for m in &b2 {
            let (s, r, t) = m.to_scale_rotation_translation();
            scale.push(s);
            rot.push(r.normalize());
            pos.push(t);
        }
        let rot1_inv = b1.iter().map(|m| m.to_scale_rotation_translation().1.normalize().inverse()).collect();
        let (par1, par2) = (parents(kh1), parents(kh2));
        let (d1, d2) = (child_dirs(&par1, &p1), child_dirs(&par2, &pos));
        // Within this distance (KH1 units) of a KH1 joint: a tenth of KH1's hip height.
        let near = p1.first().map_or(10.0, |p| p.y.abs() * 0.1).max(2.0);
        let map = (0..kh2.len())
            .map(|b| {
                let at = pos[b] * ratio;
                (0..kh1.len())
                    .filter(|&i| usable(i))
                    .filter_map(|i| {
                        let d = p1[i].distance(at);
                        if d > near {
                            return None;
                        }
                        let turn = if d1[i] == Vec3::ZERO || d2[b] == Vec3::ZERO { 0.5 } else { 1.0 - d1[i].dot(d2[b]) };
                        Some((d + near * turn, i))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, i)| i)
            })
            .collect();
        Inverse { map, parent: par2, scale, rot, pos, rot1_inv, ratio: if ratio.is_finite() && ratio > 0.1 { ratio } else { 1.0 } }
    }

    /// How many KH2 bones follow a KH1 bone.
    pub fn mapped(&self) -> usize {
        self.map.iter().filter(|m| m.is_some()).count()
    }

    /// The KH1 bone a KH2 bone follows.
    pub fn kh1_of(&self, kh2_bone: usize) -> Option<usize> {
        self.map.get(kh2_bone).copied().flatten()
    }

    /// KH2 model-space bone matrices (KH2 units) for KH1 model-space ones (KH1 units).
    pub fn pose(&self, kh1: &[Mat4]) -> Vec<Mat4> {
        let n = self.pos.len();
        let mut turn = vec![Quat::IDENTITY; n];
        let mut at = vec![Vec3::ZERO; n];
        let mut out = vec![Mat4::IDENTITY; n];
        let root = kh1.first().map(|m| m.w_axis.truncate() / self.ratio);
        for b in 0..n {
            let own = self.map[b].and_then(|i| {
                let m = kh1.get(i)?;
                let (_, r, _) = m.to_scale_rotation_translation();
                let q = (r.normalize() * self.rot1_inv[i]).normalize();
                q.is_finite().then_some(q)
            });
            match self.parent[b] {
                None => {
                    turn[b] = own.unwrap_or(Quat::IDENTITY);
                    at[b] = match (b, root) {
                        (0, Some(r)) => r,
                        _ => at[0] + turn[0] * (self.pos[b] - self.pos[0]),
                    };
                }
                Some(p) => {
                    turn[b] = own.unwrap_or(turn[p]);
                    at[b] = at[p] + turn[p] * (self.pos[b] - self.pos[p]);
                }
            }
            out[b] = Mat4::from_scale_rotation_translation(self.scale[b], (turn[b] * self.rot[b]).normalize(), at[b]);
        }
        out
    }
}
