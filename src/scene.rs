use crate::adjust::ColorAdjustments;
use glam::*;

pub const TIME_SPEED: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneType {
    Gaussian3d,
    Gaussian4d,
}

impl SceneType {
    pub fn is_dynamic(self) -> bool {
        self == Self::Gaussian4d
    }
}

pub struct Scene {
    pub scene_type: SceneType,
    pub gaussian_count: u32,
    pub time: f32,
    pub rotation: Quat,
    // Translation in display space, applied after the rotation.
    pub offset: Vec3,
    pub adjustments: ColorAdjustments,
}

impl Scene {
    pub fn new(scene_type: SceneType) -> Self {
        Self {
            scene_type,
            gaussian_count: 0,
            time: 0.0,
            rotation: Quat::IDENTITY,
            offset: Vec3::ZERO,
            adjustments: ColorAdjustments::default(),
        }
    }

    // Rotates the scene around `pivot` (a display-space point) about `axis`.
    pub fn rotate_about(&mut self, axis: Vec3, degrees: f32, pivot: Vec3) {
        let q = Quat::from_axis_angle(axis, degrees.to_radians());
        self.rotation = (q * self.rotation).normalize();
        self.offset = q * (self.offset - pivot) + pivot;
    }

    // Maps file-space points to display space.
    pub fn model_matrix(&self) -> Mat4 {
        Mat4::from_translation(self.offset) * Mat4::from_quat(self.view_rotation())
    }

    pub fn display_to_model(&self, point: Vec3) -> Vec3 {
        self.view_rotation().inverse() * (point - self.offset)
    }

    // Display-only: files are shown upside down to match the VR viewer's up axis.
    pub fn base_rotation() -> Quat {
        Quat::from_rotation_z(std::f32::consts::PI)
    }

    // Maps file data to what is shown on screen.
    pub fn view_rotation(&self) -> Quat {
        (self.rotation * Self::base_rotation()).normalize()
    }

    pub fn replace_gaussians(&mut self, scene_type: SceneType, gaussian_count: u32) {
        self.scene_type = scene_type;
        self.gaussian_count = gaussian_count;
        self.time = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        if self.scene_type.is_dynamic() {
            self.time = (self.time + dt * TIME_SPEED).rem_euclid(1.0);
        }
    }
}
