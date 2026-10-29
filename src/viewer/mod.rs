//! The interactive 3D view.
//!
//! Mouse controls (as in the original editor, except the right button):
//! * left or right drag: orbit (turntable or trackball) around the model
//!   point under the pointer, or the view centre over empty space
//! * left click: select
//! * middle drag or Ctrl + left drag: pan
//! * Shift + left drag: zoom; mouse wheel: zoom at the pointer
//! * Ctrl + click: add to / remove from the selection

pub mod camera;
pub mod capture;
mod culling;
mod ground;
mod overlays;
mod picking;
mod renderer;
mod section;
mod shadow;
mod stereo;
mod views;

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Color32, Pos2, Rect, Sense, Vec2};
use egui_phosphor::regular as icon;
use glam::{Mat4 as GMat4, Vec2 as GVec2, Vec3};

use crate::commands::Action;
use crate::scene::{Aabb, Grid, MAX_EXPLODE, Mesh, NodeKey, Scene, TopologySelection};
use crate::settings::{self, NamedView, OrbitMethod};
use camera::{Animation, Camera, StandardView};
use capture::{CaptureOptions, TurntableExport, TurntableOptions};
use culling::Detail;
use ground::{GRID_FADE, GroundGrid, ScaleBar};
pub use overlays::CubeColors;
use overlays::{NavAction, NavigationCube};
use renderer::{DrawItem, FrameInput, Layer, SceneRenderer, Style};
pub use section::{Section, SectionAxis};
use shadow::GroundShadow;

/// Zoom factor per point of scrolling (a wheel notch is about 50 points).
const ZOOM_PER_SCROLL_POINT: f32 = 0.9978;
/// Zoom factor per point of vertical drag with the zoom button.
const ZOOM_PER_DRAG_POINT: f32 = 1.006;
/// Each overlay button changes the magnification by this ratio.
const ZOOM_BUTTON_STEP: f32 = 1.25;
/// Slider range relative to the framing established by the last Fit command.
const MIN_ZOOM_PERCENT: f32 = 10.0;
const MAX_ZOOM_PERCENT: f32 = 1600.0;
const ZOOM_CONTROLS_MARGIN: f32 = 12.0;
const ZOOM_CONTROLS_HEIGHT: f32 = 42.0;
const ZOOM_CONTROLS_WIDTH: f32 = 280.0;
const MIN_VIEW_WIDTH_FOR_ZOOM_CONTROLS: f32 = 220.0;
const SECTION_CONTROLS_WIDTH: f32 = 430.0;
const EXPLODE_CONTROLS_WIDTH: f32 = 340.0;
const ISOLATION_CONTROLS_WIDTH: f32 = 330.0;
const CAPTURE_CONTROLS_WIDTH: f32 = 360.0;
const MESHING_CONTROLS_WIDTH: f32 = 360.0;
/// How far the exploded view first spreads the parts.
const DEFAULT_EXPLODE: f32 = 0.5;
/// Duration of camera transitions, seconds.
const ANIMATION_SECONDS: f64 = 0.25;
const POINT_SIZE: f32 = 7.0;
/// Face opacity in the X-ray display mode.
const XRAY_OPACITY: f32 = 0.3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DisplayMode {
    #[default]
    Shaded,
    Wireframe,
    /// See-through faces, so that every edge shows.
    XRay,
    /// Faces filled with the background: the visible edges are drawn, the
    /// hidden ones as faint dashes (unless turned off in the preferences).
    HiddenLine,
    /// For showing the model rather than working on it: the settings of
    /// Preferences > Presentation (no edges, studio lights, a material for
    /// objects without one, a light background, no helpers by default).
    Presentation,
}

impl DisplayMode {
    pub const ALL: [DisplayMode; 5] = [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::XRay,
        DisplayMode::HiddenLine,
        DisplayMode::Presentation,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DisplayMode::Shaded => "Shaded",
            DisplayMode::Wireframe => "Wireframe",
            DisplayMode::XRay => "X-Ray",
            DisplayMode::HiddenLine => "Hidden Line",
            DisplayMode::Presentation => "Presentation",
        }
    }

    /// How a scene part is drawn in this mode. Wireframe draws every edge;
    /// the other modes draw those the edge style keeps, and the outline of
    /// curved surfaces with the Outline style. Wires are drawn whole.
    fn style(
        self,
        look: PartLook,
        settings: &settings::Viewer,
        rendering: &settings::Rendering,
    ) -> Style {
        let presentation = &rendering.presentation;
        let edge_style = if self == DisplayMode::Presentation {
            presentation.edges
        } else {
            rendering.edges
        };
        let filtered = look.has_faces && self != DisplayMode::Wireframe;
        let (edge_min_angle, silhouettes) = if filtered {
            (
                edge_style.minimum_angle(rendering.sharp_angle),
                edge_style == settings::EdgeStyle::Outline,
            )
        } else {
            (0.0, false)
        };
        let edges = match self {
            // The edges are all there is to see.
            DisplayMode::Wireframe | DisplayMode::HiddenLine => true,
            DisplayMode::Shaded | DisplayMode::XRay => look.edges.unwrap_or(settings.show_edges),
            DisplayMode::Presentation => look.edges.unwrap_or(presentation.show_edges),
        } || !look.has_faces;
        let mut face_color = look.face_color;
        match self {
            DisplayMode::XRay => face_color[3] = face_color[3].min(XRAY_OPACITY),
            // Transparent faces would not hide the edges behind them.
            DisplayMode::HiddenLine => face_color[3] = 1.0,
            DisplayMode::Shaded | DisplayMode::Wireframe | DisplayMode::Presentation => {}
        }
        let edge_width = if look.has_faces {
            settings.edge_width
        } else {
            settings.edge_width + 1.0
        };
        Style {
            face_color,
            edge_color: look.edge_color,
            highlight: look.highlight,
            edge_width: edge_width + if look.emphasized { 0.5 } else { 0.0 },
            point_size: POINT_SIZE,
            faces: self != DisplayMode::Wireframe,
            edges,
            points: true,
            clipped: true,
            background_fill: self == DisplayMode::HiddenLine,
            hidden_edges: self == DisplayMode::HiddenLine && settings.hidden_edges,
            edge_min_angle,
            silhouettes,
            surface: look.finish.surface(),
        }
    }
}

/// What a frame is drawn on and lit with, by display mode.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Backdrop {
    lighting: settings::Lighting,
    /// Top and bottom of the background gradient, linear.
    top: [f32; 4],
    bottom: [f32; 4],
    shadow: Option<GroundShadow>,
}

/// What a scene part looks like before the display mode applies.
struct PartLook {
    face_color: [f32; 4],
    edge_color: [f32; 4],
    /// Selection or hover tint.
    highlight: [f32; 4],
    has_faces: bool,
    /// Selected or hovered: drawn with wider edges.
    emphasized: bool,
    /// The object's own edge visibility, when set in the Objects panel.
    edges: Option<bool>,
    finish: settings::Finish,
}

/// Restricts which entity type a click may select. `Auto` preserves the
/// vertex-before-edge-before-face behavior used by earlier releases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    #[default]
    Auto,
    Object,
    Face,
    Edge,
    Vertex,
}

impl SelectionMode {
    pub const ALL: [Self; 5] = [
        Self::Auto,
        Self::Object,
        Self::Face,
        Self::Edge,
        Self::Vertex,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Object => "Object",
            Self::Face => "Face",
            Self::Edge => "Edge",
            Self::Vertex => "Vertex",
        }
    }

    fn allows_faces(self) -> bool {
        matches!(self, Self::Auto | Self::Object | Self::Face)
    }

    fn allows_edges(self) -> bool {
        matches!(self, Self::Auto | Self::Object | Self::Edge)
    }

    fn allows_vertices(self) -> bool {
        matches!(self, Self::Auto | Self::Object | Self::Vertex)
    }

    fn accepts(self, entity: Option<crate::scene::TopologyEntity>) -> bool {
        match self {
            Self::Auto | Self::Object => true,
            Self::Face => matches!(entity, Some(crate::scene::TopologyEntity::Face(_))),
            Self::Edge => matches!(entity, Some(crate::scene::TopologyEntity::Edge(_))),
            Self::Vertex => matches!(entity, Some(crate::scene::TopologyEntity::Vertex(_))),
        }
    }

    fn entity(
        self,
        entity: Option<crate::scene::TopologyEntity>,
    ) -> Option<crate::scene::TopologyEntity> {
        (self != Self::Object).then_some(entity).flatten()
    }
}

/// Colors the view takes from the UI theme.
#[derive(Clone)]
pub struct ViewerColors {
    pub selection: Color32,
    pub hover: Color32,
    pub overlay: Color32,
    pub grid: Color32,
    pub cube: CubeColors,
    pub overlay_text: Color32,
    pub overlay_background: Color32,
}

/// What happened in the view this frame.
pub enum ViewerEvent {
    /// A click selected `key` (or nothing); `extend` toggles instead of
    /// replacing the selection.
    Picked {
        key: Option<NodeKey>,
        topology: Option<TopologySelection>,
        extend: bool,
    },
    /// The user double-clicked an object.
    FocusRequested,
    /// A screenshot or an animation was saved (with a message saying
    /// where), or could not be.
    Captured(Result<String, String>),
    /// A control in the view asks for this.
    Command(Action),
}

/// Per-frame inputs from the application.
pub struct ViewInput<'a> {
    pub scene: &'a Scene,
    pub settings: &'a settings::Viewer,
    pub rendering: &'a settings::Rendering,
    pub colors: &'a ViewerColors,
    /// Shown as a status pill while the worker is busy.
    pub busy: Option<&'a str>,
    /// Shown in the middle of an empty view.
    pub empty_hint: Option<&'a str>,
    /// Parts meshed and parts in all, while a render's meshing takes long.
    pub meshing: Option<(u32, u32)>,
}

#[derive(Clone, Copy, PartialEq)]
enum DragMode {
    Orbit,
    Pan,
    Zoom,
}

impl DragMode {
    fn cursor(self) -> egui::CursorIcon {
        match self {
            Self::Orbit | Self::Pan => egui::CursorIcon::Grabbing,
            Self::Zoom => egui::CursorIcon::ResizeVertical,
        }
    }
}

struct Drag {
    mode: DragMode,
    start: Pos2,
    last: Pos2,
    pivot: Option<Vec3>,
}

impl Drag {
    /// Returns only the motion since the previous frame.
    fn advance(&mut self, current: Pos2) -> Vec2 {
        let delta = current - self.last;
        self.last = current;
        delta
    }
}

pub struct Viewer {
    pub camera: Camera,
    pub display_mode: DisplayMode,
    pub selection_mode: SelectionMode,
    pub section: Section,
    /// The exploded-view controls are shown.
    pub explode_controls: bool,
    /// The named views of the document, in the order they were saved.
    pub named_views: Vec<NamedView>,
    animation: Option<Animation>,
    renderer: Option<SceneRenderer>,
    nav_cube: NavigationCube,
    drag: Option<Drag>,
    hovered: Option<NodeKey>,
    last_pointer: Option<Pos2>,
    orbit_method: OrbitMethod,
    aspect: f32,
    /// Camera extent treated as 100% by the zoom controls.
    zoom_reference_extent: f32,
    capture: Option<Capture>,
    /// Size of the view in physical pixels, as last drawn.
    view_px: [u32; 2],
    /// Small parts are drawn simplified (large-model mode).
    simplifying: bool,
    axis_meshes: Option<(f32, [Arc<Mesh>; 3])>,
    /// The dot marking the world origin where the axes start.
    origin_mesh: Arc<Mesh>,
    grid_mesh: Option<(GridKey, Arc<Mesh>)>,
    /// The ground grid with its lines, ring by ring from the middle out.
    ground: Option<(GroundGrid, Vec<GroundRing>)>,
    pick_index: picking::SceneIndex,
}

/// The minor and the major lines of one fading ring of the ground grid.
type GroundRing = (Arc<Mesh>, Arc<Mesh>);

#[derive(Clone, Copy, PartialEq)]
struct GridKey([f32; 10]);

impl Default for Viewer {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            display_mode: DisplayMode::Shaded,
            selection_mode: SelectionMode::Auto,
            section: Section::default(),
            explode_controls: false,
            named_views: Vec::new(),
            animation: None,
            renderer: None,
            nav_cube: NavigationCube::default(),
            drag: None,
            hovered: None,
            last_pointer: None,
            orbit_method: OrbitMethod::Turntable,
            aspect: 1.5,
            zoom_reference_extent: Camera::default().extent,
            capture: None,
            view_px: [1, 1],
            simplifying: false,
            axis_meshes: None,
            origin_mesh: Arc::new(Mesh::new(
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![[0.0; 3]],
            )),
            grid_mesh: None,
            ground: None,
            pick_index: picking::SceneIndex::default(),
        }
    }
}

impl Viewer {
    /// Frames everything visible.
    pub fn fit(&mut self, scene: &Scene, animate: bool, now: f64) {
        let pose = self.camera.fit_pose(scene.visible_bounds(), self.aspect);
        self.zoom_reference_extent = pose.extent;
        self.animate_to(pose, animate, now);
    }

    /// Frames the selection, or everything when nothing is selected.
    pub fn fit_selection(&mut self, scene: &Scene, animate: bool, now: f64) {
        let bounds = scene.selection_bounds();
        if bounds.is_empty() {
            self.fit(scene, animate, now);
        } else {
            let pose = self.camera.fit_pose(bounds, self.aspect);
            self.zoom_reference_extent = pose.extent;
            self.animate_to(pose, animate, now);
        }
    }

    pub fn set_standard_view(&mut self, view: StandardView, animate: bool, now: f64) {
        let mut pose = self.settled_pose();
        pose.rotation = view.rotation();
        self.animate_to(pose, animate, now);
    }

    /// Saves the next frame as a PNG image of the given size and kind,
    /// unless a turntable is being exported (`false`).
    pub fn request_image(&mut self, path: PathBuf, options: CaptureOptions) -> bool {
        if self.turntable_progress().is_some() {
            return false;
        }
        self.capture = Some(Capture::Image {
            path: Some(path),
            options,
        });
        true
    }

    /// Copies the next frame to the system clipboard as an image of the given
    /// size and kind, unless a turntable is being exported (`false`).
    pub fn request_clipboard_image(&mut self, options: CaptureOptions) -> bool {
        if self.turntable_progress().is_some() {
            return false;
        }
        self.capture = Some(Capture::Image {
            path: None,
            options,
        });
        true
    }

    /// Starts writing a turntable animation, one frame per frame of the
    /// view, from where the camera is heading.
    pub fn start_turntable(
        &mut self,
        path: PathBuf,
        options: CaptureOptions,
        turntable: TurntableOptions,
    ) -> Result<(), String> {
        self.cancel_turntable();
        let export =
            TurntableExport::start(path, options, turntable, self.settled_pose(), self.view_px)?;
        self.capture = Some(Capture::Turntable(Box::new(export)));
        Ok(())
    }

    /// Stops a turntable export and removes what it wrote.
    pub fn cancel_turntable(&mut self) {
        if let Some(Capture::Turntable(export)) = self.capture.take() {
            export.cancel();
        }
    }

    /// Frames written and frames in all, while a turntable is exported.
    pub fn turntable_progress(&self) -> Option<(u32, u32)> {
        match &self.capture {
            Some(Capture::Turntable(export)) => Some(export.progress()),
            _ => None,
        }
    }

    /// The size of the view in physical pixels, as last drawn.
    pub fn view_px(&self) -> [u32; 2] {
        self.view_px
    }

    /// Turns the section plane on, inside the model, or off.
    pub fn toggle_section(&mut self, scene: &Scene) {
        self.section.enabled = !self.section.enabled;
        if self.section.enabled {
            self.section.center_in(scene.visible_bounds());
        }
    }

    /// Spreads the parts and shows the exploded-view controls, or closes
    /// them and assembles the parts again.
    pub fn toggle_explode(&mut self, scene: &mut Scene) {
        self.explode_controls = !self.explode_controls;
        scene.set_explode(if self.explode_controls {
            DEFAULT_EXPLODE
        } else {
            0.0
        });
    }

    /// The pose the camera is heading to (the current one when idle).
    fn settled_pose(&self) -> camera::Pose {
        self.animation
            .as_ref()
            .map_or_else(|| self.camera.pose(), Animation::target)
    }

    fn animate_to(&mut self, pose: camera::Pose, animate: bool, now: f64) {
        if animate {
            self.animation = Some(Animation::new(
                self.camera.pose(),
                pose,
                now,
                ANIMATION_SECONDS,
            ));
        } else {
            self.animation = None;
            self.camera.set_pose(pose);
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        frame: &eframe::Frame,
        input: ViewInput<'_>,
    ) -> Vec<ViewerEvent> {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let now = ui.input(|i| i.time);
        self.aspect = (rect.width() / rect.height().max(1.0)).max(1e-3);

        self.apply_settings(input.settings);
        let bounds = input.scene.visible_bounds();
        self.update_scene_extent(input.scene, bounds);
        let zoom_controls = zoom_controls_rect(rect);
        let mut row = 0;
        let mut bar = |shown: bool, width: f32| {
            let bar = shown.then(|| bottom_bar_rect(rect, row, width)).flatten();
            row += usize::from(bar.is_some());
            bar
        };
        let section_controls = bar(self.section.enabled, SECTION_CONTROLS_WIDTH);
        let explode_controls = bar(self.explode_controls, EXPLODE_CONTROLS_WIDTH);
        let isolation_controls = bar(input.scene.is_isolating(), ISOLATION_CONTROLS_WIDTH);
        let capture_controls = bar(self.turntable_progress().is_some(), CAPTURE_CONTROLS_WIDTH);
        let meshing_controls = bar(input.meshing.is_some(), MESHING_CONTROLS_WIDTH);
        let controls: Vec<Rect> = [
            zoom_controls,
            section_controls,
            explode_controls,
            isolation_controls,
            capture_controls,
            meshing_controls,
        ]
        .into_iter()
        .flatten()
        .collect();
        let mut events = self.handle_input(ui, &response, rect, &controls, &input, now);

        if let Some(animation) = &self.animation {
            let (pose, done) = animation.sample(now);
            self.camera.set_pose(pose);
            if done {
                self.animation = None;
            } else {
                ui.ctx().request_repaint();
            }
        }

        self.update_ground(input.settings.ground_grid, bounds, rect);
        self.update_scene_extent(input.scene, bounds);

        let painter = ui.painter_at(rect);
        match frame.wgpu_render_state() {
            Some(render_state) => {
                let shown_triangles = input
                    .scene
                    .visible_nodes()
                    .iter()
                    .flat_map(|node| &node.parts)
                    .map(|part| part.mesh.triangle_count())
                    .sum();
                self.simplifying = input.settings.large_models.active(shown_triangles);
                let items = self.draw_items(&input, self.simplifying);
                // Captures show every part in full, whatever its size here.
                let full_items = (self.simplifying && self.capture.is_some())
                    .then(|| self.draw_items(&input, false));
                let backdrop = self.backdrop(&input);
                let renderer = self
                    .renderer
                    .get_or_insert_with(|| SceneRenderer::new(render_state));
                let ppp = ui.ctx().pixels_per_point();
                let size = [
                    (rect.width() * ppp).round().max(1.0) as u32,
                    (rect.height() * ppp).round().max(1.0) as u32,
                ];
                self.view_px = size;
                let settings = input.settings;
                let frame_input = FrameInput {
                    camera: &self.camera,
                    size,
                    background_top: backdrop.top,
                    background_bottom: backdrop.bottom,
                    items: &items,
                    section: self.section.renderer_plane(bounds),
                    pixel_scale: 1.0,
                    eye: self.camera.mono_eye(),
                    lighting: backdrop.lighting,
                    ground_shadow: backdrop.shadow,
                };
                let stereo = (settings.projection == settings::Projection::Stereo)
                    .then_some(settings.stereo_mode);
                let texture = {
                    let mut egui_renderer = render_state.renderer.write();
                    match stereo {
                        Some(mode) => {
                            renderer.render_stereo(&mut egui_renderer, &frame_input, mode)
                        }
                        None => renderer.render(&mut egui_renderer, &frame_input),
                    }
                };
                let mut render_pixels = |frame: &FrameInput<'_>| match stereo {
                    Some(mode) => renderer.render_stereo_pixels(frame, mode),
                    None => renderer.render_pixels(frame),
                };
                painter.image(
                    texture,
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                let capture_frame = FrameInput {
                    items: full_items.as_deref().unwrap_or(&items),
                    ..frame_input
                };
                match self.capture.take() {
                    Some(Capture::Image { path, options }) => {
                        let image = FrameInput {
                            size: options.image_size(size),
                            pixel_scale: options.pixel_scale(size),
                            ..capture_input(capture_frame, options.transparent)
                        };
                        let captured = match (render_pixels(&image), path) {
                            (Some((size, pixels)), Some(path)) => {
                                capture::save_png(&path, size, pixels, options.transparent)
                                    .map(|()| format!("Screenshot saved to {}", path.display()))
                            }
                            (Some((size, pixels)), None) => {
                                let color_image = capture::prepare_clipboard_image(
                                    size,
                                    pixels,
                                    options.transparent,
                                );
                                ui.ctx().copy_image(color_image);
                                Ok(format!(
                                    "Screenshot copied to clipboard ({} × {} px)",
                                    size[0], size[1]
                                ))
                            }
                            (None, _) => Err("the GPU did not return the image".to_owned()),
                        };
                        events.push(ViewerEvent::Captured(captured));
                    }
                    Some(Capture::Turntable(mut export)) => {
                        let mut camera = self.camera.clone();
                        camera.set_pose(export.next_pose());
                        let frame = FrameInput {
                            camera: &camera,
                            size: export.size(),
                            pixel_scale: export.pixel_scale(),
                            ..capture_input(capture_frame, export.transparent())
                        };
                        let written = render_pixels(&frame)
                            .ok_or_else(|| "the GPU did not return the image".to_owned())
                            .and_then(|(_, pixels)| export.write_frame(pixels));
                        match written {
                            Ok(false) => {
                                self.capture = Some(Capture::Turntable(export));
                                ui.ctx().request_repaint();
                            }
                            Ok(true) => events.push(ViewerEvent::Captured(export.finish())),
                            Err(error) => {
                                export.cancel();
                                events.push(ViewerEvent::Captured(Err(error)));
                            }
                        }
                    }
                    None => {}
                }
            }
            None => {
                painter.rect_filled(rect, 0.0, Color32::from_gray(40));
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "3D view unavailable: no GPU renderer",
                    egui::FontId::proportional(14.0),
                    Color32::LIGHT_GRAY,
                );
            }
        }

        self.paint_overlays(&painter, rect, &input);
        if let Some(controls) = zoom_controls {
            self.show_zoom_controls(ui, rect, controls);
        }
        if let Some(controls) = section_controls {
            self.show_section_controls(ui, rect, controls, bounds);
        }
        if let Some(controls) = explode_controls {
            show_explode_controls(ui, rect, controls, input.scene, &mut events);
        }
        if let Some(controls) = isolation_controls {
            show_isolation_controls(ui, rect, controls, input.scene, &mut events);
        }
        if let (Some(controls), Some(progress)) = (capture_controls, self.turntable_progress()) {
            show_capture_controls(ui, rect, controls, progress, &mut events);
        }
        if let (Some(controls), Some(progress)) = (meshing_controls, input.meshing) {
            show_meshing_controls(ui, rect, controls, progress, &mut events);
        }
        events
    }

    /// The lights, the background and the ground shadow of the display
    /// mode: Presentation has its own.
    fn backdrop(&self, input: &ViewInput<'_>) -> Backdrop {
        if self.display_mode == DisplayMode::Presentation {
            let presentation = &input.rendering.presentation;
            return Backdrop {
                lighting: presentation.lighting,
                top: linear_rgba(presentation.background_top, 1.0),
                bottom: linear_rgba(presentation.background_bottom, 1.0),
                shadow: presentation.shadow.then_some(GroundShadow {
                    strength: presentation.shadow_strength,
                    softness: presentation.shadow_softness,
                }),
            };
        }
        let settings = input.settings;
        let top = linear_rgba(settings.background, 1.0);
        let bottom = if settings.use_gradient {
            linear_rgba(settings.background_aux, 1.0)
        } else {
            top
        };
        Backdrop {
            lighting: input.rendering.lighting,
            top,
            bottom,
            shadow: None,
        }
    }

    fn apply_settings(&mut self, settings: &settings::Viewer) {
        self.camera.projection = settings.projection;
        if settings.orbit_method != self.orbit_method {
            self.orbit_method = settings.orbit_method;
            if self.orbit_method == OrbitMethod::Turntable {
                self.camera.level();
            }
        }
    }

    /// The ground grid under what is shown (`bounds`). Its lines are rebuilt
    /// only when the model changes or the view needs a larger floor.
    fn update_ground(&mut self, shown: bool, bounds: Aabb, rect: Rect) {
        if !shown {
            self.ground = None;
            return;
        }
        let grid = GroundGrid::under(
            bounds,
            &self.camera,
            GVec2::new(rect.width(), rect.height()),
        );
        match &mut self.ground {
            Some((old, _)) if old.same_lines(&grid) => *old = grid,
            _ => {
                let bands = grid
                    .faded_meshes()
                    .into_iter()
                    .map(|(minor, major)| (Arc::new(minor), Arc::new(major)))
                    .collect();
                self.ground = Some((grid, bands));
            }
        }
    }

    /// Keeps the clipping planes around everything that can be drawn.
    fn update_scene_extent(&mut self, scene: &Scene, bounds: Aabb) {
        let mut radius = if bounds.is_empty() {
            10.0
        } else {
            bounds.diagonal() * 0.5
        };
        let mut center = if bounds.is_empty() {
            Vec3::ZERO
        } else {
            bounds.center()
        };
        if scene.axes.iter().any(|a| a.visible) {
            // The helpers run through the origin.
            let half = self.camera.extent * 0.45;
            radius = radius.max(half + center.length());
            center *= 0.5;
        }
        if let Some(grid) = &scene.overlay.grid {
            radius = radius.max(grid.size * 1.5 + (grid.origin - center).length());
        }
        if let Some((ground, _)) = &self.ground {
            let reach = ground
                .corners()
                .iter()
                .map(|corner| (*corner - center).length())
                .fold(0.0, f32::max);
            radius = radius.max(reach);
        }
        self.camera.scene_center = center;
        self.camera.scene_radius = radius.max(1e-3);
    }

    fn handle_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        controls: &[Rect],
        input: &ViewInput<'_>,
        now: f64,
    ) -> Vec<ViewerEvent> {
        let mut events = Vec::new();
        let show_cube = input.settings.show_navigation_cube;
        let pointer = response.hover_pos();
        let over_cube = |p: Pos2| show_cube && NavigationCube::contains(rect, p);
        // The floating controls (zoom, section) take their own input.
        let over_zoom_controls = |p: Pos2| controls.iter().any(|area| area.contains(p));

        if show_cube {
            let hover_pos = pointer.filter(|_| self.drag.is_none());
            if self.nav_cube.hover(rect, self.camera.rotation, hover_pos) {
                ui.ctx().request_repaint();
            }
        } else {
            self.nav_cube.clear_hover();
        }

        // Dragging.
        if response.drag_started() {
            let start = ui
                .input(|i| i.pointer.press_origin())
                .or(pointer)
                .unwrap_or(rect.center());
            let (modifiers, primary, middle, secondary) = ui.input(|i| {
                (
                    i.modifiers,
                    i.pointer.button_down(egui::PointerButton::Primary),
                    i.pointer.button_down(egui::PointerButton::Middle),
                    i.pointer.button_down(egui::PointerButton::Secondary),
                )
            });
            let mode = if over_zoom_controls(start) || over_cube(start) {
                None
            } else {
                requested_drag_mode(modifiers, primary, middle, secondary)
            };
            self.drag = None;
            if let Some(mode) = mode {
                self.start_drag(input.scene, rect, mode, start);
            }
        }
        if response.dragged()
            && let Some(current) = response.interact_pointer_pos()
            && let Some(drag) = &mut self.drag
        {
            let (mode, start, previous, pivot) = (drag.mode, drag.start, drag.last, drag.pivot);
            let delta = drag.advance(current);
            match mode {
                DragMode::Orbit => match self.orbit_method {
                    OrbitMethod::Turntable => self.camera.orbit_turntable_about(
                        GVec2::new(delta.x, delta.y),
                        pivot.unwrap_or(self.camera.target),
                    ),
                    OrbitMethod::Trackball => {
                        self.camera.orbit_trackball_about(
                            trackball_coords(rect, previous),
                            trackball_coords(rect, current),
                            pivot.unwrap_or(self.camera.target),
                        );
                    }
                },
                DragMode::Pan => self.camera.pan(GVec2::new(delta.x, delta.y), rect.height()),
                DragMode::Zoom => {
                    let anchor = ndc(rect, start);
                    self.camera
                        .zoom(ZOOM_PER_DRAG_POINT.powf(delta.y), anchor, self.aspect);
                }
            }
        }
        if response.drag_stopped() {
            self.drag = None;
        }

        // Wheel and pinch zoom at the pointer.
        if let Some(pos) = pointer.filter(|p| response.hovered() && !over_zoom_controls(*p)) {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = ZOOM_PER_SCROLL_POINT.powf(scroll) / pinch;
            if (factor - 1.0).abs() > 1e-6 {
                self.settle_animation();
                self.camera.zoom(factor, ndc(rect, pos), self.aspect);
            }
        }

        // Clicks: the cube, then object selection.
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
            && !over_zoom_controls(pos)
        {
            if over_cube(pos) {
                if let Some(action) = self.nav_cube.click(rect, self.camera.rotation, pos) {
                    self.apply_nav_action(action, input.settings.animate, now);
                }
            } else {
                let extend = ui.input(|i| i.modifiers.command);
                let picked = self.pick(input.scene, rect, pos);
                events.push(ViewerEvent::Picked {
                    key: picked.as_ref().map(|(key, _)| key.clone()),
                    topology: picked.and_then(|(_, topology)| topology),
                    extend,
                });
            }
        }
        if response.double_clicked()
            && pointer.is_some_and(|p| !over_cube(p) && !over_zoom_controls(p))
        {
            events.push(ViewerEvent::FocusRequested);
        }

        // Hover highlighting, recomputed only when the pointer moves.
        let hover_pos =
            pointer.filter(|p| self.drag.is_none() && !over_cube(*p) && !over_zoom_controls(*p));
        if hover_pos != self.last_pointer {
            self.last_pointer = hover_pos;
            let hovered = hover_pos
                .and_then(|p| self.pick(input.scene, rect, p))
                .map(|(key, _)| key);
            if hovered != self.hovered {
                self.hovered = hovered;
                ui.ctx().request_repaint();
            }
        }

        // Give immediate feedback on mouse-down, before the movement is large
        // enough for egui to classify it as a drag. Once dragging starts, keep
        // the cursor even if the pointer leaves the viewport.
        let cursor_mode = self.drag.as_ref().map(|drag| drag.mode).or_else(|| {
            let (origin, modifiers, primary, middle, secondary) = ui.input(|i| {
                (
                    i.pointer.press_origin(),
                    i.modifiers,
                    i.pointer.button_down(egui::PointerButton::Primary),
                    i.pointer.button_down(egui::PointerButton::Middle),
                    i.pointer.button_down(egui::PointerButton::Secondary),
                )
            });
            origin
                .filter(|p| rect.contains(*p) && !over_cube(*p) && !over_zoom_controls(*p))
                .and_then(|_| requested_drag_mode(modifiers, primary, middle, secondary))
        });
        if let Some(mode) = cursor_mode {
            ui.ctx().set_cursor_icon(mode.cursor());
        }
        events
    }

    fn apply_nav_action(&mut self, action: NavAction, animate: bool, now: f64) {
        let mut pose = self.settled_pose();
        match action {
            NavAction::LookFrom { eye_dir, up } => {
                pose.rotation = camera::look_rotation(eye_dir, up)
            }
            NavAction::Roll(angle) => {
                let forward = pose.rotation * Vec3::NEG_Z;
                pose.rotation =
                    (glam::Quat::from_axis_angle(forward, angle) * pose.rotation).normalize();
            }
        }
        self.settle_animation();
        self.animate_to(pose, animate, now);
    }

    /// Jumps to the end of a running animation, so user input starts from a
    /// stable camera.
    fn settle_animation(&mut self) {
        if let Some(animation) = self.animation.take() {
            self.camera.set_pose(animation.target());
        }
    }

    fn zoom_percent(&self) -> f32 {
        self.zoom_reference_extent / self.camera.extent * 100.0
    }

    /// Sets magnification relative to the last Fit framing. The center of the
    /// view stays fixed; pointer-oriented zoom remains available on the wheel.
    fn set_zoom_percent(&mut self, percent: f32) {
        let percent = percent.clamp(MIN_ZOOM_PERCENT, MAX_ZOOM_PERCENT);
        let desired_extent = self.zoom_reference_extent * 100.0 / percent;
        self.camera.zoom(
            desired_extent / self.camera.extent,
            GVec2::ZERO,
            self.aspect,
        );
    }

    fn show_zoom_controls(&mut self, parent: &mut egui::Ui, view: Rect, controls: Rect) {
        let mut requested_zoom = self
            .zoom_percent()
            .clamp(MIN_ZOOM_PERCENT, MAX_ZOOM_PERCENT);
        let mut changed = false;
        let mut controls_ui = parent.new_child(
            egui::UiBuilder::new()
                .max_rect(controls)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        controls_ui.set_clip_rect(view);

        egui::Frame::popup(parent.style())
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(6, 5))
            .show(&mut controls_ui, |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let zoom_out = ui
                    .add_sized(
                        [28.0, 28.0],
                        egui::Button::new(
                            egui::RichText::new(icon::MAGNIFYING_GLASS_MINUS).size(17.0),
                        ),
                    )
                    .on_hover_text("Zoom out");

                let slider_width =
                    (ui.available_width() - 28.0 - 48.0 - 3.0 * ui.spacing().item_spacing.x)
                        .max(60.0);
                let slider = ui
                    .add_sized(
                        [slider_width, 28.0],
                        egui::Slider::new(&mut requested_zoom, MIN_ZOOM_PERCENT..=MAX_ZOOM_PERCENT)
                            .logarithmic(true)
                            .show_value(false),
                    )
                    .on_hover_text("Zoom level (100% = last Fit)");

                let zoom_in = ui
                    .add_sized(
                        [28.0, 28.0],
                        egui::Button::new(
                            egui::RichText::new(icon::MAGNIFYING_GLASS_PLUS).size(17.0),
                        ),
                    )
                    .on_hover_text("Zoom in");

                if zoom_out.clicked() {
                    requested_zoom /= ZOOM_BUTTON_STEP;
                    changed = true;
                }
                if zoom_in.clicked() {
                    requested_zoom *= ZOOM_BUTTON_STEP;
                    changed = true;
                }
                changed |= slider.changed();
                ui.add_sized(
                    [48.0, 28.0],
                    egui::Label::new(format!("{requested_zoom:.0}%")),
                )
                .on_hover_text("Magnification relative to the last Fit command");
            });

        if changed {
            self.settle_animation();
            self.set_zoom_percent(requested_zoom);
            parent.ctx().request_repaint();
        }
    }

    fn show_section_controls(
        &mut self,
        parent: &mut egui::Ui,
        view: Rect,
        controls: Rect,
        bounds: Aabb,
    ) {
        let section = &mut self.section;
        let before = section.clone();
        control_bar(parent, view, controls, |ui| {
            ui.label(format!("{} Section", icon::SCISSORS));
            for axis in SectionAxis::ALL {
                if ui
                    .selectable_label(section.axis == axis, axis.label())
                    .on_hover_text(format!("Cut across the {} axis", axis.label()))
                    .clicked()
                    && section.axis != axis
                {
                    section.axis = axis;
                    section.offset = f32::NAN;
                    section.center_in(bounds);
                }
            }
            ui.toggle_value(&mut section.flipped, icon::ARROWS_DOWN_UP)
                .on_hover_text("Keep the other side");
            let range = section.range(bounds);
            ui.add_sized(
                [(ui.available_width() - 90.0).max(60.0), 24.0],
                egui::Slider::new(&mut section.offset, range)
                    .show_value(true)
                    .max_decimals(3)
                    .clamping(egui::SliderClamping::Never),
            )
            .on_hover_text("Position of the plane along the axis");
            ui.checkbox(&mut section.caps, "Fill")
                .on_hover_text("Fill the cut faces of solids");
            if ui
                .small_button(icon::X)
                .on_hover_text("Remove the section (View > Section View)")
                .clicked()
            {
                section.enabled = false;
            }
        });
        if self.section != before {
            parent.ctx().request_repaint();
        }
    }

    fn pick(
        &mut self,
        scene: &Scene,
        rect: Rect,
        pos: Pos2,
    ) -> Option<(NodeKey, Option<TopologySelection>)> {
        if !scene.preview_parts.is_empty() {
            return None;
        }
        self.pick_index.update(scene);
        let candidate_indices = self.pick_index.query(
            &self.camera,
            self.aspect,
            ndc(rect, pos),
            GVec2::new(rect.width(), rect.height()),
            self.selection_mode,
        );
        let candidates = candidate_indices
            .into_iter()
            .filter_map(|index| self.pick_index.candidate(scene, index));
        picking::pick_clipped(
            &self.camera,
            self.aspect,
            ndc(rect, pos),
            GVec2::new(rect.width(), rect.height()),
            self.selection_mode,
            candidates,
            self.section.clip_plane(),
        )
        .map(|hit| {
            let (node, part) = hit.id;
            let topology = hit.entity.map(|entity| TopologySelection {
                node: node.clone(),
                part,
                entity,
                position: hit.position,
            });
            (node, topology)
        })
    }

    /// Starts a drag in `mode` from `start`. A camera transition still under
    /// way ends first: an orbit then turns around what is under the pointer
    /// in the view the drag moves, not in a frame of the transition.
    fn start_drag(&mut self, scene: &Scene, rect: Rect, mode: DragMode, start: Pos2) {
        self.settle_animation();
        let pivot = (mode == DragMode::Orbit).then(|| self.orbit_pivot(scene, rect, start));
        self.drag = Some(Drag {
            mode,
            start,
            last: start,
            pivot,
        });
    }

    /// Choose the orbit point once per gesture. A hit stays under the cursor;
    /// empty space keeps the current view centre, including after panning.
    fn orbit_pivot(&mut self, scene: &Scene, rect: Rect, pos: Pos2) -> Vec3 {
        if scene.preview_parts.is_empty() {
            self.pick_index.update(scene);
            let pointer = ndc(rect, pos);
            let viewport = GVec2::new(rect.width(), rect.height());
            let candidates = self
                .pick_index
                .query(
                    &self.camera,
                    self.aspect,
                    pointer,
                    viewport,
                    SelectionMode::Object,
                )
                .into_iter()
                .filter_map(|index| self.pick_index.candidate(scene, index));
            if let Some(hit) = picking::pick_clipped(
                &self.camera,
                self.aspect,
                pointer,
                viewport,
                SelectionMode::Object,
                candidates,
                self.section.clip_plane(),
            ) {
                return hit.position;
            }
        }
        self.camera.target
    }

    /// What to draw; `simplify` draws parts that are small on screen with
    /// their simplified meshes (large-model mode), unless they are selected
    /// or hovered.
    fn draw_items(&mut self, input: &ViewInput<'_>, simplify: bool) -> Vec<DrawItem> {
        let (scene, settings, colors) = (input.scene, input.settings, input.colors);
        let default_color = srgb_unit(settings.default_color);
        let default_edge = linear_rgba(settings.edge_color, 1.0);
        let presentation = &input.rendering.presentation;
        let presenting = self.display_mode == DisplayMode::Presentation;
        // The axes and the ground grid, which Presentation leaves out.
        let helpers = !presenting || presentation.helpers;
        let mut items = Vec::new();

        // Selecting an assembly highlights all of its parts.
        let mut highlighted = std::collections::HashSet::new();
        for key in &scene.selection {
            if let Some(node) = scene.find(key) {
                collect_keys(node, &mut highlighted);
            }
        }

        let topology_nodes: std::collections::HashSet<_> = scene
            .topology_selections
            .iter()
            .map(|selection| selection.node.as_str())
            .collect();
        for node in scene
            .visible_nodes()
            .into_iter()
            .filter(|_| scene.preview_parts.is_empty())
        {
            let selected =
                highlighted.contains(&node.key) && !topology_nodes.contains(node.key.as_str());
            let hovered = self.hovered.as_deref() == Some(node.key.as_str());
            // Presentation gives objects without a material its own, and
            // that material's color to those without a color either.
            let own_finish = node.overrides.finish.or(node.finish);
            let (finish, fallback_color) = match own_finish {
                None if presenting => (
                    presentation.material,
                    presentation
                        .material
                        .natural_color()
                        .unwrap_or(default_color),
                ),
                _ => (node.finish(), default_color),
            };
            for part in &node.parts {
                let (rgb, alpha) = node.part_color(part, fallback_color);
                let has_faces = !part.mesh.indices.is_empty();
                let highlight = if selected {
                    with_alpha(colors.selection, 0.6)
                } else if hovered {
                    with_alpha(colors.hover, 0.25)
                } else {
                    [0.0; 4]
                };
                let edge_color = if selected {
                    linear_color(colors.selection, 1.0)
                } else if !has_faces {
                    // Wires and points have no faces to carry the color.
                    rgb_to_linear(rgb, alpha)
                } else {
                    node.edge_color
                        .map_or(default_edge, |c| rgb_to_linear(c, 1.0))
                };
                let look = PartLook {
                    face_color: rgb_to_linear(rgb, alpha),
                    edge_color,
                    highlight,
                    has_faces,
                    emphasized: selected || hovered,
                    edges: node.overrides.edges,
                    finish,
                };
                let transform = scene.part_transform(part);
                let mut mesh = if node.overrides.color.is_some() {
                    &part.mesh
                } else {
                    part.render_mesh.as_ref().unwrap_or(&part.mesh)
                };
                let mut style = self.display_mode.style(look, settings, input.rendering);
                if simplify && !selected && !hovered && part.render_mesh.is_none() {
                    let bounds = part.mesh.bounds.transformed(transform);
                    let size =
                        culling::projected_size(&self.camera, self.view_px[1] as f32, bounds);
                    let detail = Detail::for_size(size);
                    if detail != Detail::Full
                        && let Some(simplified) = part.mesh.simplified()
                    {
                        mesh = simplified;
                    }
                    if detail == Detail::Faint {
                        // Specks: their edges would only add noise.
                        style.edges &= !has_faces;
                        style.points = false;
                    }
                }
                items.push(DrawItem {
                    mesh: mesh.clone(),
                    transform,
                    style,
                    layer: Layer::Scene,
                });
            }
        }

        for part in &scene.preview_parts {
            let rgb = part.color.unwrap_or(default_color);
            let alpha = part.alpha.unwrap_or(1.0);
            let has_faces = !part.mesh.indices.is_empty();
            let look = PartLook {
                face_color: rgb_to_linear(rgb, alpha),
                edge_color: default_edge,
                highlight: [0.0; 4],
                has_faces,
                emphasized: false,
                edges: None,
                finish: if presenting {
                    presentation.material
                } else {
                    crate::settings::Finish::Default
                },
            };
            items.push(DrawItem {
                mesh: part.render_mesh.as_ref().unwrap_or(&part.mesh).clone(),
                transform: part.transform,
                style: self.display_mode.style(look, settings, input.rendering),
                layer: Layer::Scene,
            });
        }

        for (part, transform) in scene
            .placed_topology_highlights()
            .into_iter()
            .filter(|_| scene.preview_parts.is_empty())
        {
            let color = linear_color(colors.selection, 1.0);
            items.push(DrawItem {
                mesh: part.mesh.clone(),
                transform,
                style: Style {
                    face_color: linear_color(colors.selection, 0.55),
                    edge_color: color,
                    highlight: [0.0; 4],
                    edge_width: settings.edge_width + 2.0,
                    point_size: POINT_SIZE + 5.0,
                    faces: true,
                    edges: true,
                    points: true,
                    clipped: true,
                    background_fill: false,
                    hidden_edges: false,
                    edge_min_angle: 0.0,
                    silhouettes: false,
                    surface: [0.0; 4],
                },
                layer: Layer::Overlay,
            });
        }

        // Short positive origin axes give a consistent X/Y/Z reference at
        // every zoom. Where the model hides them they are dashed, so that a
        // part built around the origin never makes them disappear.
        let half_length = self.camera.extent * 0.45;
        if self
            .axis_meshes
            .as_ref()
            .is_none_or(|(len, _)| *len != half_length)
        {
            let line = |dir: Vec3| {
                Arc::new(Mesh::lines(vec![
                    Vec3::ZERO.to_array(),
                    (dir * half_length).to_array(),
                ]))
            };
            let [x, y, z] = &scene.axes;
            self.axis_meshes = Some((
                half_length,
                [line(x.direction), line(y.direction), line(z.direction)],
            ));
        }
        if let Some((_, meshes)) = &self.axis_meshes
            && helpers
        {
            for (axis, mesh) in scene.axes.iter().zip(meshes) {
                if axis.visible {
                    let mut item = line_item(
                        mesh.clone(),
                        rgb_to_linear(axis.color, 1.0),
                        1.5,
                        Layer::Scene,
                    );
                    item.style.hidden_edges = true;
                    items.push(item);
                }
            }
            if scene.axes.iter().any(|axis| axis.visible) {
                // A light dot in a dark ring, visible on any background.
                for (color, size) in [(colors.cube.outline, 9.0), (colors.overlay_text, 6.0)] {
                    let mut item = line_item(
                        self.origin_mesh.clone(),
                        linear_color(color, 1.0),
                        1.0,
                        Layer::Overlay,
                    );
                    item.style.edges = false;
                    item.style.points = true;
                    item.style.point_size = size;
                    items.push(item);
                }
            }
        }

        if let Some((ground, bands)) = &self.ground
            && helpers
        {
            let color = |alpha| linear_color(colors.grid, alpha);
            for ((minor, major), opacity) in bands.iter().zip(GRID_FADE) {
                items.push(line_item(
                    minor.clone(),
                    color(0.35 * ground.minor_opacity * opacity),
                    1.0,
                    Layer::Reference,
                ));
                items.push(line_item(
                    major.clone(),
                    color(0.7 * ground.major_opacity * opacity),
                    1.0,
                    Layer::Reference,
                ));
            }
        }

        if let Some(grid) = &scene.overlay.grid {
            let key = grid_key(grid);
            if self.grid_mesh.as_ref().is_none_or(|(k, _)| *k != key) {
                self.grid_mesh = Some((key, Arc::new(grid_mesh(grid))));
            }
            if let Some((_, mesh)) = &self.grid_mesh {
                items.push(line_item(
                    mesh.clone(),
                    linear_color(colors.grid, 0.8),
                    1.0,
                    Layer::Scene,
                ));
            }
        }

        for mesh in &scene.overlay.meshes {
            items.push(DrawItem {
                mesh: mesh.clone(),
                transform: GMat4::IDENTITY,
                style: Style {
                    face_color: linear_color(colors.overlay, 0.55),
                    edge_color: linear_color(colors.overlay, 1.0),
                    highlight: [0.0; 4],
                    edge_width: 3.0,
                    point_size: POINT_SIZE + 3.0,
                    faces: true,
                    edges: true,
                    points: true,
                    clipped: false,
                    background_fill: false,
                    hidden_edges: false,
                    edge_min_angle: 0.0,
                    silhouettes: false,
                    surface: [0.0; 4],
                },
                layer: Layer::Overlay,
            });
        }
        items
    }

    fn paint_overlays(&self, painter: &egui::Painter, rect: Rect, input: &ViewInput<'_>) {
        let colors = input.colors;
        if input.settings.show_navigation_cube {
            self.nav_cube
                .paint(painter, rect, self.camera.rotation, &colors.cube);
        }
        if input.settings.show_axes_indicator {
            overlays::paint_axes_indicator(
                painter,
                rect,
                self.camera.rotation,
                [
                    Color32::from_rgb(230, 60, 60),
                    Color32::from_rgb(90, 200, 60),
                    Color32::from_rgb(70, 120, 240),
                ],
                colors.overlay_text,
            );
        }

        if input.settings.projection == settings::Projection::Stereo
            && input.settings.stereo_mode.shown() != input.settings.stereo_mode
        {
            painter.text(
                Pos2::new(rect.center().x, rect.top() + 54.0),
                egui::Align2::CENTER_TOP,
                "Quad-buffered stereo cannot be shown here: showing a red-cyan anaglyph",
                egui::FontId::proportional(12.0),
                colors.overlay_text,
            );
        }

        let mut bottom = rect.bottom() - 10.0;
        let presenting = self.display_mode == DisplayMode::Presentation;
        if input.settings.scale_bar && (!presenting || input.rendering.presentation.helpers) {
            let units_per_point = ground::units_per_point(&self.camera, rect.height());
            let bar = ScaleBar::new(units_per_point, input.settings.units);
            let mut label = bar.label.clone();
            if let Some((grid, _)) = &self.ground {
                label += &format!(
                    " · grid {}",
                    ground::format_length(grid.minor, input.settings.units)
                );
            }
            let right = rect.right() - 14.0;
            let y = bottom - 4.0;
            let stroke = egui::Stroke::new(1.5, colors.overlay_text);
            let left = right - bar.width;
            painter.line_segment([Pos2::new(left, y), Pos2::new(right, y)], stroke);
            for x in [left, right] {
                painter.line_segment([Pos2::new(x, y - 5.0), Pos2::new(x, y)], stroke);
            }
            painter.text(
                Pos2::new(right, y - 7.0),
                egui::Align2::RIGHT_BOTTOM,
                label,
                egui::FontId::proportional(11.0),
                colors.overlay_text,
            );
            bottom = y - 24.0;
        }
        if self.simplifying {
            painter.text(
                Pos2::new(rect.right() - 10.0, bottom),
                egui::Align2::RIGHT_BOTTOM,
                "Large model: small parts simplified",
                egui::FontId::proportional(11.0),
                colors.overlay_text.gamma_multiply(0.7),
            );
        }

        if let Some(text) = input.busy {
            let galley = painter.layout_no_wrap(
                text.to_owned(),
                egui::FontId::proportional(13.0),
                colors.overlay_text,
            );
            let size = galley.size() + Vec2::new(44.0, 14.0);
            let pill = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 24.0), size);
            painter.rect_filled(pill, size.y / 2.0, colors.overlay_background);
            let spinner_center = Pos2::new(pill.left() + 18.0, pill.center().y);
            paint_spinner(
                painter,
                spinner_center,
                6.0,
                colors.overlay_text,
                painter.ctx().input(|i| i.time),
            );
            painter.galley(
                Pos2::new(pill.left() + 32.0, pill.center().y - galley.size().y / 2.0),
                galley,
                colors.overlay_text,
            );
            painter.ctx().request_repaint();
        } else if let Some(hint) = input.empty_hint.filter(|_| input.scene.is_empty()) {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                hint,
                egui::FontId::proportional(15.0),
                colors.overlay_text.gamma_multiply(0.75),
            );
        }
    }
}

fn requested_drag_mode(
    modifiers: egui::Modifiers,
    primary: bool,
    middle: bool,
    secondary: bool,
) -> Option<DragMode> {
    if middle || (primary && (modifiers.ctrl || modifiers.command)) {
        Some(DragMode::Pan)
    } else if primary && modifiers.shift {
        Some(DragMode::Zoom)
    } else if primary || secondary {
        Some(DragMode::Orbit)
    } else {
        None
    }
}

fn zoom_controls_rect(view: Rect) -> Option<Rect> {
    if view.width() < MIN_VIEW_WIDTH_FOR_ZOOM_CONTROLS
        || view.height() < ZOOM_CONTROLS_HEIGHT + 2.0 * ZOOM_CONTROLS_MARGIN
    {
        return None;
    }
    let size = Vec2::new(
        ZOOM_CONTROLS_WIDTH.min(view.width() - 2.0 * ZOOM_CONTROLS_MARGIN),
        ZOOM_CONTROLS_HEIGHT,
    );
    Some(Rect::from_min_size(
        view.left_top() + Vec2::splat(ZOOM_CONTROLS_MARGIN),
        size,
    ))
}

/// The `row`th control bar up from the bottom left of the view (section,
/// exploded view, isolation), unless the view is too small to hold it
/// below the zoom controls.
fn bottom_bar_rect(view: Rect, row: usize, width: f32) -> Option<Rect> {
    let rows = row as f32 + 1.0;
    if view.width() < MIN_VIEW_WIDTH_FOR_ZOOM_CONTROLS
        || view.height() < (rows + 1.0) * ZOOM_CONTROLS_HEIGHT + (rows + 2.0) * ZOOM_CONTROLS_MARGIN
    {
        return None;
    }
    let size = Vec2::new(
        width.min(view.width() - 2.0 * ZOOM_CONTROLS_MARGIN),
        ZOOM_CONTROLS_HEIGHT,
    );
    Some(Rect::from_min_size(
        Pos2::new(
            view.left() + ZOOM_CONTROLS_MARGIN,
            view.bottom() - rows * (ZOOM_CONTROLS_MARGIN + size.y),
        ),
        size,
    ))
}

/// A bar of controls floating over the view.
fn control_bar(
    parent: &mut egui::Ui,
    view: Rect,
    rect: Rect,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let mut bar = parent.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    bar.set_clip_rect(view);
    egui::Frame::popup(parent.style())
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(6, 5))
        .show(&mut bar, |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            add_contents(ui);
        });
}

fn show_explode_controls(
    parent: &mut egui::Ui,
    view: Rect,
    rect: Rect,
    scene: &Scene,
    events: &mut Vec<ViewerEvent>,
) {
    control_bar(parent, view, rect, |ui| {
        ui.label(format!("{} Explode", icon::ARROWS_OUT));
        let mut percent = scene.explode() * 100.0;
        let slider = ui
            .add_sized(
                [(ui.available_width() - 30.0).max(60.0), 24.0],
                egui::Slider::new(&mut percent, 0.0..=MAX_EXPLODE * 100.0)
                    .suffix("%")
                    .max_decimals(0),
            )
            .on_hover_text("How far the parts move away from the centre of the model");
        if slider.changed() {
            events.push(ViewerEvent::Command(Action::SetExplode(percent / 100.0)));
        }
        if ui
            .small_button(icon::X)
            .on_hover_text("Assemble the parts again (View > Exploded View)")
            .clicked()
        {
            events.push(ViewerEvent::Command(Action::ToggleExplode));
        }
    });
}

fn show_capture_controls(
    parent: &mut egui::Ui,
    view: Rect,
    rect: Rect,
    (written, frames): (u32, u32),
    events: &mut Vec<ViewerEvent>,
) {
    control_bar(parent, view, rect, |ui| {
        ui.label(format!("{} Turntable {written}/{frames}", icon::FILM_STRIP));
        ui.add(
            egui::ProgressBar::new(written as f32 / frames.max(1) as f32)
                .desired_width((ui.available_width() - 30.0).max(40.0)),
        );
        if ui
            .small_button(icon::X)
            .on_hover_text("Stop and delete what was written")
            .clicked()
        {
            events.push(ViewerEvent::Command(Action::CancelTurntable));
        }
    });
}

fn show_meshing_controls(
    parent: &mut egui::Ui,
    view: Rect,
    rect: Rect,
    (done, total): (u32, u32),
    events: &mut Vec<ViewerEvent>,
) {
    control_bar(parent, view, rect, |ui| {
        ui.label(format!("{} Meshing {done}/{total}", icon::POLYGON));
        ui.add(
            egui::ProgressBar::new(done as f32 / total.max(1) as f32)
                .desired_width((ui.available_width() - 30.0).max(40.0)),
        );
        if ui
            .small_button(icon::X)
            .on_hover_text("Stop meshing and keep the model shown now")
            .clicked()
        {
            events.push(ViewerEvent::Command(Action::CancelMeshing));
        }
    });
}

fn show_isolation_controls(
    parent: &mut egui::Ui,
    view: Rect,
    rect: Rect,
    scene: &Scene,
    events: &mut Vec<ViewerEvent>,
) {
    control_bar(parent, view, rect, |ui| {
        let count = scene.isolated_count();
        ui.label(format!(
            "{} Isolated: {count} object{}",
            icon::SELECTION_FOREGROUND,
            if count == 1 { "" } else { "s" }
        ));
        if ui
            .button("End")
            .on_hover_text("Show the other objects again (View > End Isolation)")
            .clicked()
        {
            events.push(ViewerEvent::Command(Action::ToggleIsolate));
        }
        if ui
            .button("Show All")
            .on_hover_text("End the isolation and show the hidden objects too")
            .clicked()
        {
            events.push(ViewerEvent::Command(Action::ShowAll));
        }
    });
}

fn collect_keys<'a>(
    node: &'a crate::scene::SceneNode,
    out: &mut std::collections::HashSet<&'a NodeKey>,
) {
    out.insert(&node.key);
    for child in &node.children {
        collect_keys(child, out);
    }
}

fn line_item(mesh: Arc<Mesh>, color: [f32; 4], width: f32, layer: Layer) -> DrawItem {
    DrawItem {
        mesh,
        transform: GMat4::IDENTITY,
        style: Style {
            face_color: color,
            edge_color: color,
            highlight: [0.0; 4],
            edge_width: width,
            point_size: POINT_SIZE,
            faces: false,
            edges: true,
            points: false,
            clipped: false,
            background_fill: false,
            hidden_edges: false,
            edge_min_angle: 0.0,
            silhouettes: false,
            surface: [0.0; 4],
        },
        layer,
    }
}

fn grid_key(grid: &Grid) -> GridKey {
    GridKey([
        grid.origin.x,
        grid.origin.y,
        grid.origin.z,
        grid.x_dir.x,
        grid.x_dir.y,
        grid.x_dir.z,
        grid.z_dir.x,
        grid.z_dir.y,
        grid.z_dir.z,
        grid.size,
    ])
}

/// A square grid on the plane, `size` wide on each side of the origin,
/// with 20 cells per side as in the original editor.
fn grid_mesh(grid: &Grid) -> Mesh {
    let x = grid.x_dir.normalize_or(Vec3::X);
    let y = grid.z_dir.normalize_or(Vec3::Z).cross(x);
    let size = grid.size.max(1e-3);
    let step = size / 20.0;
    let mut segments = Vec::new();
    for i in -20..=20 {
        let offset = i as f32 * step;
        for (a, b) in [
            (
                grid.origin + x * offset - y * size,
                grid.origin + x * offset + y * size,
            ),
            (
                grid.origin + y * offset - x * size,
                grid.origin + y * offset + x * size,
            ),
        ] {
            segments.push(a.to_array());
            segments.push(b.to_array());
        }
    }
    Mesh::lines(segments)
}

/// Normalized device coordinates of `pos` within `rect` (y up).
fn ndc(rect: Rect, pos: Pos2) -> GVec2 {
    GVec2::new(
        (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
        1.0 - (pos.y - rect.top()) / rect.height() * 2.0,
    )
}

fn trackball_coords(rect: Rect, pos: Pos2) -> GVec2 {
    let half = rect.width().min(rect.height()) / 2.0;
    let offset = pos - rect.center();
    GVec2::new(offset.x / half, -offset.y / half)
}

fn paint_spinner(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32, time: f64) {
    let start = (time * 5.0) as f32;
    let points: Vec<Pos2> = (0..=16)
        .map(|i| {
            let angle = start + i as f32 / 16.0 * std::f32::consts::PI * 1.4;
            center + Vec2::angled(angle) * radius
        })
        .collect();
    painter.add(egui::Shape::line(points, egui::Stroke::new(2.0, color)));
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_navigation.rs"]
mod tests;

/// A capture to make with the next frame of the view.
enum Capture {
    Image {
        path: Option<PathBuf>,
        options: CaptureOptions,
    },
    /// Written one frame per frame of the view.
    Turntable(Box<TurntableExport>),
}

/// The frame as captured: without the background when it is transparent
/// (premultiplied black, which `capture::save_png` straightens).
fn capture_input(frame: FrameInput<'_>, transparent: bool) -> FrameInput<'_> {
    if transparent {
        FrameInput {
            background_top: [0.0; 4],
            background_bottom: [0.0; 4],
            ..frame
        }
    } else {
        frame
    }
}

// -- colors ------------------------------------------------------------------

fn srgb_unit(color: [u8; 3]) -> [f32; 3] {
    color.map(|c| f32::from(c) / 255.0)
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn rgb_to_linear(rgb: [f32; 3], alpha: f32) -> [f32; 4] {
    [
        srgb_to_linear(rgb[0]),
        srgb_to_linear(rgb[1]),
        srgb_to_linear(rgb[2]),
        alpha,
    ]
}

fn linear_rgba(color: [u8; 3], alpha: f32) -> [f32; 4] {
    rgb_to_linear(srgb_unit(color), alpha)
}

fn linear_color(color: Color32, alpha: f32) -> [f32; 4] {
    linear_rgba([color.r(), color.g(), color.b()], alpha)
}

/// Highlight tint: linear RGB with the mix factor in alpha.
fn with_alpha(color: Color32, amount: f32) -> [f32; 4] {
    let [r, g, b, _] = linear_color(color, 1.0);
    [r, g, b, amount]
}
