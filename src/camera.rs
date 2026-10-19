use glam::*;
use winit::keyboard::KeyCode;

pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn build_view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }
    pub fn build_projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fovy.to_radians(), self.aspect, self.znear, self.zfar)
    }

    pub fn update_aspect(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CameraState {
    Idle,
    Active,
}

// first-person fly camera: WASD to move, right mouse drag to look
pub struct CameraController {
    move_speed: f32,
    look_sensitivity: f32,

    yaw: f32,
    pitch: f32,
    look_dirty: bool,

    forward_pressed: bool,
    backward_pressed: bool,
    left_pressed: bool,
    right_pressed: bool,
    up_pressed: bool,
    down_pressed: bool,
    boost_pressed: bool,
}

impl CameraController {
    pub fn new(speed: f32) -> Self {
        Self {
            move_speed: speed,
            look_sensitivity: 0.003,

            yaw: 0.0,
            pitch: 0.0,
            look_dirty: false,

            forward_pressed: false,
            backward_pressed: false,
            left_pressed: false,
            right_pressed: false,
            up_pressed: false,
            down_pressed: false,
            boost_pressed: false,
        }
    }

    pub fn sync_from_camera(&mut self, camera: &Camera) {
        let dir = (camera.target - camera.eye).normalize_or_zero();
        if dir == Vec3::ZERO {
            return;
        }

        self.yaw = dir.x.atan2(-dir.z);
        self.pitch = dir.y.clamp(-1.0, 1.0).asin();
    }

    pub fn adjust_speed(&mut self, scroll_steps: f32) {
        self.move_speed = (self.move_speed * 1.15_f32.powf(scroll_steps)).clamp(0.05, 1000.0);
    }

    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    pub fn reset_view(&mut self) {
        // yaw of PI faces +Z, the default heading
        self.yaw = std::f32::consts::PI;
        self.pitch = 0.0;
        self.look_dirty = true;
    }

    pub fn rotate(&mut self, dx: f32, dy: f32) {
        const MAX_PITCH: f32 = 89.0 * std::f32::consts::PI / 180.0;

        self.yaw += dx * self.look_sensitivity;
        self.pitch = (self.pitch - dy * self.look_sensitivity).clamp(-MAX_PITCH, MAX_PITCH);
        self.look_dirty = true;
    }

    fn forward(&self) -> Vec3 {
        Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            -self.pitch.cos() * self.yaw.cos(),
        )
    }

    pub fn handle_key(&mut self, code: KeyCode, is_pressed: bool) -> bool {
        let flag = match code {
            KeyCode::KeyW | KeyCode::ArrowUp => &mut self.forward_pressed,
            KeyCode::KeyS | KeyCode::ArrowDown => &mut self.backward_pressed,
            KeyCode::KeyA | KeyCode::ArrowLeft => &mut self.left_pressed,
            KeyCode::KeyD | KeyCode::ArrowRight => &mut self.right_pressed,
            KeyCode::KeyE | KeyCode::Space => &mut self.up_pressed,
            KeyCode::KeyQ | KeyCode::ControlLeft => &mut self.down_pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => &mut self.boost_pressed,
            _ => return false,
        };
        let changed = *flag != is_pressed;
        *flag = is_pressed;
        changed
    }

    pub fn release_all(&mut self) {
        self.forward_pressed = false;
        self.backward_pressed = false;
        self.left_pressed = false;
        self.right_pressed = false;
        self.up_pressed = false;
        self.down_pressed = false;
        self.boost_pressed = false;
    }

    pub fn update_camera(&mut self, camera: &mut Camera, dt_sec: f32) -> CameraState {
        let axis = |pos: bool, neg: bool| pos as i32 as f32 - neg as i32 as f32;
        let forward_input = axis(self.forward_pressed, self.backward_pressed);
        let right_input = axis(self.right_pressed, self.left_pressed);
        let up_input = axis(self.up_pressed, self.down_pressed);

        let moving = forward_input != 0.0 || right_input != 0.0 || up_input != 0.0;
        if !moving && !self.look_dirty {
            return CameraState::Idle;
        }

        let forward = self.forward();
        let right = forward.cross(Vec3::Y).normalize_or_zero();

        if moving {
            let boost = if self.boost_pressed { 4.0 } else { 1.0 };
            let dir = forward * forward_input + right * right_input + Vec3::Y * up_input;
            camera.eye += dir.normalize_or_zero() * self.move_speed * boost * dt_sec;
        }

        camera.target = camera.eye + forward;
        camera.up = Vec3::Y;
        self.look_dirty = false;

        if moving {
            CameraState::Active
        } else {
            CameraState::Idle
        }
    }
}
