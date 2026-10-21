use crate::resources::gaussians::Gaussians;

const SH0: f32 = 0.282_094_8;

pub struct SliderDef {
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
}

// Order matches `ColorAdjustments::value`. Ranges are symmetric so zero is the middle.
pub const SLIDERS: [SliderDef; 9] = [
    SliderDef {
        label: "Temperature",
        min: -1.0,
        max: 1.0,
    },
    SliderDef {
        label: "Tint",
        min: -1.0,
        max: 1.0,
    },
    SliderDef {
        label: "Exposure",
        min: -2.0,
        max: 2.0,
    },
    SliderDef {
        label: "Contrast",
        min: -1.0,
        max: 1.0,
    },
    SliderDef {
        label: "Highlights",
        min: -1.0,
        max: 1.0,
    },
    SliderDef {
        label: "Shadows",
        min: -1.0,
        max: 1.0,
    },
    SliderDef {
        label: "Saturation",
        min: -1.0,
        max: 1.0,
    },
    SliderDef {
        label: "Vibrance",
        min: -1.0,
        max: 1.0,
    },
    SliderDef {
        label: "Hue",
        min: -180.0,
        max: 180.0,
    },
];

// Lightroom-style grading applied to every splat color; all zero means unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColorAdjustments {
    pub temperature: f32,
    pub tint: f32,
    // In stops (EV).
    pub exposure: f32,
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub saturation: f32,
    pub vibrance: f32,
    // In degrees.
    pub hue: f32,
}

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl ColorAdjustments {
    pub fn value(&self, index: usize) -> f32 {
        match index {
            0 => self.temperature,
            1 => self.tint,
            2 => self.exposure,
            3 => self.contrast,
            4 => self.highlights,
            5 => self.shadows,
            6 => self.saturation,
            7 => self.vibrance,
            _ => self.hue,
        }
    }

    pub fn set_value(&mut self, index: usize, value: f32) {
        match index {
            0 => self.temperature = value,
            1 => self.tint = value,
            2 => self.exposure = value,
            3 => self.contrast = value,
            4 => self.highlights = value,
            5 => self.shadows = value,
            6 => self.saturation = value,
            7 => self.vibrance = value,
            _ => self.hue = value,
        }
    }

    pub fn is_neutral(&self) -> bool {
        *self == Self::default()
    }

    fn white_balance_gains(&self) -> [f32; 3] {
        [
            1.0 + 0.3 * self.temperature + 0.1 * self.tint,
            1.0 - 0.2 * self.tint,
            1.0 - 0.3 * self.temperature + 0.1 * self.tint,
        ]
    }

    // Packed for the preprocess shaders' `apply_grade`; keep both in sync with `apply`.
    pub fn uniform(&self) -> [[f32; 4]; 3] {
        let gains = self.white_balance_gains();
        [
            [
                2f32.powf(self.exposure),
                1.0 + self.contrast,
                1.0 + self.saturation,
                self.vibrance,
            ],
            [
                self.highlights,
                self.shadows,
                self.hue.to_radians(),
                if self.is_neutral() { 0.0 } else { 1.0 },
            ],
            [gains[0], gains[1], gains[2], 0.0],
        ]
    }

    // Grades one display-space color (the value after SH evaluation, before blending).
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        if self.is_neutral() {
            return rgb;
        }

        let gains = self.white_balance_gains();
        let exposure = 2f32.powf(self.exposure);
        let mut c = [
            rgb[0] * gains[0] * exposure,
            rgb[1] * gains[1] * exposure,
            rgb[2] * gains[2] * exposure,
        ];

        let l = luma(c);
        let shadow_weight = 1.0 - smoothstep(0.0, 0.5, l);
        let highlight_weight = smoothstep(0.5, 1.0, l);
        let gain =
            1.0 + 0.8 * self.shadows * shadow_weight + 0.5 * self.highlights * highlight_weight;
        let lift = 0.12 * self.shadows * shadow_weight;
        c = c.map(|v| v * gain + lift);

        let contrast = 1.0 + self.contrast;
        c = c.map(|v| (v - 0.5) * contrast + 0.5);

        let l = luma(c);
        let chroma = (c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])).clamp(0.0, 1.0);
        let saturation = (1.0 + self.saturation) * (1.0 + self.vibrance * (1.0 - chroma));
        c = c.map(|v| l + (v - l) * saturation);

        let (s, k) = self.hue.to_radians().sin_cos();
        let out = [
            c[0] * (0.213 + k * 0.787 - s * 0.213)
                + c[1] * (0.715 - k * 0.715 - s * 0.715)
                + c[2] * (0.072 - k * 0.072 + s * 0.928),
            c[0] * (0.213 - k * 0.213 + s * 0.143)
                + c[1] * (0.715 + k * 0.285 + s * 0.140)
                + c[2] * (0.072 - k * 0.072 - s * 0.283),
            c[0] * (0.213 - k * 0.213 - s * 0.787)
                + c[1] * (0.715 - k * 0.715 + s * 0.715)
                + c[2] * (0.072 + k * 0.928 + s * 0.072),
        ];
        out.map(|v| v.max(0.0))
    }

    // Rows are output channels, columns are input channels.
    fn jacobian(&self, base: [f32; 3]) -> [[f32; 3]; 3] {
        const EPS: f32 = 0.01;

        let mut jac = [[0.0; 3]; 3];
        for col in 0..3 {
            let mut hi = base;
            hi[col] += EPS;
            let mut lo = base;
            lo[col] -= EPS;
            let (a, b) = (self.apply(hi), self.apply(lo));
            for row in 0..3 {
                jac[row][col] = (a[row] - b[row]) / (2.0 * EPS);
            }
        }
        jac
    }
}

// Bakes the grade into the data: exact for the base color, and view-dependent SH
// coefficients are scaled by the local slope of the grade at that color.
pub fn bake_adjustments(gaussians: &mut Gaussians, adjustments: &ColorAdjustments) {
    if adjustments.is_neutral() {
        return;
    }

    match gaussians {
        Gaussians::Gaussian3d(items) => {
            for g in items {
                let base = [0, 1, 2].map(|c| (SH0 * g.sh[c] + 0.5).max(0.0));
                let graded = adjustments.apply(base);
                let jac = adjustments.jacobian(base);

                for k in 0..15 {
                    let v = [g.sh[3 + k], g.sh[18 + k], g.sh[33 + k]];
                    for (row, offset) in [3, 18, 33].into_iter().enumerate() {
                        g.sh[offset + k] =
                            jac[row][0] * v[0] + jac[row][1] * v[1] + jac[row][2] * v[2];
                    }
                }
                for c in 0..3 {
                    g.sh[c] = (graded[c] - 0.5) / SH0;
                }
            }
        }
        Gaussians::Gaussian4d(items) => {
            for g in items {
                let base = g.base_color.map(|v| v.clamp(0.0, 1.0));
                g.base_color = adjustments.apply(base);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::gaussians::Gaussian3d;

    fn splat() -> Gaussian3d {
        let mut sh = [0.0f32; 48];
        for (i, v) in sh.iter_mut().enumerate() {
            *v = ((i * 7 % 13) as f32 - 6.0) * 0.02;
        }
        Gaussian3d {
            position: [0.0; 3],
            opacity: 1.0,
            scale: [-3.0; 3],
            _pad0: 0,
            rotation: [1.0, 0.0, 0.0, 0.0],
            sh,
        }
    }

    #[test]
    fn neutral_adjustments_change_nothing() {
        let adjustments = ColorAdjustments::default();
        assert_eq!(adjustments.apply([0.2, 0.5, 0.9]), [0.2, 0.5, 0.9]);

        let original = splat();
        let mut gaussians = Gaussians::Gaussian3d(vec![original]);
        bake_adjustments(&mut gaussians, &adjustments);
        let Gaussians::Gaussian3d(items) = gaussians else {
            unreachable!()
        };
        assert_eq!(items[0].sh, original.sh);
    }

    #[test]
    fn exposure_doubles_base_color_and_view_dependent_terms() {
        let adjustments = ColorAdjustments {
            exposure: 1.0,
            ..Default::default()
        };

        let original = splat();
        let mut gaussians = Gaussians::Gaussian3d(vec![original]);
        bake_adjustments(&mut gaussians, &adjustments);
        let Gaussians::Gaussian3d(items) = gaussians else {
            unreachable!()
        };

        let before = (SH0 * original.sh[0] + 0.5).max(0.0);
        let after = SH0 * items[0].sh[0] + 0.5;
        assert!((after - before * 2.0).abs() < 1e-4);
        assert!((items[0].sh[3] - original.sh[3] * 2.0).abs() < 1e-3);
    }

    #[test]
    fn saturation_to_minimum_makes_gray() {
        let adjustments = ColorAdjustments {
            saturation: -1.0,
            ..Default::default()
        };
        let out = adjustments.apply([0.9, 0.2, 0.1]);
        assert!((out[0] - out[1]).abs() < 1e-5 && (out[1] - out[2]).abs() < 1e-5);
    }
}
