use crate::adjust::{ColorAdjustments, SLIDERS};
use crate::renderer::ui::UiBatch;

const PANEL_WIDTH: f32 = 104.0;
const PADDING: f32 = 6.0;
const SLIDER_TOP: f32 = 22.0;
const SLIDER_STEP: f32 = 20.0;

const TEXT: [f32; 4] = [0.92, 0.92, 0.92, 1.0];
const PANEL: [f32; 4] = [0.07, 0.07, 0.08, 0.9];
const BUTTON: [f32; 4] = [0.2, 0.2, 0.22, 0.95];
const BUTTON_HOVER: [f32; 4] = [0.33, 0.33, 0.36, 0.95];
const TRACK: [f32; 4] = [0.38, 0.38, 0.4, 1.0];
const ACCENT: [f32; 4] = [1.0, 0.62, 0.15, 1.0];

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    fn contains(&self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Target {
    OpenButton,
    Close,
    Reset,
    Label(usize),
    Track(usize),
    Panel,
}

struct Layout {
    // Pixels per font pixel.
    u: f32,
    panel: Rect,
    open_button: Rect,
    close: Rect,
    reset: Rect,
    labels: [Rect; SLIDERS.len()],
    tracks: [Rect; SLIDERS.len()],
}

fn layout(screen: (f32, f32)) -> Layout {
    let u = (screen.1 / 230.0).floor().clamp(1.0, 3.0);
    let panel = Rect::new(screen.0 - PANEL_WIDTH * u, 0.0, PANEL_WIDTH * u, screen.1);
    let inner_x = panel.x + PADDING * u;
    let inner_w = (PANEL_WIDTH - 2.0 * PADDING) * u;

    let mut labels = [Rect::new(0.0, 0.0, 0.0, 0.0); SLIDERS.len()];
    let mut tracks = labels;
    for i in 0..SLIDERS.len() {
        let top = (SLIDER_TOP + i as f32 * SLIDER_STEP) * u;
        labels[i] = Rect::new(inner_x, top - u, inner_w, 9.0 * u);
        tracks[i] = Rect::new(inner_x, top + 9.0 * u, inner_w, 9.0 * u);
    }

    Layout {
        u,
        panel,
        open_button: Rect::new(screen.0 - 49.0 * u, 6.0 * u, 43.0 * u, 13.0 * u),
        close: Rect::new(
            panel.x + (PANEL_WIDTH - PADDING - 11.0) * u,
            4.0 * u,
            11.0 * u,
            11.0 * u,
        ),
        reset: Rect::new(
            inner_x,
            (SLIDER_TOP + SLIDERS.len() as f32 * SLIDER_STEP + 2.0) * u,
            inner_w,
            12.0 * u,
        ),
        labels,
        tracks,
    }
}

fn format_value(index: usize, value: f32) -> String {
    match index {
        2 => format!("{value:+.2}"),
        8 => format!("{:+}", value.round() as i32),
        _ => {
            let percent = (value * 100.0).round() as i32;
            if percent == 0 {
                "0".to_string()
            } else {
                format!("{percent:+}")
            }
        }
    }
}

// Collapsible side panel with the color adjustment sliders.
#[derive(Default)]
pub struct Sidebar {
    pub open: bool,
    pub values: ColorAdjustments,
    dragging: Option<usize>,
    hover: Option<Target>,
}

impl Sidebar {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn target_at(&self, layout: &Layout, pos: (f32, f32)) -> Option<Target> {
        if !self.open {
            return layout
                .open_button
                .contains(pos)
                .then_some(Target::OpenButton);
        }

        if layout.close.contains(pos) {
            return Some(Target::Close);
        }
        if layout.reset.contains(pos) {
            return Some(Target::Reset);
        }
        for i in 0..SLIDERS.len() {
            if layout.labels[i].contains(pos) {
                return Some(Target::Label(i));
            }
            if layout.tracks[i].contains(pos) {
                return Some(Target::Track(i));
            }
        }
        layout.panel.contains(pos).then_some(Target::Panel)
    }

    // Returns true when the hovered element changed and a redraw is needed.
    pub fn update_hover(&mut self, pos: (f32, f32), screen: (f32, f32)) -> bool {
        let target = self.target_at(&layout(screen), pos);
        let changed = target != self.hover;
        self.hover = target;
        changed
    }

    fn drag_to(&mut self, index: usize, x: f32, screen: (f32, f32)) {
        let track = layout(screen).tracks[index];
        let mut t = ((x - track.x) / track.w).clamp(0.0, 1.0);
        if (t - 0.5).abs() < 0.012 {
            t = 0.5;
        }
        let slider = &SLIDERS[index];
        self.values
            .set_value(index, slider.min + t * (slider.max - slider.min));
    }

    // Returns (click consumed, values changed).
    pub fn press(&mut self, pos: (f32, f32), screen: (f32, f32)) -> (bool, bool) {
        match self.target_at(&layout(screen), pos) {
            None => (false, false),
            Some(Target::Panel) => (true, false),
            Some(Target::OpenButton) => {
                self.open = true;
                (true, false)
            }
            Some(Target::Close) => {
                self.open = false;
                self.dragging = None;
                (true, false)
            }
            Some(Target::Reset) => {
                self.values = ColorAdjustments::default();
                (true, true)
            }
            Some(Target::Label(i)) => {
                self.values.set_value(i, 0.0);
                (true, true)
            }
            Some(Target::Track(i)) => {
                self.dragging = Some(i);
                self.drag_to(i, pos.0, screen);
                (true, true)
            }
        }
    }

    // Returns true when a slider moved.
    pub fn drag(&mut self, pos: (f32, f32), screen: (f32, f32)) -> bool {
        let Some(index) = self.dragging else {
            return false;
        };
        self.drag_to(index, pos.0, screen);
        true
    }

    // Returns true when a drag ended.
    pub fn release(&mut self) -> bool {
        self.dragging.take().is_some()
    }

    pub fn draw(&self, batch: &mut UiBatch, screen: (f32, f32)) {
        let l = layout(screen);
        let u = l.u;

        let button = |batch: &mut UiBatch, rect: Rect, target: Target, label: &str| {
            let color = if self.hover == Some(target) {
                BUTTON_HOVER
            } else {
                BUTTON
            };
            batch.rect(rect.x, rect.y, rect.w, rect.h, color);
            let width = UiBatch::text_width(label, u);
            batch.text(
                (rect.x + (rect.w - width) / 2.0).floor(),
                (rect.y + (rect.h - 7.0 * u) / 2.0).floor(),
                u,
                label,
                TEXT,
            );
        };

        if !self.open {
            button(batch, l.open_button, Target::OpenButton, "Adjust");
            return;
        }

        batch.rect(l.panel.x, l.panel.y, l.panel.w, l.panel.h, PANEL);
        batch.text(l.panel.x + PADDING * u, 6.0 * u, u, "Adjustments", TEXT);
        button(batch, l.close, Target::Close, "x");

        for (i, slider) in SLIDERS.iter().enumerate() {
            let value = self.values.value(i);
            let label = l.labels[i];
            let track = l.tracks[i];
            let hovered = self.hover == Some(Target::Label(i));

            batch.text(
                label.x,
                label.y + u,
                u,
                slider.label,
                if hovered { ACCENT } else { TEXT },
            );
            let text = format_value(i, value);
            batch.text(
                label.x + label.w - UiBatch::text_width(&text, u),
                label.y + u,
                u,
                &text,
                TEXT,
            );

            let center_y = track.y + track.h / 2.0;
            let center_x = track.x + track.w / 2.0;
            let t = (value - slider.min) / (slider.max - slider.min);
            let handle_x = (track.x + t * track.w).floor();

            batch.rect(
                track.x,
                (center_y - 1.5 * u).floor(),
                track.w,
                3.0 * u,
                TRACK,
            );
            let (from, to) = if handle_x < center_x {
                (handle_x, center_x)
            } else {
                (center_x, handle_x)
            };
            batch.rect(
                from,
                (center_y - 1.5 * u).floor(),
                to - from,
                3.0 * u,
                ACCENT,
            );
            batch.rect((center_x - u / 2.0).floor(), track.y, u, track.h, TRACK);

            let active = self.dragging == Some(i);
            batch.rect(
                handle_x - 2.0 * u,
                (center_y - 4.5 * u).floor(),
                4.0 * u,
                9.0 * u,
                if active { ACCENT } else { TEXT },
            );
        }

        button(batch, l.reset, Target::Reset, "Reset all");
    }
}
