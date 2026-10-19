use crate::camera::Camera;
use crate::gpu::context::GpuContext;
use crate::redraw_scheduler::RedrawScheduler;
use crate::resources::gaussians::Gaussians;
use crate::scene::{Scene, SceneType};

use super::measure::MeasurePass;
use super::text_overlay::{TextOverlay, TextStyle};
use glam::{Mat4, Vec3};

const PLACEHOLDER_TEXT: &str = "Drag your scene here to load it\n\nPress F1 for help";

const HELP_TEXT: &str = "\
GAUSSIAN SPLAT VIEWER HELP                      (F1 to close)

NAVIGATION
  W A S D / arrows      Move
  E / Space             Up
  Q / Ctrl              Down
  Shift                 Move faster
  Hold right mouse      Look around
  Mouse wheel           Change movement speed
  R                     Reset view direction
  Hold X / Y / Z        + move mouse up/down to rotate the scene

KEYS
  F1   Show / hide this help
  F2   Export in the loaded file's format (.ply or .sog)
  F3   Export as an optimized .sog (3D scenes only)

MEASURE (set the real-world scale)
  1 / 2   Place point 1 / 2 on the splat under the + crosshair
  3       Type the line length in meters (for example 0.6) and press
          Enter. The whole scene is scaled so the line has that length.

EXPORT
  Each export creates a folder named after the loaded file. It holds
  the scene, a thumbnail (.gallery.png) for the GaussianXR gallery
  taken from the camera's current viewpoint, and an
  .xrSceneTransform.json file.

  The camera position becomes the origin of the scene in the Arcturus
  GaussianXR app, and the direction you face becomes forward.

  Optional: add a file named scene.mp3 to the exported folder to play
  it as looping background sound.";

#[cfg(not(feature = "tile-renderer"))]
pub use super::global_sort::renderer::GlobalSortRenderer as ActiveRendererBackend;
#[cfg(feature = "tile-renderer")]
pub use super::tile::renderer::TileRenderer as ActiveRendererBackend;

#[derive(PartialEq, Eq)]
pub enum RenderStatus {
    Submitted,
    Skipped,
}

pub struct Renderer {
    backend: ActiveRendererBackend,
    // Shown until the first scene is loaded.
    placeholder: Option<TextOverlay>,
    help: TextOverlay,
    show_help: bool,

    measure: MeasurePass,
    measure_points: [Option<Vec3>; 2],
    crosshair: TextOverlay,
    // Length prompt shown while the user types a measurement.
    input_prompt: TextOverlay,
    show_input_prompt: bool,
}

impl Renderer {
    pub fn new(
        gpu: &GpuContext,
        camera: &Camera,
        gaussians: &Gaussians,
        scene_type: SceneType,
    ) -> Self {
        let backend = ActiveRendererBackend::new(gpu, gaussians, camera, scene_type);
        let placeholder = Some(TextOverlay::new(
            gpu,
            PLACEHOLDER_TEXT,
            TextStyle {
                center: true,
                max_width: 0.5,
                max_height: 0.3,
                background_alpha: 0.0,
            },
        ));
        let help = TextOverlay::new(
            gpu,
            HELP_TEXT,
            TextStyle {
                center: false,
                max_width: 0.9,
                max_height: 0.9,
                background_alpha: 0.85,
            },
        );

        Self {
            backend,
            placeholder,
            help,
            show_help: false,
            measure: MeasurePass::new(gpu),
            measure_points: [None, None],
            crosshair: TextOverlay::new(
                gpu,
                "+",
                TextStyle {
                    center: true,
                    max_width: 0.02,
                    max_height: 0.02,
                    background_alpha: 0.0,
                },
            ),
            input_prompt: TextOverlay::new(
                gpu,
                "",
                TextStyle {
                    center: true,
                    max_width: 0.6,
                    max_height: 0.4,
                    background_alpha: 0.85,
                },
            ),
            show_input_prompt: false,
        }
    }

    pub fn set_measure_points(&mut self, points: [Option<Vec3>; 2]) {
        self.measure_points = points;
    }

    // `None` hides the prompt.
    pub fn set_input_prompt(&mut self, gpu: &GpuContext, text: Option<&str>) {
        self.show_input_prompt = text.is_some();
        if let Some(text) = text {
            self.input_prompt.set_text(gpu, text);
        }
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn update(&mut self, gpu: &GpuContext, scene: &Scene, camera: &Camera) {
        self.backend.update_scene_uniform(gpu, scene, camera);

        let view_proj = camera.build_projection_matrix()
            * camera.build_view_matrix()
            * Mat4::from_quat(scene.view_rotation());
        self.measure.update(
            gpu,
            view_proj,
            self.measure_points[0],
            self.measure_points[1],
        );
    }

    pub fn render(
        &mut self,
        gpu: &GpuContext,
        redraw_scheduler: &RedrawScheduler,
    ) -> anyhow::Result<RenderStatus> {
        if !gpu.is_surface_configured() {
            redraw_scheduler.cancel_frame();
            return Ok(RenderStatus::Skipped);
        }

        let output = match gpu.get_current_texture() {
            Ok(Some(output)) => output,

            Ok(None) => {
                redraw_scheduler.cancel_frame();
                return Ok(RenderStatus::Skipped);
            }

            Err(error) => {
                redraw_scheduler.cancel_frame();
                return Err(error);
            }
        };

        let output_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = gpu.create_command_encoder("Render Encoder");

        self.backend.render(&mut encoder, &output_view, true);
        self.measure.encode(&mut encoder, &output_view);

        if let Some(placeholder) = &self.placeholder {
            if !self.show_help {
                placeholder.encode(gpu, &mut encoder, &output_view);
            }
        }

        if self.show_help {
            self.help.encode(gpu, &mut encoder, &output_view);
        } else if self.placeholder.is_none() {
            self.crosshair.encode(gpu, &mut encoder, &output_view);
        }

        if self.show_input_prompt {
            self.input_prompt.encode(gpu, &mut encoder, &output_view);
        }

        redraw_scheduler.submit_frame(gpu, encoder.finish());

        output.present();

        Ok(RenderStatus::Submitted)
    }

    // Renders the current view without overlays and returns (width, height, RGBA8).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn capture(&mut self, gpu: &GpuContext) -> anyhow::Result<(u32, u32, Vec<u8>)> {
        gpu.capture_frame(|encoder, view| self.backend.render(encoder, view, false))
    }

    pub fn resize(&mut self, gpu: &mut GpuContext, scene: &Scene, camera: &Camera) {
        self.backend.resize(gpu, scene, camera);
    }

    pub fn replace_gaussians(
        &mut self,
        gpu: &mut GpuContext,
        gaussians: &Gaussians,
        scene_type: SceneType,
    ) {
        self.placeholder = None;
        self.backend.replace_gaussians(gpu, gaussians, scene_type);
    }
}
