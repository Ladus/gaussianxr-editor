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
}

impl Scene {
    pub fn new(scene_type: SceneType) -> Self {
        Self {
            scene_type,
            gaussian_count: 0,
            time: 0.0,
            rotation: Quat::IDENTITY,
        }
    }

    pub fn rotate(&mut self, axis: Vec3, degrees: f32) {
        let q = Quat::from_axis_angle(axis, degrees.to_radians());
        self.rotation = (q * self.rotation).normalize();
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
