use crate::resources::gaussians::{Gaussian3d, Gaussians};
use anyhow::{bail, ensure};
use image::{ExtendedColorType, codecs::webp::WebPEncoder};
use serde_json::json;
use std::io::{Cursor, Write};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const CODEBOOK_SIZE: usize = 256;
const MAX_SHN_CENTROIDS: usize = 65536;
const SHN_LABEL_ROW: usize = 64;
const CODEBOOK_TRAIN_SAMPLES: usize = 1 << 20;

fn log_transform(x: f32) -> f32 {
    x.signum() * x.abs().ln_1p()
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

// Layout used by SOG: rows of pixels in gaussian order, padded to a multiple of 4.
fn image_size(count: usize) -> (u32, u32) {
    let width = (((count as f64).sqrt().ceil() as usize).max(1)).next_multiple_of(4);
    let height = count.div_ceil(width).max(1).next_multiple_of(4);
    (width as u32, height as u32)
}

struct Pixels {
    width: u32,
    height: u32,
    channels: usize,
    data: Vec<u8>,
}

impl Pixels {
    fn new(width: u32, height: u32, channels: usize) -> Self {
        Self {
            width,
            height,
            channels,
            data: vec![0; width as usize * height as usize * channels],
        }
    }

    fn set(&mut self, index: usize, value: &[u8]) {
        let base = index * self.channels;
        self.data[base..base + self.channels].copy_from_slice(value);
    }

    fn encode_webp(&self) -> anyhow::Result<Vec<u8>> {
        let color = match self.channels {
            3 => ExtendedColorType::Rgb8,
            4 => ExtendedColorType::Rgba8,
            n => bail!("unsupported channel count: {n}"),
        };

        let mut out = Vec::new();
        WebPEncoder::new_lossless(&mut out).encode(&self.data, self.width, self.height, color)?;
        Ok(out)
    }
}

// 256-entry 1D codebook via Lloyd iterations seeded with quantiles.
fn build_codebook(values: &[f32]) -> Vec<f32> {
    let step = (values.len() / CODEBOOK_TRAIN_SAMPLES).max(1);
    let mut samples: Vec<f32> = values.iter().step_by(step).copied().collect();
    samples.sort_by(|a, b| a.total_cmp(b));

    if samples.is_empty() {
        return vec![0.0; CODEBOOK_SIZE];
    }

    let mut codebook: Vec<f32> = (0..CODEBOOK_SIZE)
        .map(|i| samples[(i * (samples.len() - 1)) / (CODEBOOK_SIZE - 1)])
        .collect();

    for _ in 0..8 {
        let mut sums = vec![0.0f64; CODEBOOK_SIZE];
        let mut counts = vec![0usize; CODEBOOK_SIZE];
        for &v in &samples {
            let i = nearest(&codebook, v) as usize;
            sums[i] += v as f64;
            counts[i] += 1;
        }
        for i in 0..CODEBOOK_SIZE {
            if counts[i] > 0 {
                codebook[i] = (sums[i] / counts[i] as f64) as f32;
            }
        }
        codebook.sort_by(|a, b| a.total_cmp(b));
    }

    codebook
}

// codebook must be sorted ascending
fn nearest(codebook: &[f32], v: f32) -> u8 {
    let upper = codebook.partition_point(|&c| c < v);
    let best = if upper == 0 {
        0
    } else if upper == codebook.len() {
        upper - 1
    } else if v - codebook[upper - 1] <= codebook[upper] - v {
        upper - 1
    } else {
        upper
    };
    best as u8
}

struct ShNData {
    bands: u32,
    centroid_count: usize,
    codebook: Vec<f32>,
    centroids: Pixels,
    labels: Pixels,
}

fn coeff_count_for_bands(bands: u32) -> usize {
    match bands {
        1 => 3,
        2 => 8,
        _ => 15,
    }
}

fn detect_bands(items: &[Gaussian3d]) -> Option<u32> {
    let used = |k: usize| {
        items.iter().any(|g| {
            [g.sh[3 + k], g.sh[18 + k], g.sh[33 + k]]
                .iter()
                .any(|v| *v != 0.0)
        })
    };

    if (8..15).any(used) {
        Some(3)
    } else if (3..8).any(used) {
        Some(2)
    } else if (0..3).any(used) {
        Some(1)
    } else {
        None
    }
}

// Balanced median-split clustering; writes one label per point and appends leaf means.
fn cluster(
    indices: &mut [u32],
    depth: u32,
    data: &[f32],
    dim: usize,
    labels: &mut [u32],
    centroids: &mut Vec<f32>,
) {
    let max_depth = MAX_SHN_CENTROIDS.ilog2();

    if indices.len() <= 1 || depth >= max_depth {
        let label = (centroids.len() / dim) as u32;
        let mut mean = vec![0.0f64; dim];
        for &i in indices.iter() {
            labels[i as usize] = label;
            for (m, v) in mean.iter_mut().zip(&data[i as usize * dim..][..dim]) {
                *m += *v as f64;
            }
        }
        let n = indices.len().max(1) as f64;
        centroids.extend(mean.iter().map(|m| (*m / n) as f32));
        return;
    }

    let n = indices.len() as f64;
    let mut mean = vec![0.0f64; dim];
    let mut sq = vec![0.0f64; dim];
    for &i in indices.iter() {
        for (k, v) in data[i as usize * dim..][..dim].iter().enumerate() {
            mean[k] += *v as f64;
            sq[k] += (*v as f64) * (*v as f64);
        }
    }
    let axis = (0..dim)
        .max_by(|&a, &b| {
            let var = |k: usize| sq[k] / n - (mean[k] / n) * (mean[k] / n);
            var(a).total_cmp(&var(b))
        })
        .unwrap();

    let mid = indices.len() / 2;
    indices.select_nth_unstable_by(mid, |&a, &b| {
        data[a as usize * dim + axis].total_cmp(&data[b as usize * dim + axis])
    });

    let (left, right) = indices.split_at_mut(mid);
    cluster(left, depth + 1, data, dim, labels, centroids);
    cluster(right, depth + 1, data, dim, labels, centroids);
}

fn build_shn(items: &[Gaussian3d], bands: u32, label_size: (u32, u32)) -> ShNData {
    let coeffs = coeff_count_for_bands(bands);
    let dim = coeffs * 3;

    let mut data = Vec::with_capacity(items.len() * dim);
    for g in items {
        for channel_base in [3, 18, 33] {
            data.extend_from_slice(&g.sh[channel_base..channel_base + coeffs]);
        }
    }

    let mut indices: Vec<u32> = (0..items.len() as u32).collect();
    let mut labels = vec![0u32; items.len()];
    let mut centroids = Vec::new();
    cluster(&mut indices, 0, &data, dim, &mut labels, &mut centroids);

    let centroid_count = centroids.len() / dim;
    let codebook = build_codebook(&centroids);

    let width = SHN_LABEL_ROW * coeffs;
    let height = centroid_count.div_ceil(SHN_LABEL_ROW).max(1);
    let mut centroid_pixels = Pixels::new(width as u32, height as u32, 3);
    for c in 0..centroid_count {
        let vector = &centroids[c * dim..][..dim];
        for k in 0..coeffs {
            let u = (c % SHN_LABEL_ROW) * coeffs + k;
            let v = c / SHN_LABEL_ROW;
            centroid_pixels.set(
                v * width + u,
                &[
                    nearest(&codebook, vector[k]),
                    nearest(&codebook, vector[coeffs + k]),
                    nearest(&codebook, vector[2 * coeffs + k]),
                ],
            );
        }
    }

    let mut label_pixels = Pixels::new(label_size.0, label_size.1, 3);
    for (i, &label) in labels.iter().enumerate() {
        label_pixels.set(i, &[(label & 0xff) as u8, (label >> 8) as u8, 0]);
    }

    ShNData {
        bands,
        centroid_count,
        codebook,
        centroids: centroid_pixels,
        labels: label_pixels,
    }
}

fn encode_quat(q: [f32; 4]) -> [u8; 4] {
    let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    let q = if norm > 0.0 {
        q.map(|v| v / norm)
    } else {
        [1.0, 0.0, 0.0, 0.0]
    };

    let largest = (0..4)
        .max_by(|&a, &b| q[a].abs().total_cmp(&q[b].abs()))
        .unwrap();
    let sign = if q[largest] < 0.0 { -1.0 } else { 1.0 };

    let mut out = [0u8; 4];
    let mut slot = 0;
    for (i, v) in q.iter().enumerate() {
        if i == largest {
            continue;
        }
        let n = (sign * v / std::f32::consts::SQRT_2 + 0.5) * 255.0;
        out[slot] = n.round().clamp(0.0, 255.0) as u8;
        slot += 1;
    }
    out[3] = 252 + largest as u8;
    out
}

pub fn write_sog(gaussians: &Gaussians) -> anyhow::Result<Vec<u8>> {
    let Gaussians::Gaussian3d(items) = gaussians else {
        bail!("SOG export supports only 3D Gaussians");
    };
    ensure!(!items.is_empty(), "nothing to export");

    let count = items.len();
    let (width, height) = image_size(count);

    // means: 16-bit values in the log domain split across two images
    let logged: Vec<[f32; 3]> = items
        .iter()
        .map(|g| g.position.map(log_transform))
        .collect();
    let mut mins = [f32::INFINITY; 3];
    let mut maxs = [f32::NEG_INFINITY; 3];
    for p in &logged {
        for k in 0..3 {
            mins[k] = mins[k].min(p[k]);
            maxs[k] = maxs[k].max(p[k]);
        }
    }

    let mut means_l = Pixels::new(width, height, 3);
    let mut means_u = Pixels::new(width, height, 3);
    for (i, p) in logged.iter().enumerate() {
        let mut lo = [0u8; 3];
        let mut hi = [0u8; 3];
        for k in 0..3 {
            let range = maxs[k] - mins[k];
            let q = if range > 0.0 {
                ((p[k] - mins[k]) / range * 65535.0)
                    .round()
                    .clamp(0.0, 65535.0) as u16
            } else {
                0
            };
            lo[k] = (q & 0xff) as u8;
            hi[k] = (q >> 8) as u8;
        }
        means_l.set(i, &lo);
        means_u.set(i, &hi);
    }

    let mut quats = Pixels::new(width, height, 4);
    for (i, g) in items.iter().enumerate() {
        quats.set(i, &encode_quat(g.rotation));
    }

    let scale_values: Vec<f32> = items.iter().flat_map(|g| g.scale).collect();
    let scale_codebook = build_codebook(&scale_values);
    let mut scales = Pixels::new(width, height, 3);
    for (i, g) in items.iter().enumerate() {
        scales.set(i, &g.scale.map(|v| nearest(&scale_codebook, v)));
    }

    let dc_values: Vec<f32> = items
        .iter()
        .flat_map(|g| [g.sh[0], g.sh[1], g.sh[2]])
        .collect();
    let sh0_codebook = build_codebook(&dc_values);
    let mut sh0 = Pixels::new(width, height, 4);
    for (i, g) in items.iter().enumerate() {
        let alpha = (sigmoid(g.opacity) * 255.0).round().clamp(0.0, 255.0) as u8;
        sh0.set(
            i,
            &[
                nearest(&sh0_codebook, g.sh[0]),
                nearest(&sh0_codebook, g.sh[1]),
                nearest(&sh0_codebook, g.sh[2]),
                alpha,
            ],
        );
    }

    let shn = detect_bands(items).map(|bands| build_shn(items, bands, (width, height)));

    let mut meta = json!({
        "version": 2,
        "asset": { "generator": "gaussianxr-editor" },
        "count": count,
        "antialias": false,
        "means": {
            "mins": mins,
            "maxs": maxs,
            "files": ["means_l.webp", "means_u.webp"],
        },
        "scales": { "codebook": scale_codebook, "files": ["scales.webp"] },
        "quats": { "files": ["quats.webp"] },
        "sh0": { "codebook": sh0_codebook, "files": ["sh0.webp"] },
    });
    if let Some(shn) = &shn {
        meta["shN"] = json!({
            "count": shn.centroid_count,
            "bands": shn.bands,
            "codebook": shn.codebook,
            "files": ["shN_centroids.webp", "shN_labels.webp"],
        });
    }

    let mut entries: Vec<(&str, Vec<u8>)> = vec![
        ("meta.json", serde_json::to_vec_pretty(&meta)?),
        ("means_l.webp", means_l.encode_webp()?),
        ("means_u.webp", means_u.encode_webp()?),
        ("quats.webp", quats.encode_webp()?),
        ("scales.webp", scales.encode_webp()?),
        ("sh0.webp", sh0.encode_webp()?),
    ];
    if let Some(shn) = &shn {
        entries.push(("shN_centroids.webp", shn.centroids.encode_webp()?));
        entries.push(("shN_labels.webp", shn.labels.encode_webp()?));
    }

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, bytes) in entries {
        zip.start_file(name, options)?;
        zip.write_all(&bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}
