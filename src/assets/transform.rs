use crate::resources::gaussians::{Gaussian3d, Gaussian4d, Gaussians};
use glam::{Quat, Vec3};

// Same basis and sign conventions as eval_sh16 in the preprocess shaders.
fn sh_basis(d: Vec3) -> [f32; 15] {
    let (x, y, z) = (d.x, d.y, d.z);
    [
        -0.4886025119029199 * y,
        0.4886025119029199 * z,
        -0.4886025119029199 * x,
        1.0925484305920792 * x * y,
        -1.0925484305920792 * y * z,
        0.31539156525252005 * (2.0 * z * z - x * x - y * y),
        -1.0925484305920792 * z * x,
        0.5462742152960396 * (x * x - y * y),
        -0.5900435899266435 * (3.0 * x * x - y * y) * y,
        2.890611442640554 * x * y * z,
        -0.4570457994644658 * (4.0 * z * z - x * x - y * y) * y,
        0.3731763325901154 * z * (2.0 * z * z - 3.0 * x * x - 3.0 * y * y),
        -0.4570457994644658 * x * (4.0 * z * z - x * x - y * y),
        1.445305721320277 * (x * x - y * y) * z,
        -0.5900435899266435 * x * (x * x - 3.0 * y * y),
    ]
}

// (first coefficient index, count) per band
const BANDS: [(usize, usize); 3] = [(0, 3), (3, 5), (8, 7)];

// Solves a * x = b for square a (n x n) and b (n x m), row-major.
fn solve(a: &mut [f64], b: &mut [f64], n: usize, m: usize) {
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&i, &j| a[i * n + col].abs().total_cmp(&a[j * n + col].abs()))
            .unwrap();
        if pivot != col {
            for k in 0..n {
                a.swap(col * n + k, pivot * n + k);
            }
            for k in 0..m {
                b.swap(col * m + k, pivot * m + k);
            }
        }

        let inv = 1.0 / a[col * n + col];
        for row in 0..n {
            if row == col {
                continue;
            }
            let f = a[row * n + col] * inv;
            for k in 0..n {
                a[row * n + k] -= f * a[col * n + k];
            }
            for k in 0..m {
                b[row * m + k] -= f * b[col * m + k];
            }
        }
    }

    for row in 0..n {
        let inv = 1.0 / a[row * n + row];
        for k in 0..m {
            b[row * m + k] *= inv;
        }
    }
}

// Per-band matrices T so that coefficients c' = T * c describe the field c(R^-1 d).
fn sh_rotation_matrices(rot: Quat) -> [Vec<f32>; 3] {
    const SAMPLES: usize = 64;

    let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    let inv_rot = rot.inverse();

    let mut a_rows = Vec::with_capacity(SAMPLES);
    let mut b_rows = Vec::with_capacity(SAMPLES);
    for i in 0..SAMPLES {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / SAMPLES as f32;
        let r = (1.0 - y * y).sqrt();
        let phi = golden * i as f32;
        let d = Vec3::new(r * phi.cos(), y, r * phi.sin());
        a_rows.push(sh_basis(d));
        b_rows.push(sh_basis(inv_rot * d));
    }

    BANDS.map(|(start, n)| {
        // Normal equations: (A^T A) T^T = A^T B
        let mut ata = vec![0.0f64; n * n];
        let mut atb = vec![0.0f64; n * n];
        for (a, b) in a_rows.iter().zip(&b_rows) {
            for i in 0..n {
                for j in 0..n {
                    ata[i * n + j] += (a[start + i] * a[start + j]) as f64;
                    atb[i * n + j] += (a[start + i] * b[start + j]) as f64;
                }
            }
        }
        solve(&mut ata, &mut atb, n, n);

        atb.iter().map(|v| *v as f32).collect()
    })
}

fn rotate_sh(sh: &mut [f32; 48], matrices: &[Vec<f32>; 3]) {
    for channel in 0..3 {
        let base = 3 + channel * 15;
        let src: [f32; 15] = sh[base..base + 15].try_into().unwrap();

        for ((start, n), t) in BANDS.iter().zip(matrices) {
            for j in 0..*n {
                let mut v = 0.0;
                for m in 0..*n {
                    v += t[j * n + m] * src[start + m];
                }
                sh[base + start + j] = v;
            }
        }
    }
}

// Not normalized: 4D omega is added to this quaternion before normalization.
fn rotate_quat_wxyz(rot: Quat, q: [f32; 4]) -> [f32; 4] {
    let new = rot * Quat::from_xyzw(q[1], q[2], q[3], q[0]);
    [new.w, new.x, new.y, new.z]
}

fn rotate_vec(rot: Quat, v: [f32; 3]) -> [f32; 3] {
    (rot * Vec3::from(v)).into()
}

fn rotate_3d(g: &mut Gaussian3d, rot: Quat, sh_matrices: &[Vec<f32>; 3]) {
    g.position = rotate_vec(rot, g.position);
    g.rotation = rotate_quat_wxyz(rot, g.rotation);
    rotate_sh(&mut g.sh, sh_matrices);
}

fn rotate_4d(g: &mut Gaussian4d, rot: Quat) {
    g.position = rotate_vec(rot, g.position);
    g.rotation = rotate_quat_wxyz(rot, g.rotation);
    g.motion_0 = rotate_vec(rot, g.motion_0);
    g.motion_1 = rotate_vec(rot, g.motion_1);
    g.motion_2 = rotate_vec(rot, g.motion_2);

    // omega is a quaternion delta, so it is premultiplied like the orientation
    let omega = Quat::from_xyzw(g.omega[1], g.omega[2], g.omega[3], g.omega[0]);
    let omega = rot * omega;
    g.omega = [omega.w, omega.x, omega.y, omega.z];
}

pub fn rotate_gaussians(gaussians: &Gaussians, rot: Quat) -> Gaussians {
    match gaussians {
        Gaussians::Gaussian3d(items) => {
            let matrices = sh_rotation_matrices(rot);
            let mut items = items.clone();
            for g in &mut items {
                rotate_3d(g, rot, &matrices);
            }
            Gaussians::Gaussian3d(items)
        }
        Gaussians::Gaussian4d(items) => {
            let mut items = items.clone();
            for g in &mut items {
                rotate_4d(g, rot);
            }
            Gaussians::Gaussian4d(items)
        }
    }
}

// Calls `f(index, position)` for every splat in file order.
pub fn for_each_position(gaussians: &Gaussians, mut f: impl FnMut(usize, [f32; 3])) {
    match gaussians {
        Gaussians::Gaussian3d(items) => {
            for (i, g) in items.iter().enumerate() {
                f(i, g.position);
            }
        }
        Gaussians::Gaussian4d(items) => {
            for (i, g) in items.iter().enumerate() {
                f(i, g.position);
            }
        }
    }
}

// Removes the splats whose flag in `keep` is false and returns them.
pub fn split_off_gaussians(gaussians: &mut Gaussians, keep: &[bool]) -> Gaussians {
    fn split<T>(items: &mut Vec<T>, keep: &[bool]) -> Vec<T> {
        let mut kept = Vec::with_capacity(items.len());
        let mut removed = Vec::new();
        for (i, item) in items.drain(..).enumerate() {
            if keep.get(i).copied().unwrap_or(true) {
                kept.push(item);
            } else {
                removed.push(item);
            }
        }
        *items = kept;
        removed
    }

    match gaussians {
        Gaussians::Gaussian3d(items) => Gaussians::Gaussian3d(split(items, keep)),
        Gaussians::Gaussian4d(items) => Gaussians::Gaussian4d(split(items, keep)),
    }
}

// Puts splats removed by `split_off_gaussians` back at their original positions.
pub fn merge_gaussians(gaussians: &mut Gaussians, removed: Gaussians, keep: &[bool]) {
    fn merge<T>(items: &mut Vec<T>, removed: Vec<T>, keep: &[bool]) {
        let mut kept = std::mem::take(items).into_iter();
        let mut removed = removed.into_iter();
        let mut merged = Vec::with_capacity(keep.len());
        for &keep_item in keep {
            let next = if keep_item {
                kept.next()
            } else {
                removed.next()
            };
            merged.extend(next);
        }
        merged.extend(kept);
        merged.extend(removed);
        *items = merged;
    }

    match (gaussians, removed) {
        (Gaussians::Gaussian3d(items), Gaussians::Gaussian3d(removed)) => {
            merge(items, removed, keep)
        }
        (Gaussians::Gaussian4d(items), Gaussians::Gaussian4d(removed)) => {
            merge(items, removed, keep)
        }
        _ => {}
    }
}

pub fn translate_gaussians(gaussians: &mut Gaussians, offset: Vec3) {
    match gaussians {
        Gaussians::Gaussian3d(items) => {
            for g in items {
                g.position = (Vec3::from(g.position) + offset).into();
            }
        }
        Gaussians::Gaussian4d(items) => {
            for g in items {
                g.position = (Vec3::from(g.position) + offset).into();
            }
        }
    }
}

// Uniformly scales the scene around `center`, including splat sizes and 4D motion.
pub fn scale_gaussians(gaussians: &mut Gaussians, center: Vec3, factor: f32) {
    let log_factor = factor.ln();
    let scale_position =
        |p: [f32; 3]| -> [f32; 3] { (center + (Vec3::from(p) - center) * factor).into() };
    let scale_vec = |v: [f32; 3]| -> [f32; 3] { (Vec3::from(v) * factor).into() };

    match gaussians {
        Gaussians::Gaussian3d(items) => {
            for g in items {
                g.position = scale_position(g.position);
                g.scale = g.scale.map(|s| s + log_factor);
            }
        }
        Gaussians::Gaussian4d(items) => {
            for g in items {
                g.position = scale_position(g.position);
                g.scale = g.scale.map(|s| s + log_factor);
                g.motion_0 = scale_vec(g.motion_0);
                g.motion_1 = scale_vec(g.motion_1);
                g.motion_2 = scale_vec(g.motion_2);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(sh: &[f32; 48], channel: usize, d: Vec3) -> f32 {
        let basis = sh_basis(d);
        (0..15).map(|k| basis[k] * sh[3 + channel * 15 + k]).sum()
    }

    #[test]
    fn rotated_sh_matches_rotated_view_direction() {
        let rot = Quat::from_euler(glam::EulerRot::XYZ, 0.4, -1.1, 0.7);
        let matrices = sh_rotation_matrices(rot);

        let mut sh = [0.0f32; 48];
        for (i, v) in sh.iter_mut().enumerate() {
            *v = ((i * 37 % 11) as f32 - 5.0) * 0.1;
        }
        let mut rotated = sh;
        rotate_sh(&mut rotated, &matrices);

        let d = Vec3::new(0.3, -0.5, 0.8).normalize();
        for channel in 0..3 {
            let expected = eval(&sh, channel, d);
            let actual = eval(&rotated, channel, rot * d);
            assert!((expected - actual).abs() < 1e-3, "{expected} vs {actual}");
        }
    }
}
