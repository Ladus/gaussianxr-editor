use crate::resources::gaussians::Gaussians;
use glam::Vec3;

const MIN_OPACITY: f32 = 0.3;
const MIN_DEPTH: f32 = 0.001;
// Ray hit tolerance as a fraction of the distance, so far splats stay pickable.
const MIN_ANGLE: f32 = 0.003;

// Front-most reasonably opaque splat center near the ray; `dir` must be normalized.
pub fn pick_along_ray(gaussians: &Gaussians, origin: Vec3, dir: Vec3) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;

    let mut test = |position: [f32; 3], opacity: f32, scale: [f32; 3]| {
        if 1.0 / (1.0 + (-opacity).exp()) < MIN_OPACITY {
            return;
        }

        let p = Vec3::from(position);
        let v = p - origin;
        let t = v.dot(dir);
        if t <= MIN_DEPTH || best.is_some_and(|(best_t, _)| t >= best_t) {
            return;
        }

        let radius = scale
            .iter()
            .map(|s| s.exp())
            .fold(0.0, f32::max)
            .max(t * MIN_ANGLE);
        if (v - dir * t).length_squared() <= radius * radius {
            best = Some((t, p));
        }
    };

    match gaussians {
        Gaussians::Gaussian3d(items) => {
            for g in items {
                test(g.position, g.opacity, g.scale);
            }
        }
        Gaussians::Gaussian4d(items) => {
            for g in items {
                test(g.position, g.opacity, g.scale);
            }
        }
    }

    best.map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::transform::scale_gaussians;
    use crate::resources::gaussians::Gaussian3d;

    fn splat(position: [f32; 3]) -> Gaussian3d {
        Gaussian3d {
            position,
            opacity: 3.0,
            scale: [-3.0; 3],
            _pad0: 0,
            rotation: [1.0, 0.0, 0.0, 0.0],
            sh: [0.0; 48],
        }
    }

    #[test]
    fn picks_front_most_splat_on_the_ray() {
        let gaussians = Gaussians::Gaussian3d(vec![
            splat([0.0, 0.0, 8.0]),
            splat([0.0, 0.0, 5.0]),
            splat([3.0, 0.0, 2.0]),
        ]);

        let hit = pick_along_ray(&gaussians, Vec3::ZERO, Vec3::Z);
        assert_eq!(hit, Some(Vec3::new(0.0, 0.0, 5.0)));
        assert_eq!(pick_along_ray(&gaussians, Vec3::ZERO, Vec3::NEG_Z), None);
    }

    #[test]
    fn split_off_then_merge_restores_the_original_order() {
        let original: Vec<Gaussian3d> = (0..6).map(|i| splat([i as f32, 0.0, 0.0])).collect();
        let mut gaussians = Gaussians::Gaussian3d(original.clone());
        let keep = [true, false, true, false, false, true];

        let removed = crate::assets::transform::split_off_gaussians(&mut gaussians, &keep);
        assert_eq!(gaussians.len(), 3);
        assert_eq!(removed.len(), 3);

        crate::assets::transform::merge_gaussians(&mut gaussians, removed, &keep);
        let Gaussians::Gaussian3d(items) = &gaussians else {
            unreachable!()
        };
        let xs: Vec<f32> = items.iter().map(|g| g.position[0]).collect();
        assert_eq!(xs, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn scaling_changes_distances_and_splat_sizes() {
        let mut gaussians =
            Gaussians::Gaussian3d(vec![splat([0.0, 0.0, 1.0]), splat([0.0, 0.0, 3.0])]);
        scale_gaussians(&mut gaussians, Vec3::new(0.0, 0.0, 2.0), 0.5);

        let Gaussians::Gaussian3d(items) = &gaussians else {
            unreachable!()
        };
        assert!((items[0].position[2] - 1.5).abs() < 1e-6);
        assert!((items[1].position[2] - 2.5).abs() < 1e-6);
        assert!((items[0].scale[0] - (-3.0 + 0.5f32.ln())).abs() < 1e-6);
    }
}
