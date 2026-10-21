use super::format::FileFormat;
use super::{ply_writer, sog_writer, transform};
use crate::resources::gaussians::Gaussians;
use crate::adjust::{ColorAdjustments, bake_adjustments};
use glam::{Quat, Vec3};

// The exported scene is rotated, then shifted so `origin` becomes (0, 0, 0).
pub fn export_gaussians(
    format: FileFormat,
    gaussians: &Gaussians,
    rotation: Quat,
    origin: Vec3,
    adjustments: &ColorAdjustments,
) -> anyhow::Result<Vec<u8>> {
    let mut rotated = transform::rotate_gaussians(gaussians, rotation);
    transform::translate_gaussians(&mut rotated, -origin);
    bake_adjustments(&mut rotated, adjustments);

    match format {
        FileFormat::Ply => Ok(ply_writer::write_ply(&rotated)),
        FileFormat::Sog => sog_writer::write_sog(&rotated),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{ply_loader, sog_loader};
    use crate::resources::gaussians::Gaussian3d;

    fn sample(count: usize) -> Gaussians {
        let items = (0..count)
            .map(|i| {
                let f = i as f32;
                let mut sh = [0.0f32; 48];
                for (k, v) in sh.iter_mut().enumerate() {
                    *v = ((i * 7 + k * 13) % 17) as f32 * 0.05 - 0.4;
                }
                Gaussian3d {
                    position: [f.sin() * 5.0, f.cos() * 3.0, f * 0.1 - 2.0],
                    opacity: (f * 0.3).sin() * 3.0,
                    scale: [-3.0 + f.sin(), -2.5 + f.cos(), -4.0],
                    _pad0: 0,
                    rotation: [1.0 + f.sin(), f.cos(), 0.3, -0.2],
                    sh,
                }
            })
            .collect();
        Gaussians::Gaussian3d(items)
    }

    fn items(g: &Gaussians) -> &[Gaussian3d] {
        match g {
            Gaussians::Gaussian3d(v) => v,
            _ => panic!("expected 3D"),
        }
    }

    #[test]
    fn ply_round_trip() {
        let original = sample(50);
        let bytes =
            export_gaussians(
                FileFormat::Ply,
                &original,
                Quat::IDENTITY,
                Vec3::ZERO,
                &ColorAdjustments::default(),
            ).unwrap();
        let loaded = ply_loader::parse_gaussian_ply_bytes(&bytes).unwrap();

        for (a, b) in items(&original).iter().zip(items(&loaded)) {
            assert_eq!(a.position, b.position);
            for k in 0..48 {
                assert!((a.sh[k] - b.sh[k]).abs() < 1e-5);
            }
            assert_eq!(a.rotation, b.rotation);
        }
    }

    #[test]
    fn sog_round_trip_is_close() {
        let original = sample(300);
        let bytes =
            export_gaussians(
                FileFormat::Sog,
                &original,
                Quat::IDENTITY,
                Vec3::ZERO,
                &ColorAdjustments::default(),
            ).unwrap();
        let loaded = sog_loader::load_sog_from_bytes(&bytes).unwrap();

        assert_eq!(items(&loaded).len(), 300);
        for (a, b) in items(&original).iter().zip(items(&loaded)) {
            for k in 0..3 {
                assert!(
                    (a.position[k] - b.position[k]).abs() < 0.01,
                    "{:?} {:?}",
                    a.position,
                    b.position
                );
                assert!((a.scale[k] - b.scale[k]).abs() < 0.1);
            }
            for k in 0..48 {
                assert!(
                    (a.sh[k] - b.sh[k]).abs() < 0.05,
                    "sh {k}: {} {}",
                    a.sh[k],
                    b.sh[k]
                );
            }
            let na = Quat::from_xyzw(a.rotation[1], a.rotation[2], a.rotation[3], a.rotation[0])
                .normalize();
            let nb = Quat::from_xyzw(b.rotation[1], b.rotation[2], b.rotation[3], b.rotation[0])
                .normalize();
            assert!(na.dot(nb).abs() > 0.99);
        }
    }
}
