use glam::Vec3;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window},
};

use crate::{
    assets::{self, format::FileFormat},
    camera::{Camera, CameraController, CameraState},
    gpu::context::GpuContext,
    redraw_scheduler::RedrawScheduler,
    renderer::renderer::{RenderStatus, Renderer},
    resources::gaussians::Gaussians,
    scene::{Scene, SceneType},
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_time::{Duration, Instant};

#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

pub enum UserEvent {
    AppReady(AppState),

    #[cfg(target_arch = "wasm32")]
    ExportRequested,

    #[cfg(target_arch = "wasm32")]
    DroppedFileBytes {
        file_name: String,
        bytes: Vec<u8>,
    },
}

pub struct LoadedSource {
    format: FileFormat,
    path: std::path::PathBuf,
    gaussians: Gaussians,
}

pub struct AppState {
    gpu: GpuContext,

    source: Option<LoadedSource>,

    camera: Camera,
    camera_controller: CameraController,
    camera_state: CameraState,

    fps_timer: Instant,
    frame_count: u32,
    window: Arc<Window>,

    scene: Scene,

    renderer: Renderer,

    last_update_time: Instant,

    is_focused: bool,
    is_occluded: bool,

    // Draw one frame even if the window is unfocused (e.g. right after a drop from Explorer).
    force_frame: bool,

    is_looking: bool,

    redraw_scheduler: RedrawScheduler,
}

impl AppState {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let gpu = GpuContext::new(Arc::clone(&window)).await?;

        let surface_size = gpu.surface_size();
        let camera = Camera {
            eye: Vec3::ZERO,
            target: Vec3::Z,
            up: Vec3::Y,
            aspect: surface_size.width as f32 / surface_size.height as f32,
            fovy: 45.0,
            znear: 0.0001,
            zfar: 100.0,
        };
        let mut camera_controller = CameraController::new(5.0);
        camera_controller.sync_from_camera(&camera);

        let scene_type = SceneType::Gaussian3d;
        let scene = Scene::new(scene_type);

        let gaussians = Gaussians::Gaussian3d(Vec::new());
        let renderer = Renderer::new(&gpu, &camera, &gaussians, scene_type);

        let redraw_scheduler = RedrawScheduler::new(Arc::clone(&window));

        Ok(Self {
            gpu,
            source: None,
            camera,
            camera_controller,
            camera_state: CameraState::Idle,
            fps_timer: Instant::now(),
            frame_count: 0,
            window,
            scene,
            renderer,
            last_update_time: Instant::now(),
            is_focused: true,
            is_occluded: false,
            force_frame: false,
            is_looking: false,
            redraw_scheduler,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f64) {
        if !self.gpu.resize_surface(width, height, scale_factor) {
            return;
        }

        let surface_size = self.gpu.surface_size();
        let width = surface_size.width;
        let height = surface_size.height;
        log::info!("resize: ({width}, {height})");

        self.camera.update_aspect(width, height);

        self.renderer
            .resize(&mut self.gpu, &self.scene, &self.camera);

        self.redraw_scheduler.request_redraw();
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt_sec = (now - self.last_update_time).as_secs_f32();
        self.last_update_time = now;

        self.camera_state = self
            .camera_controller
            .update_camera(&mut self.camera, dt_sec);

        self.scene.update(dt_sec);

        self.renderer.update(&self.gpu, &self.scene, &self.camera);
    }

    pub fn render(&mut self) -> anyhow::Result<RenderStatus> {
        let status = self.renderer.render(&self.gpu, &self.redraw_scheduler)?;

        if status == RenderStatus::Submitted {
            self.update_fps_counter();
        }
        Ok(status)
    }

    fn handle_key(
        &mut self,
        event_loop: &ActiveEventLoop,
        code: KeyCode,
        is_pressed: bool,
        repeat: bool,
    ) {
        const ROTATE_STEP_DEGREES: f32 = 11.25;

        match (code, is_pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            (KeyCode::KeyR, true) => {
                self.camera_controller.reset_view();
                self.redraw_scheduler.request_redraw();
            }
            (KeyCode::F2, true) => {
                if !repeat {
                    self.export(false);
                }
            }
            (KeyCode::F3, true) => {
                if !repeat {
                    self.export(true);
                }
            }
            (KeyCode::KeyX | KeyCode::KeyY | KeyCode::KeyZ, true) => {
                if repeat {
                    return;
                }
                let axis = match code {
                    KeyCode::KeyX => Vec3::X,
                    KeyCode::KeyY => Vec3::Y,
                    _ => Vec3::Z,
                };
                let sign = if self.camera_controller.is_boost_pressed() {
                    -1.0
                } else {
                    1.0
                };
                self.scene.rotate(axis, sign * ROTATE_STEP_DEGREES);
                self.redraw_scheduler.request_redraw();
            }
            _ => {
                let changed = self.camera_controller.handle_key(code, is_pressed);

                if changed {
                    if is_pressed {
                        self.last_update_time = Instant::now();
                    }

                    self.redraw_scheduler.request_redraw();
                }
            }
        }
    }

    fn set_looking(&mut self, looking: bool) {
        if self.is_looking == looking {
            return;
        }
        self.is_looking = looking;

        if looking {
            // Windows has no Locked mode, so fall back to Confined
            let grab = self
                .window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined));
            if let Err(e) = grab {
                log::warn!("failed to grab cursor: {e}");
            }
            self.window.set_cursor_visible(false);
        } else {
            let _ = self.window.set_cursor_grab(CursorGrabMode::None);
            self.window.set_cursor_visible(true);
        }
    }

    fn handle_mouse_motion(&mut self, dx: f64, dy: f64) {
        if self.is_looking {
            self.camera_controller.rotate(dx as f32, dy as f32);
            self.redraw_scheduler.request_redraw();
        }
    }

    fn update_fps_counter(&mut self) {
        self.frame_count += 1;

        let elapsed = self.fps_timer.elapsed();
        if elapsed >= Duration::from_secs(1) {
            let fps = self.frame_count as f64 / elapsed.as_secs_f64();
            log::info!("FPS: {:.1}", fps);
            log::info!(
                "frame: {:.1} ms",
                elapsed.as_millis() as f32 / self.frame_count as f32
            );
            self.frame_count = 0;
            self.fps_timer = Instant::now();
        }
    }

    pub fn should_request_redraw(&self) -> bool {
        self.camera_state == CameraState::Active || self.scene.scene_type.is_dynamic()
    }

    pub fn replace_gaussians(
        &mut self,
        format: FileFormat,
        path: std::path::PathBuf,
        gaussians: Gaussians,
    ) -> anyhow::Result<()> {
        let new_scene_type = match &gaussians {
            Gaussians::Gaussian3d(_) => SceneType::Gaussian3d,
            Gaussians::Gaussian4d(_) => SceneType::Gaussian4d,
        };

        self.renderer
            .replace_gaussians(&mut self.gpu, &gaussians, new_scene_type);

        let gaussian_count = gaussians.len() as u32;
        self.scene.replace_gaussians(new_scene_type, gaussian_count);
        self.scene.rotation = glam::Quat::IDENTITY;

        // Start at the scene origin facing +Z, which is where exported scenes put the camera.
        self.camera.eye = Vec3::ZERO;
        self.camera.target = Vec3::Z;
        self.camera.up = Vec3::Y;
        self.camera_controller.sync_from_camera(&self.camera);

        self.force_frame = true;
        #[cfg(not(target_arch = "wasm32"))]
        self.window.focus_window();
        self.redraw_scheduler.request_redraw();

        self.source = Some(LoadedSource {
            format,
            path,
            gaussians,
        });

        Ok(())
    }

    // Center-crops the viewport to a square and saves it as a 512x512 PNG.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_gallery_image(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        use anyhow::Context;
        use image::imageops;

        const GALLERY_SIZE: u32 = 512;

        let (width, height, pixels) = self.renderer.capture(&self.gpu)?;
        let full = image::RgbaImage::from_raw(width, height, pixels)
            .context("captured frame has unexpected size")?;

        let side = width.min(height);
        let square = imageops::crop_imm(&full, (width - side) / 2, (height - side) / 2, side, side)
            .to_image();
        let resized = imageops::resize(
            &square,
            GALLERY_SIZE,
            GALLERY_SIZE,
            imageops::FilterType::Lanczos3,
        );

        resized.save(path)?;
        Ok(())
    }

    // Writes the loaded scene with the current rotation baked in and the camera as origin.
    // `force_sog` exports as .sog even when the loaded file was a .ply.
    pub fn export(&mut self, force_sog: bool) {
        self.renderer.update(&self.gpu, &self.scene, &self.camera);

        let Some(source) = &self.source else {
            log::warn!("nothing to export");
            return;
        };
        let format = if force_sog {
            FileFormat::Sog
        } else {
            source.format
        };

        // Express the on-screen result in the file's own frame so base orientation is not baked in.
        // The exported scene is centered on the camera and turned about up so the current heading becomes +Z.
        let to_file_frame = Scene::base_rotation().inverse();
        let heading =
            glam::Quat::from_rotation_y(self.camera_controller.yaw() + std::f32::consts::PI);
        let bytes = match assets::exporter::export_gaussians(
            format,
            &source.gaussians,
            to_file_frame * heading * self.scene.view_rotation(),
            to_file_frame * heading * self.camera.eye,
        ) {
            Ok(bytes) => bytes,
            Err(e) => {
                log::error!("export failed: {e:?}");
                return;
            }
        };

        let stem = source
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("scene");
        let out_name = format!("{stem}.{}", format.extension());

        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = source.path.with_file_name(stem);
            let out_path = dir.join(&out_name);
            let write =
                std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&out_path, &bytes));
            match write {
                Ok(()) => log::info!("exported: {out_path:?}"),
                Err(e) => {
                    log::error!("failed to write {out_path:?}: {e}");
                    return;
                }
            }

            let gallery_path = dir.join(format!("{stem}.gallery.png"));

            // Hand-written so the key order matches what the XR viewer's samples use.
            let title = serde_json::to_string(stem).unwrap_or_else(|_| "\"scene\"".to_string());
            let transform_json = format!(
                "{{\n  \"audio\": {{\n    \"ambientLoop\": {{\n      \"file\": \"scene.mp3\",\n      \"volume\": 0.7,\n      \"fadeInTime\": 2.0\n    }}\n  }},\n  \"title\": {title}\n}}\n"
            );
            let json_path = dir.join(format!("{stem}.xrSceneTransform.json"));
            match std::fs::write(&json_path, transform_json) {
                Ok(()) => log::info!("exported: {json_path:?}"),
                Err(e) => log::error!("failed to write {json_path:?}: {e}"),
            }

            match self.save_gallery_image(&gallery_path) {
                Ok(()) => log::info!("exported: {gallery_path:?}"),
                Err(e) => log::error!("failed to write {gallery_path:?}: {e:?}"),
            }
        }

        #[cfg(target_arch = "wasm32")]
        match download_bytes(&out_name, &bytes) {
            Ok(()) => log::info!("exported: {out_name}"),
            Err(e) => log::error!("failed to download {out_name}: {e:?}"),
        }
    }
}

pub struct App {
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
    state: Option<AppState>,
    scale_factor: f64,
    window_size: winit::dpi::PhysicalSize<u32>,
}

impl App {
    pub fn new(#[cfg(target_arch = "wasm32")] event_loop: &EventLoop<UserEvent>) -> Self {
        #[cfg(target_arch = "wasm32")]
        let proxy = Some(event_loop.create_proxy());
        Self {
            state: None,
            #[cfg(target_arch = "wasm32")]
            proxy,
            scale_factor: 1.0,
            window_size: winit::dpi::PhysicalSize::new(1, 1),
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes();

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;

            const CANVAS_ID: &str = "canvas";

            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();
            let canvas = document.get_element_by_id(CANVAS_ID).unwrap_throw();
            let html_canvas_element = canvas.unchecked_into();
            window_attributes = window_attributes.with_canvas(Some(html_canvas_element));
        }

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let width = window.inner_size().width;
        let height = window.inner_size().height;
        self.window_size = winit::dpi::PhysicalSize::new(width, height);
        log::info!("resume window size: ({width}, {height})");

        self.scale_factor = window.scale_factor();

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.state = Some(pollster::block_on(AppState::new(window)).unwrap());
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(proxy) = self.proxy.take() {
                install_web_drag_and_drop(proxy.clone());
                install_web_export_button(proxy.clone());
                wasm_bindgen_futures::spawn_local(async move {
                    assert!(
                        proxy
                            .send_event(UserEvent::AppReady(
                                AppState::new(window)
                                    .await
                                    .expect("Unable to create canvas!!")
                            ))
                            .is_ok()
                    )
                });
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size.width, size.height, self.scale_factor);
                self.window_size = winit::dpi::PhysicalSize::new(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.resize(
                    self.window_size.width,
                    self.window_size.height,
                    scale_factor,
                );
                self.scale_factor = scale_factor;
            }
            #[cfg(not(target_arch = "wasm32"))]
            WindowEvent::DroppedFile(path) => {
                let format = assets::format::FileFormat::from_path(&path);
                let Some(format) = format else {
                    log::error!("unsupported file format: {path:?}");
                    return;
                };
                match assets::loader::load_gaussians_from_path(format, &path).and_then(
                    |gaussians| {
                        let gaussian_count = gaussians.len();
                        state.replace_gaussians(format, path.clone(), gaussians)?;
                        Ok(gaussian_count)
                    },
                ) {
                    Ok(gaussian_count) => {
                        log::info!("loaded dropped file: {path:?}");
                        log::info!("gaussian count: {gaussian_count}");
                        state.redraw_scheduler.request_redraw();
                    }
                    Err(e) => {
                        log::error!("{e:?}: {path:?}");
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if state.is_occluded || (!state.is_focused && !state.force_frame) {
                    return;
                }
                state.force_frame = false;

                if !state.redraw_scheduler.try_begin_frame() {
                    return;
                }

                state.update();

                match state.render() {
                    Ok(RenderStatus::Submitted) => {}

                    Ok(RenderStatus::Skipped) => {
                        state.redraw_scheduler.cancel_frame();
                    }

                    Err(error) => {
                        state.redraw_scheduler.cancel_frame();

                        log::error!("{error}");
                        event_loop.exit();
                        return;
                    }
                }

                if state.should_request_redraw() {
                    state.redraw_scheduler.request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        repeat,
                        ..
                    },
                ..
            } => state.handle_key(event_loop, code, key_state.is_pressed(), repeat),
            WindowEvent::MouseInput {
                state: button_state,
                button: MouseButton::Right,
                ..
            } => state.set_looking(button_state.is_pressed()),
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 40.0,
                };
                state.camera_controller.adjust_speed(steps);
            }
            WindowEvent::Focused(focused) => {
                state.is_focused = focused;

                if !focused {
                    state.set_looking(false);
                    state.camera_controller.release_all();
                }

                if focused {
                    state.last_update_time = Instant::now();
                    state.redraw_scheduler.request_redraw();
                } else {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if let Err(error) = state.gpu.poll_once() {
                            log::error!("failed to poll GPU device: {error:?}");
                        }
                    }
                }
            }

            WindowEvent::Occluded(occluded) => {
                state.is_occluded = occluded;

                if !occluded {
                    state.last_update_time = Instant::now();
                    state.redraw_scheduler.request_redraw();
                } else {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if let Err(error) = state.gpu.poll_once() {
                            log::error!("failed to poll GPU device: {error:?}");
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let (Some(state), DeviceEvent::MouseMotion { delta }) = (&mut self.state, event) {
            state.handle_mouse_motion(delta.0, delta.1);
        }
    }

    #[allow(unused_mut)]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::AppReady(mut state) => {
                #[cfg(target_arch = "wasm32")]
                {
                    state.redraw_scheduler.request_redraw();
                    state.resize(
                        state.window.inner_size().width,
                        state.window.inner_size().height,
                        state.window.scale_factor(),
                    );

                    let scale_factor = state.window.scale_factor();
                    log::info!("{scale_factor}");
                }

                self.state = Some(state);
            }

            #[cfg(target_arch = "wasm32")]
            UserEvent::DroppedFileBytes { file_name, bytes } => {
                let Some(state) = &mut self.state else {
                    return;
                };

                let format = assets::format::FileFormat::from_file_name(&file_name);
                let Some(format) = format else {
                    log::error!("unsupported file format: {file_name}");
                    return;
                };

                match assets::loader::load_gaussians_from_bytes(format, &bytes).and_then(
                    |gaussians| {
                        let gaussian_count = gaussians.len();
                        state.replace_gaussians(format, file_name.clone().into(), gaussians)?;
                        Ok(gaussian_count)
                    },
                ) {
                    Ok(gaussian_count) => {
                        log::info!("loaded dropped file: {file_name}");
                        log::info!("gaussian count: {gaussian_count}");
                        state.redraw_scheduler.request_redraw();
                    }
                    Err(e) => {
                        log::error!("{e:?}: {file_name}");
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            UserEvent::ExportRequested => {
                if let Some(state) = &mut self.state {
                    state.export(false);
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn download_bytes(file_name: &str, bytes: &[u8]) -> Result<(), JsValue> {
    use wasm_bindgen::JsCast;

    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;

    let document = wgpu::web_sys::window()
        .unwrap_throw()
        .document()
        .unwrap_throw();
    let anchor: web_sys::HtmlAnchorElement = document.create_element("a")?.unchecked_into();
    anchor.set_href(&url);
    anchor.set_download(file_name);
    anchor.click();

    web_sys::Url::revoke_object_url(&url)
}

#[cfg(target_arch = "wasm32")]
fn install_web_export_button(proxy: winit::event_loop::EventLoopProxy<UserEvent>) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let document = wgpu::web_sys::window()
        .unwrap_throw()
        .document()
        .unwrap_throw();
    let Some(button) = document.get_element_by_id("export-btn") else {
        return;
    };
    let button: web_sys::HtmlElement = button.unchecked_into();

    let handler = Closure::<dyn FnMut()>::new(move || {
        let _ = proxy.send_event(UserEvent::ExportRequested);

        // give keyboard focus back to the canvas so WASD keeps working
        if let Some(canvas) = document.get_element_by_id("canvas") {
            let canvas: web_sys::HtmlElement = canvas.unchecked_into();
            let _ = canvas.focus();
        }
    });
    button.set_onclick(Some(handler.as_ref().unchecked_ref()));
    handler.forget();
}

#[cfg(target_arch = "wasm32")]
fn install_web_drag_and_drop(proxy: winit::event_loop::EventLoopProxy<UserEvent>) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use web_sys::{DragEvent, FileReader};

    let window = wgpu::web_sys::window().unwrap_throw();
    let document = window.document().unwrap_throw();
    let body = document.body().unwrap_throw();

    let dragover_handler = Closure::<dyn FnMut(DragEvent)>::new(|event: DragEvent| {
        event.prevent_default();
    });
    body.set_ondragover(Some(dragover_handler.as_ref().unchecked_ref()));
    dragover_handler.forget();

    let proxy = std::rc::Rc::new(proxy);
    let drop_handler = Closure::<dyn FnMut(DragEvent)>::new(move |event: DragEvent| {
        event.prevent_default();
        let proxy = proxy.clone();
        if let Some(dt) = event.data_transfer() {
            if let Some(files) = dt.files() {
                if let Some(file) = files.item(0) {
                    let reader = FileReader::new().unwrap_throw();
                    let reader_clone = reader.clone();
                    let file_name = file.name();
                    let onload = Closure::<dyn FnMut()>::new(move || {
                        if let Ok(result) = reader_clone.result() {
                            if let Some(ab) = result.dyn_ref::<js_sys::ArrayBuffer>() {
                                let bytes = js_sys::Uint8Array::new(ab).to_vec();
                                let _ = proxy.send_event(UserEvent::DroppedFileBytes {
                                    file_name: file_name.clone(),
                                    bytes,
                                });
                            }
                        }
                    });
                    reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                    onload.forget();
                    reader.read_as_array_buffer(&file).unwrap_throw();
                }
            }
        }
    });
    body.set_ondrop(Some(drop_handler.as_ref().unchecked_ref()));
    drop_handler.forget();
}
