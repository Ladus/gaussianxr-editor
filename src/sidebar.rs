use crate::adjust::{ColorAdjustments, SLIDERS};
use crate::renderer::ui::UiBatch;

const PANEL_WIDTH: f32 = 120.0;
const PADDING: f32 = 6.0;
const SLIDER_TOP: f32 = 22.0;
const SLIDER_STEP: f32 = 20.0;
const RESET_TOP: f32 = SLIDER_TOP + COLOR_COUNT as f32 * SLIDER_STEP + 2.0;
const ROTATE_HEADER_TOP: f32 = 222.0;
const ROTATE_TOP: f32 = 236.0;
const MEASURE_HEADER_TOP: f32 = 300.0;
const MEASURE_TOP: f32 = 310.0;
const BUTTON_HEIGHT: f32 = 12.0;
const BUTTON_STEP: f32 = 14.0;
const EXPORT_HEADER_TOP: f32 = 356.0;
const EXPORT_TOP: f32 = 366.0;
// Panel content height in font pixels; the UI scale is chosen so this fits the window.
const CONTENT_HEIGHT: f32 = 394.0;

const COLOR_COUNT: usize = SLIDERS.len();
const ROTATION_COUNT: usize = 3;
const SLIDER_COUNT: usize = COLOR_COUNT + ROTATION_COUNT;

// Jog sliders that spring back to zero when released.
const ROTATION_LABELS: [&str; ROTATION_COUNT] = ["Rotate X", "Rotate Y", "Rotate Z"];
const ROTATION_RANGE: f32 = 180.0;

const TEXT: [f32; 4] = [0.92, 0.92, 0.92, 1.0];
const DIM: [f32; 4] = [0.6, 0.6, 0.64, 1.0];
const PANEL: [f32; 4] = [0.07, 0.07, 0.08, 0.9];
const BUTTON: [f32; 4] = [0.2, 0.2, 0.22, 0.95];
const BUTTON_HOVER: [f32; 4] = [0.33, 0.33, 0.36, 0.95];
const BUTTON_SET: [f32; 4] = [0.5, 0.32, 0.08, 0.95];
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
    // Sliders are indexed with the color sliders first, then the rotation sliders.
    Label(usize),
    Track(usize),
    Place(usize),
    Enter,
    Clear,
    Export(bool),
    Panel,
}

// Things the panel asks the app to do.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    PlacePoint(usize),
    EnterMeasurement,
    ClearMeasurement,
    // The flag forces .sog output, like F3.
    Export(bool),
}

#[derive(Default)]
pub struct Outcome {
    // The mouse event hit the panel, so the app should redraw.
    pub consumed: bool,
    pub adjustments_changed: bool,
    // Rotation axis index (0 = X, 1 = Y, 2 = Z) and the change in degrees.
    pub rotate: Option<(usize, f32)>,
    pub action: Option<Action>,
}

struct Layout {
    // Pixels per font pixel.
    u: f32,
    panel: Rect,
    open_button: Rect,
    close: Rect,
    reset: Rect,
    labels: [Rect; SLIDER_COUNT],
    tracks: [Rect; SLIDER_COUNT],
    rotate_header: (f32, f32),
    measure_header: (f32, f32),
    place: [Rect; 2],
    enter: Rect,
    clear: Rect,
    export_header: (f32, f32),
    export: [Rect; 2],
}

fn slider_top(index: usize) -> f32 {
    if index < COLOR_COUNT {
        SLIDER_TOP + index as f32 * SLIDER_STEP
    } else {
        ROTATE_TOP + (index - COLOR_COUNT) as f32 * SLIDER_STEP
    }
}

fn slider_range(index: usize) -> (f32, f32) {
    if index < COLOR_COUNT {
        (SLIDERS[index].min, SLIDERS[index].max)
    } else {
        (-ROTATION_RANGE, ROTATION_RANGE)
    }
}

fn slider_label(index: usize) -> &'static str {
    if index < COLOR_COUNT {
        SLIDERS[index].label
    } else {
        ROTATION_LABELS[index - COLOR_COUNT]
    }
}

fn layout(screen: (f32, f32)) -> Layout {
    let u = (screen.1 / CONTENT_HEIGHT).floor().clamp(1.0, 4.0);
    let panel = Rect::new(screen.0 - PANEL_WIDTH * u, 0.0, PANEL_WIDTH * u, screen.1);
    let inner_x = panel.x + PADDING * u;
    let inner_w = (PANEL_WIDTH - 2.0 * PADDING) * u;

    let mut labels = [Rect::new(0.0, 0.0, 0.0, 0.0); SLIDER_COUNT];
    let mut tracks = labels;
    for i in 0..SLIDER_COUNT {
        let top = slider_top(i) * u;
        labels[i] = Rect::new(inner_x, top - u, inner_w, 9.0 * u);
        tracks[i] = Rect::new(inner_x, top + 9.0 * u, inner_w, 9.0 * u);
    }

    let half_w = (inner_w - 2.0 * u) / 2.0;
    let button_row = |row: usize| (MEASURE_TOP + row as f32 * BUTTON_STEP) * u;

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
        reset: Rect::new(inner_x, RESET_TOP * u, inner_w, BUTTON_HEIGHT * u),
        labels,
        tracks,
        rotate_header: (inner_x, ROTATE_HEADER_TOP * u),
        measure_header: (inner_x, MEASURE_HEADER_TOP * u),
        place: [
            Rect::new(inner_x, button_row(0), half_w, BUTTON_HEIGHT * u),
            Rect::new(
                inner_x + half_w + 2.0 * u,
                button_row(0),
                half_w,
                BUTTON_HEIGHT * u,
            ),
        ],
        enter: Rect::new(inner_x, button_row(1), inner_w, BUTTON_HEIGHT * u),
        clear: Rect::new(inner_x, button_row(2), inner_w, BUTTON_HEIGHT * u),
        export_header: (inner_x, EXPORT_HEADER_TOP * u),
        export: [0, 1].map(|row| {
            Rect::new(
                inner_x,
                (EXPORT_TOP + row as f32 * BUTTON_STEP) * u,
                inner_w,
                BUTTON_HEIGHT * u,
            )
        }),
    }
}

fn format_value(index: usize, value: f32) -> String {
    if index >= COLOR_COUNT || index == 8 {
        return format!("{:+}", value.round() as i32);
    }
    if index == 2 {
        return format!("{value:+.2}");
    }

    let percent = (value * 100.0).round() as i32;
    if percent == 0 {
        "0".to_string()
    } else {
        format!("{percent:+}")
    }
}

// Collapsible side panel: color sliders, rotation jog sliders and measurement buttons.
#[derive(Default)]
pub struct Sidebar {
    pub open: bool,
    pub values: ColorAdjustments,
    rotation: [f32; ROTATION_COUNT],
    dragging: Option<usize>,
    hover: Option<Target>,
}

impl Sidebar {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    // True while a rotation slider is being dragged.
    pub fn is_rotating(&self) -> bool {
        self.dragging.is_some_and(|i| i >= COLOR_COUNT)
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
        for i in 0..SLIDER_COUNT {
            if layout.labels[i].contains(pos) {
                return Some(Target::Label(i));
            }
            if layout.tracks[i].contains(pos) {
                return Some(Target::Track(i));
            }
        }
        for (i, rect) in layout.place.iter().enumerate() {
            if rect.contains(pos) {
                return Some(Target::Place(i));
            }
        }
        if layout.enter.contains(pos) {
            return Some(Target::Enter);
        }
        if layout.clear.contains(pos) {
            return Some(Target::Clear);
        }
        for (i, rect) in layout.export.iter().enumerate() {
            if rect.contains(pos) {
                return Some(Target::Export(i == 1));
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

    fn drag_to(&mut self, index: usize, x: f32, screen: (f32, f32), outcome: &mut Outcome) {
        let track = layout(screen).tracks[index];
        let mut t = ((x - track.x) / track.w).clamp(0.0, 1.0);
        if (t - 0.5).abs() < 0.012 {
            t = 0.5;
        }
        let (min, max) = slider_range(index);
        let value = min + t * (max - min);

        if index < COLOR_COUNT {
            self.values.set_value(index, value);
            outcome.adjustments_changed = true;
        } else {
            let axis = index - COLOR_COUNT;
            let delta = value - self.rotation[axis];
            self.rotation[axis] = value;
            outcome.rotate = Some((axis, delta));
        }
    }

    pub fn press(&mut self, pos: (f32, f32), screen: (f32, f32)) -> Outcome {
        let mut outcome = Outcome::default();
        let target = self.target_at(&layout(screen), pos);
        outcome.consumed = target.is_some();

        match target {
            None | Some(Target::Panel) => {}
            Some(Target::OpenButton) => self.open = true,
            Some(Target::Close) => {
                self.open = false;
                self.dragging = None;
            }
            Some(Target::Reset) => {
                self.values = ColorAdjustments::default();
                outcome.adjustments_changed = true;
            }
            Some(Target::Label(i)) => {
                if i < COLOR_COUNT {
                    self.values.set_value(i, 0.0);
                    outcome.adjustments_changed = true;
                }
            }
            Some(Target::Track(i)) => {
                self.dragging = Some(i);
                self.drag_to(i, pos.0, screen, &mut outcome);
            }
            Some(Target::Place(i)) => outcome.action = Some(Action::PlacePoint(i)),
            Some(Target::Enter) => outcome.action = Some(Action::EnterMeasurement),
            Some(Target::Clear) => outcome.action = Some(Action::ClearMeasurement),
            Some(Target::Export(force_sog)) => outcome.action = Some(Action::Export(force_sog)),
        }
        outcome
    }

    pub fn drag(&mut self, pos: (f32, f32), screen: (f32, f32)) -> Outcome {
        let mut outcome = Outcome::default();
        if let Some(index) = self.dragging {
            outcome.consumed = true;
            self.drag_to(index, pos.0, screen, &mut outcome);
        }
        outcome
    }

    // Rotation sliders spring back to the center when released.
    pub fn release(&mut self) -> Outcome {
        let outcome = Outcome {
            consumed: self.dragging.is_some(),
            ..Default::default()
        };
        if self.is_rotating() {
            self.rotation = [0.0; ROTATION_COUNT];
        }
        self.dragging = None;
        outcome
    }

    pub fn draw(&self, batch: &mut UiBatch, screen: (f32, f32), points_set: [bool; 2]) {
        let l = layout(screen);
        let u = l.u;

        let button = |batch: &mut UiBatch, rect: Rect, target: Target, label: &str, set: bool| {
            let color = if self.hover == Some(target) {
                BUTTON_HOVER
            } else if set {
                BUTTON_SET
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
            button(batch, l.open_button, Target::OpenButton, "Adjust", false);
            return;
        }

        batch.rect(l.panel.x, l.panel.y, l.panel.w, l.panel.h, PANEL);
        batch.text(l.panel.x + PADDING * u, 6.0 * u, u, "Adjustments", TEXT);
        button(batch, l.close, Target::Close, "x", false);

        for i in 0..SLIDER_COUNT {
            let value = if i < COLOR_COUNT {
                self.values.value(i)
            } else {
                self.rotation[i - COLOR_COUNT]
            };
            let (min, max) = slider_range(i);
            let label = l.labels[i];
            let track = l.tracks[i];
            let hovered = self.hover == Some(Target::Label(i));

            batch.text(
                label.x,
                label.y + u,
                u,
                slider_label(i),
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
            let t = (value - min) / (max - min);
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

        button(batch, l.reset, Target::Reset, "Reset all", false);

        batch.text(l.rotate_header.0, l.rotate_header.1, u, "Rotate scene", DIM);
        batch.text(l.measure_header.0, l.measure_header.1, u, "Measure", DIM);
        button(
            batch,
            l.place[0],
            Target::Place(0),
            "Place 1",
            points_set[0],
        );
        button(
            batch,
            l.place[1],
            Target::Place(1),
            "Place 2",
            points_set[1],
        );
        button(batch, l.enter, Target::Enter, "Enter measurement", false);
        button(batch, l.clear, Target::Clear, "Clear", false);

        batch.text(l.export_header.0, l.export_header.1, u, "Export", DIM);
        button(
            batch,
            l.export[0],
            Target::Export(false),
            "Export original",
            false,
        );
        button(
            batch,
            l.export[1],
            Target::Export(true),
            "Export as .sog",
            false,
        );
    }
}
