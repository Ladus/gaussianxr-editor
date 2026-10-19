use crate::resources::gaussians::{Gaussian3d, Gaussian4d, Gaussians};

fn header(count: usize, properties: &[String]) -> Vec<u8> {
    let mut out = String::from("ply\nformat binary_little_endian 1.0\n");
    out.push_str(&format!("element vertex {count}\n"));
    for name in properties {
        out.push_str(&format!("property float {name}\n"));
    }
    out.push_str("end_header\n");
    out.into_bytes()
}

fn names(prefix: &str, count: usize) -> impl Iterator<Item = String> + '_ {
    (0..count).map(move |i| format!("{prefix}{i}"))
}

fn push_f32s(out: &mut Vec<u8>, values: &[f32]) {
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn write_3d(items: &[Gaussian3d]) -> Vec<u8> {
    let properties: Vec<String> = ["x", "y", "z"]
        .into_iter()
        .map(String::from)
        .chain(names("f_dc_", 3))
        .chain(names("f_rest_", 45))
        .chain(["opacity".to_string()])
        .chain(names("scale_", 3))
        .chain(names("rot_", 4))
        .collect();

    let mut out = header(items.len(), &properties);
    out.reserve(items.len() * properties.len() * 4);
    for g in items {
        push_f32s(&mut out, &g.position);
        push_f32s(&mut out, &g.sh);
        push_f32s(&mut out, &[g.opacity]);
        push_f32s(&mut out, &g.scale);
        push_f32s(&mut out, &g.rotation);
    }
    out
}

fn write_4d(items: &[Gaussian4d]) -> Vec<u8> {
    let properties: Vec<String> = ["x", "y", "z"]
        .into_iter()
        .map(String::from)
        .chain(names("f_dc_", 3))
        .chain(["opacity".to_string()])
        .chain(names("scale_", 3))
        .chain(names("rot_", 4))
        .chain(["trbf_center".to_string(), "trbf_scale".to_string()])
        .chain(names("motion_", 9))
        .chain(names("omega_", 4))
        .collect();

    let mut out = header(items.len(), &properties);
    out.reserve(items.len() * properties.len() * 4);
    for g in items {
        push_f32s(&mut out, &g.position);
        push_f32s(&mut out, &g.base_color);
        push_f32s(&mut out, &[g.opacity]);
        push_f32s(&mut out, &g.scale);
        push_f32s(&mut out, &g.rotation);
        push_f32s(&mut out, &[g.trbf_center, g.trbf_scale]);
        push_f32s(&mut out, &g.motion_0);
        push_f32s(&mut out, &g.motion_1);
        push_f32s(&mut out, &g.motion_2);
        push_f32s(&mut out, &g.omega);
    }
    out
}

pub fn write_ply(gaussians: &Gaussians) -> Vec<u8> {
    match gaussians {
        Gaussians::Gaussian3d(items) => write_3d(items),
        Gaussians::Gaussian4d(items) => write_4d(items),
    }
}
