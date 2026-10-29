//! User preferences, persisted by eframe between sessions.
//!
//! Defaults follow the original CQ-editor where practical. Compatibility-
//! relevant deviations are recorded in `docs/development/compatibility.md`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::worker::protocol::{Release, RunSettings, SceneSettings};

const MAX_RECENT_FILES: usize = 10;
/// Files whose breakpoints are remembered, most recently used first.
const MAX_BREAKPOINT_FILES: usize = 100;
/// Files whose named views are remembered, most recently used first.
const MAX_VIEW_FILES: usize = 100;
/// Project folders remembered, most recently opened first.
const MAX_PROJECTS: usize = 20;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub general: General,
    pub viewer: Viewer,
    pub rendering: Rendering,
    pub editor: Editor,
    pub objects: Objects,
    pub execution: Execution,
    pub recent_files: Vec<PathBuf>,
    /// File shown on startup: the active tab of the last session.
    pub last_file: Option<PathBuf>,
    /// Files open in tabs when the editor was last closed, in tab order.
    pub open_files: Vec<PathBuf>,
    /// The folder shown in the Project panel.
    pub project: Option<PathBuf>,
    /// Recently opened project folders and their state, most recent first.
    pub projects: Vec<ProjectState>,
    /// Breakpoints of recently edited files, most recent first.
    pub breakpoints: Vec<FileBreakpoints>,
    /// Named views of recently edited files, most recent first.
    pub views: Vec<FileViews>,
    /// Favorite commands pinned to the top of the Command Palette.
    pub favorite_commands: Vec<String>,
}

/// What is remembered of a project folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectState {
    pub root: PathBuf,
    /// The script F5 runs while a file of the project is shown; `None`
    /// once cleared, so it is not guessed again.
    pub main: Option<PathBuf>,
    /// Folders expanded in the Project panel.
    pub expanded: Vec<PathBuf>,
}

/// The named views of one file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FileViews {
    pub path: PathBuf,
    pub views: Vec<NamedView>,
}

/// A camera saved under a name: the point it looks at, its orientation,
/// how much it shows and its projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NamedView {
    pub name: String,
    pub target: [f32; 3],
    /// Unit quaternion (x, y, z, w).
    pub rotation: [f32; 4],
    /// Half the height of the view, in model units.
    pub extent: f32,
    pub projection: Projection,
}

/// The breakpoints of one file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FileBreakpoints {
    pub path: PathBuf,
    pub lines: Vec<SavedBreakpoint>,
}

/// A breakpoint as remembered between sessions: its 1-based line and the
/// trimmed text of that line, to find it again if the file changed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SavedBreakpoint {
    pub line: usize,
    pub text: String,
}

impl Settings {
    pub fn add_recent_file(&mut self, path: &Path) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_path_buf());
        self.recent_files.truncate(MAX_RECENT_FILES);
    }

    pub fn project_state(&self, root: &Path) -> Option<&ProjectState> {
        self.projects.iter().find(|project| project.root == root)
    }

    /// Remembers a project folder's state and moves it to the front of the
    /// recent folders.
    pub fn remember_project(&mut self, state: ProjectState) {
        self.projects.retain(|project| project.root != state.root);
        self.projects.insert(0, state);
        self.projects.truncate(MAX_PROJECTS);
    }

    /// Follows a file or folder renamed from `from` to `to`: what is
    /// remembered under the old path is kept under the new one.
    pub fn relocate(&mut self, from: &Path, to: &Path) {
        let moved = |path: &mut PathBuf| {
            if let Some(new) = crate::project::relocated(path, from, to) {
                *path = new;
            }
        };
        self.recent_files.iter_mut().for_each(moved);
        self.open_files.iter_mut().for_each(moved);
        self.last_file.iter_mut().for_each(moved);
        self.breakpoints
            .iter_mut()
            .for_each(|file| moved(&mut file.path));
        self.views.iter_mut().for_each(|file| moved(&mut file.path));
        for project in &mut self.projects {
            project.main.iter_mut().for_each(moved);
            project.expanded.iter_mut().for_each(moved);
        }
    }

    pub fn breakpoints_of(&self, path: &Path) -> &[SavedBreakpoint] {
        self.breakpoints
            .iter()
            .find(|file| file.path == path)
            .map_or(&[], |file| &file.lines)
    }

    /// Remembers the breakpoints of `path`; none forgets the file.
    pub fn remember_breakpoints(&mut self, path: &Path, lines: Vec<SavedBreakpoint>) {
        if self.breakpoints_of(path) == lines.as_slice()
            && self
                .breakpoints
                .first()
                .is_some_and(|file| file.path == path)
        {
            return;
        }
        self.breakpoints.retain(|file| file.path != path);
        if !lines.is_empty() {
            self.breakpoints.insert(
                0,
                FileBreakpoints {
                    path: path.to_path_buf(),
                    lines,
                },
            );
            self.breakpoints.truncate(MAX_BREAKPOINT_FILES);
        }
    }

    pub fn views_of(&self, path: &Path) -> &[NamedView] {
        self.views
            .iter()
            .find(|file| file.path == path)
            .map_or(&[], |file| &file.views)
    }

    /// Remembers the named views of `path`; none forgets the file.
    pub fn remember_views(&mut self, path: &Path, views: Vec<NamedView>) {
        if self.views_of(path) == views.as_slice()
            && self.views.first().is_some_and(|file| file.path == path)
        {
            return;
        }
        self.views.retain(|file| file.path != path);
        if !views.is_empty() {
            self.views.insert(
                0,
                FileViews {
                    path: path.to_path_buf(),
                    views,
                },
            );
            self.views.truncate(MAX_VIEW_FILES);
        }
    }

    pub fn run_settings(&self) -> RunSettings {
        RunSettings {
            reload_cq: self.execution.reload_cq,
            add_script_dir_to_path: self.execution.add_script_dir_to_path,
            change_working_dir: self.execution.change_working_dir,
            reload_imported_modules: self.execution.reload_imported_modules,
        }
    }

    pub fn scene_settings(&self) -> SceneSettings {
        SceneSettings {
            linear_tolerance: self.viewer.linear_tolerance,
            angular_tolerance: self.viewer.angular_tolerance,
            merge_assemblies: self.objects.merge_assemblies,
            units: self.viewer.units.export_code().to_owned(),
        }
    }

    pub fn is_favorite_command(&self, id: &str) -> bool {
        self.favorite_commands.iter().any(|fav| fav == id)
    }

    /// Toggles a command in `favorite_commands`; returns `true` when it is now a favorite.
    pub fn toggle_favorite_command(&mut self, id: &str) -> bool {
        if let Some(pos) = self.favorite_commands.iter().position(|fav| fav == id) {
            self.favorite_commands.remove(pos);
            false
        } else {
            self.favorite_commands.push(id.to_owned());
            true
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Theme {
    /// Follow the operating system.
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub theme: Theme,
    /// Python interpreter with CadQuery; empty means auto-detect.
    pub python: String,
    /// Scale factor on top of the system's.
    pub ui_scale: f32,
    /// Look for a newer CQ-editor once a day, when Python starts.
    pub check_updates: bool,
    /// When it last looked, in seconds since the Unix epoch.
    pub last_update_check: u64,
    /// The newest release it found then (see `updates::available`).
    pub newer_release: Option<Release>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            python: String::new(),
            ui_scale: 1.0,
            check_updates: true,
            last_update_check: 0,
            newer_release: None,
        }
    }
}

/// The original editor's projection types, under the same names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Projection {
    #[default]
    Orthographic,
    Perspective,
    /// Perspective seen by two eyes, composed as [`Viewer::stereo_mode`]
    /// says.
    Stereo,
    /// Perspective from the left eye of the stereo pair alone.
    MonoLeftEye,
    MonoRightEye,
}

impl Projection {
    pub const ALL: [Projection; 5] = [
        Projection::Orthographic,
        Projection::Perspective,
        Projection::Stereo,
        Projection::MonoLeftEye,
        Projection::MonoRightEye,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Projection::Orthographic => "Orthographic",
            Projection::Perspective => "Perspective",
            Projection::Stereo => "Stereo",
            Projection::MonoLeftEye => "Mono Left Eye",
            Projection::MonoRightEye => "Mono Right Eye",
        }
    }

    /// All but orthographic projection have a vanishing point.
    pub fn is_perspective(self) -> bool {
        self != Projection::Orthographic
    }
}

/// How the two images of the stereo projection are combined, under the
/// original editor's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StereoMode {
    /// Hardware stereo (one buffer per eye), which this renderer cannot
    /// present: shown as [`StereoMode::Anaglyph`] instead.
    QuadBuffer,
    /// Red-cyan glasses.
    #[default]
    Anaglyph,
    /// Even rows for the left eye, odd ones for the right (passive 3D
    /// displays).
    RowInterlaced,
    ColumnInterlaced,
    ChessBoard,
    /// The left eye's image on the left half, the right eye's on the right.
    SideBySide,
    /// The left eye's image on the top half.
    OverUnder,
}

impl StereoMode {
    pub const ALL: [StereoMode; 7] = [
        StereoMode::QuadBuffer,
        StereoMode::Anaglyph,
        StereoMode::RowInterlaced,
        StereoMode::ColumnInterlaced,
        StereoMode::ChessBoard,
        StereoMode::SideBySide,
        StereoMode::OverUnder,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StereoMode::QuadBuffer => "Quad Buffer",
            StereoMode::Anaglyph => "Anaglyph (red-cyan)",
            StereoMode::RowInterlaced => "Row Interlaced",
            StereoMode::ColumnInterlaced => "Column Interlaced",
            StereoMode::ChessBoard => "Chess Board",
            StereoMode::SideBySide => "Side by Side",
            StereoMode::OverUnder => "Over Under",
        }
    }

    /// The mode actually shown: quad-buffered stereo needs a stereo
    /// swap chain, which wgpu does not offer on any platform.
    pub fn shown(self) -> StereoMode {
        match self {
            StereoMode::QuadBuffer => StereoMode::Anaglyph,
            mode => mode,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OrbitMethod {
    /// Rotation about the world Z axis and the screen's horizontal axis;
    /// Z stays up.
    #[default]
    Turntable,
    /// Free rotation, like rolling a ball under the mouse.
    Trackball,
}

impl OrbitMethod {
    pub const ALL: [OrbitMethod; 2] = [OrbitMethod::Turntable, OrbitMethod::Trackball];

    pub fn label(self) -> &'static str {
        match self {
            OrbitMethod::Turntable => "Turntable",
            OrbitMethod::Trackball => "Trackball",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Viewer {
    pub fit_automatically: bool,
    pub show_navigation_cube: bool,
    pub show_axes_indicator: bool,
    pub use_gradient: bool,
    /// sRGB.
    pub background: [u8; 3],
    /// Bottom color of the gradient, sRGB.
    pub background_aux: [u8; 3],
    pub default_color: [u8; 3],
    pub edge_color: [u8; 3],
    pub show_edges: bool,
    /// Hidden edges as faint dashes in the hidden-line display mode.
    pub hidden_edges: bool,
    pub edge_width: f32,
    /// Chordal deviation of the mesh relative to the object size.
    pub linear_tolerance: f64,
    /// Radians.
    pub angular_tolerance: f64,
    pub projection: Projection,
    pub orbit_method: OrbitMethod,
    /// Animate view changes (standard views, navigation cube, fit).
    pub animate: bool,
    /// When parts drawn small are simplified.
    pub large_models: LargeModels,
    /// The grid on the XY plane.
    pub ground_grid: bool,
    pub scale_bar: bool,
    /// What a model unit is called in the scale bar.
    pub units: Units,
    /// How the stereo projection shows its two images.
    pub stereo_mode: StereoMode,
    /// Configurable keyboard shortcuts for switching 3D selection modes.
    pub selection_shortcuts: SelectionShortcuts,
}

/// Configurable keyboard shortcuts for switching between 3D selection modes
/// when no text field has keyboard focus (or anytime if combined with Ctrl/Alt).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectionShortcuts {
    pub auto: String,
    pub object: String,
    pub face: String,
    pub edge: String,
    pub vertex: String,
}

impl Default for SelectionShortcuts {
    fn default() -> Self {
        Self {
            auto: "1".to_owned(),
            object: "2".to_owned(),
            face: "3".to_owned(),
            edge: "4".to_owned(),
            vertex: "5".to_owned(),
        }
    }
}

impl SelectionShortcuts {
    pub fn get(&self, mode: crate::viewer::SelectionMode) -> &str {
        use crate::viewer::SelectionMode;
        match mode {
            SelectionMode::Auto => &self.auto,
            SelectionMode::Object => &self.object,
            SelectionMode::Face => &self.face,
            SelectionMode::Edge => &self.edge,
            SelectionMode::Vertex => &self.vertex,
        }
    }

    pub fn get_mut(&mut self, mode: crate::viewer::SelectionMode) -> &mut String {
        use crate::viewer::SelectionMode;
        match mode {
            SelectionMode::Auto => &mut self.auto,
            SelectionMode::Object => &mut self.object,
            SelectionMode::Face => &mut self.face,
            SelectionMode::Edge => &mut self.edge,
            SelectionMode::Vertex => &mut self.vertex,
        }
    }
}

/// What a model unit is: CadQuery has no units, so the editor supplies this
/// interpretation to exports and measurement labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Units {
    #[default]
    Millimeters,
    Centimeters,
    Meters,
    Inches,
    /// Plain numbers.
    None,
}

impl Units {
    pub const ALL: [Units; 5] = [
        Units::Millimeters,
        Units::Centimeters,
        Units::Meters,
        Units::Inches,
        Units::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Units::Millimeters => "Millimeters",
            Units::Centimeters => "Centimeters",
            Units::Meters => "Meters",
            Units::Inches => "Inches",
            Units::None => "None",
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Units::Millimeters => "mm",
            Units::Centimeters => "cm",
            Units::Meters => "m",
            Units::Inches => "in",
            Units::None => "",
        }
    }

    pub fn export_code(self) -> &'static str {
        match self {
            Units::Millimeters => "millimeter",
            Units::Centimeters => "centimeter",
            Units::Meters => "meter",
            Units::Inches => "inch",
            Units::None => "none",
        }
    }
}

/// When the view simplifies the parts it draws small (large-model mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LargeModels {
    /// From a couple of million triangles shown.
    #[default]
    Automatic,
    Always,
    Off,
}

impl LargeModels {
    pub const ALL: [LargeModels; 3] = [
        LargeModels::Automatic,
        LargeModels::Always,
        LargeModels::Off,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LargeModels::Automatic => "Automatic",
            LargeModels::Always => "Always",
            LargeModels::Off => "Off",
        }
    }
}

impl Default for Viewer {
    fn default() -> Self {
        Self {
            fit_automatically: true,
            show_navigation_cube: true,
            show_axes_indicator: true,
            use_gradient: false,
            background: [95, 95, 95],
            background_aux: [30, 30, 30],
            default_color: [255, 215, 0],
            edge_color: [0, 0, 0],
            show_edges: true,
            hidden_edges: true,
            edge_width: 1.5,
            linear_tolerance: 1e-3,
            angular_tolerance: 0.2,
            projection: Projection::Orthographic,
            orbit_method: OrbitMethod::Turntable,
            animate: true,
            large_models: LargeModels::Automatic,
            ground_grid: true,
            scale_bar: true,
            units: Units::Millimeters,
            stereo_mode: StereoMode::Anaglyph,
            selection_shortcuts: SelectionShortcuts::default(),
        }
    }
}

/// How the 3D view lights the faces it draws, and which of their edges it
/// draws.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rendering {
    pub lighting: Lighting,
    /// Which edges the Shaded, X-Ray and Hidden Line modes draw.
    pub edges: EdgeStyle,
    /// Edges whose faces meet at less than this many degrees are smooth:
    /// the Sharp and Outline styles leave them out.
    pub sharp_angle: f32,
    /// The Presentation display mode's own settings.
    pub presentation: Presentation,
}

impl Default for Rendering {
    fn default() -> Self {
        Self {
            lighting: Lighting::default(),
            edges: EdgeStyle::All,
            sharp_angle: 30.0,
            presentation: Presentation::default(),
        }
    }
}

/// The Presentation display mode, for showing a model rather than working
/// on it: lights, edges, a material for objects without one, a background
/// and helpers of its own, so switching back to Shaded restores the
/// modeling view untouched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Presentation {
    pub lighting: Lighting,
    /// Edges are drawn at all; off, the shapes speak alone.
    pub show_edges: bool,
    /// Which edges, when they are drawn.
    pub edges: EdgeStyle,
    /// The material of objects whose script and Objects panel choose none.
    pub material: Finish,
    /// The background, a vertical gradient from top to bottom (sRGB).
    pub background_top: [u8; 3],
    pub background_bottom: [u8; 3],
    /// The axes and the ground grid are drawn.
    pub helpers: bool,
    /// A soft shadow on the ground under the model.
    pub shadow: bool,
    /// How dark the shadow is where the model touches the ground, 0 to 1.
    pub shadow_strength: f32,
    /// How far the shadow's edge spreads, 0 (crisp) to 1 (very soft).
    pub shadow_softness: f32,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            lighting: LightingPreset::Studio.lighting(),
            show_edges: false,
            edges: EdgeStyle::Outline,
            material: Finish::MattePlastic,
            background_top: [236, 237, 240],
            background_bottom: [188, 191, 198],
            helpers: false,
            shadow: true,
            shadow_strength: 0.6,
            shadow_softness: 0.5,
        }
    }
}

/// The look of a surface: the plain shading of a CAD view, or a material
/// shaded by its metalness and roughness. Scripts choose one with
/// `show_object(..., material="matte plastic")`, the Objects panel per
/// object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Finish {
    /// The shading of earlier releases, with the lighting's highlights.
    #[default]
    Default,
    MattePlastic,
    GlossyPlastic,
    BrushedSteel,
    Aluminum,
    /// Plain wood with a procedural grain around the part's Z axis.
    Wood,
}

impl Finish {
    pub const ALL: [Finish; 6] = [
        Finish::Default,
        Finish::MattePlastic,
        Finish::GlossyPlastic,
        Finish::BrushedSteel,
        Finish::Aluminum,
        Finish::Wood,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Finish::Default => "Default",
            Finish::MattePlastic => "Matte plastic",
            Finish::GlossyPlastic => "Glossy plastic",
            Finish::BrushedSteel => "Brushed steel",
            Finish::Aluminum => "Aluminum",
            Finish::Wood => "Wood",
        }
    }

    /// The name the worker sends for `show_object`'s `material` option.
    pub fn name(self) -> &'static str {
        match self {
            Finish::Default => "default",
            Finish::MattePlastic => "matte_plastic",
            Finish::GlossyPlastic => "glossy_plastic",
            Finish::BrushedSteel => "brushed_steel",
            Finish::Aluminum => "aluminum",
            Finish::Wood => "wood",
        }
    }

    pub fn from_name(name: &str) -> Option<Finish> {
        Finish::ALL.into_iter().find(|finish| finish.name() == name)
    }

    /// How the shader draws it: model (0 plain, 1 physically based, 2 the
    /// same with a wood grain), metalness and roughness.
    pub fn surface(self) -> [f32; 4] {
        let (model, metallic, roughness) = match self {
            Finish::Default => (0.0, 0.0, 0.0),
            Finish::MattePlastic => (1.0, 0.0, 0.75),
            Finish::GlossyPlastic => (1.0, 0.0, 0.2),
            Finish::BrushedSteel => (1.0, 1.0, 0.55),
            Finish::Aluminum => (1.0, 1.0, 0.3),
            Finish::Wood => (2.0, 0.0, 0.65),
        };
        [model, metallic, roughness, 0.0]
    }

    /// The color of the material itself (sRGB), for objects without a
    /// color of their own; plastics take the default object color.
    pub fn natural_color(self) -> Option<[f32; 3]> {
        match self {
            Finish::BrushedSteel => Some([0.62, 0.63, 0.64]),
            Finish::Aluminum => Some([0.91, 0.92, 0.93]),
            Finish::Wood => Some([0.72, 0.52, 0.32]),
            Finish::Default | Finish::MattePlastic | Finish::GlossyPlastic => None,
        }
    }
}

/// Which edges of a model are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EdgeStyle {
    /// Every edge of every face, as in earlier releases.
    #[default]
    All,
    /// The edges where faces meet at an angle, and the borders of open
    /// faces: not those between faces that continue smoothly (a fillet's
    /// borders, seams of cylinders), which clutter curved and fluted
    /// models.
    Sharp,
    /// The sharp edges plus the outline of curved surfaces seen from the
    /// camera, as a drawing has them.
    Outline,
}

impl EdgeStyle {
    pub const ALL: [EdgeStyle; 3] = [EdgeStyle::All, EdgeStyle::Sharp, EdgeStyle::Outline];

    pub fn label(self) -> &'static str {
        match self {
            EdgeStyle::All => "All edges",
            EdgeStyle::Sharp => "Sharp edges",
            EdgeStyle::Outline => "Outline",
        }
    }

    /// The smallest angle (degrees) an edge must have to be drawn, with a
    /// sharp angle of `sharp`.
    pub fn minimum_angle(self, sharp: f32) -> f32 {
        match self {
            EdgeStyle::All => 0.0,
            EdgeStyle::Sharp | EdgeStyle::Outline => sharp.clamp(0.0, 179.0),
        }
    }
}

/// The lights of the 3D view. They are attached to the camera, as a
/// photographer's lights are to the subject: the model is lit the same way
/// from every side it is seen from. The defaults are the look of earlier
/// releases.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lighting {
    /// The main light, from the upper left.
    pub key: f32,
    /// A softer light from the lower right, which keeps shadows readable.
    pub fill: f32,
    /// Light from everywhere: the darkest a face can be.
    pub ambient: f32,
    /// Light grazing the outline of curved surfaces.
    pub rim: f32,
    /// Strength of the highlights of the key light.
    pub specular: f32,
    /// Sharpness of the highlights: higher is glossier.
    pub shininess: f32,
    /// Where the key light comes from, in degrees: around the line of
    /// sight (negative is left) and above it.
    pub azimuth: f32,
    pub elevation: f32,
    /// Brightness in photographic stops: +1 doubles the light.
    pub exposure: f32,
    /// 1 leaves the lit colors as they are; above 1 darkens the shadows and
    /// brightens the lights around middle grey.
    pub contrast: f32,
}

impl Default for Lighting {
    fn default() -> Self {
        LightingPreset::Default.lighting()
    }
}

impl Lighting {
    /// Direction of the key light in the camera's frame: right, up, and
    /// towards the viewer.
    pub fn key_direction(&self) -> [f32; 3] {
        let (azimuth, elevation) = (self.azimuth.to_radians(), self.elevation.to_radians());
        [
            elevation.cos() * azimuth.sin(),
            elevation.sin(),
            elevation.cos() * azimuth.cos(),
        ]
    }

    /// The preset these values are, if any.
    pub fn preset(&self) -> Option<LightingPreset> {
        LightingPreset::ALL
            .into_iter()
            .find(|preset| preset.lighting() == *self)
    }
}

/// Starting points for [`Lighting`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightingPreset {
    /// The look of earlier releases: crisp, for modeling.
    Default,
    /// Soft and even, with gentle highlights, for showing a model.
    Studio,
    /// Lighter overall, for dark colors.
    Bright,
    /// One strong light from the side, deep shadows, sharp highlights.
    Dramatic,
}

impl LightingPreset {
    pub const ALL: [LightingPreset; 4] = [
        LightingPreset::Default,
        LightingPreset::Studio,
        LightingPreset::Bright,
        LightingPreset::Dramatic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LightingPreset::Default => "Default",
            LightingPreset::Studio => "Studio",
            LightingPreset::Bright => "Bright",
            LightingPreset::Dramatic => "Dramatic",
        }
    }

    pub fn lighting(self) -> Lighting {
        let (key, fill, ambient, rim, specular, shininess) = match self {
            LightingPreset::Default => (0.62, 0.24, 0.24, 0.10, 0.22, 48.0),
            LightingPreset::Studio => (0.52, 0.34, 0.30, 0.16, 0.28, 64.0),
            LightingPreset::Bright => (0.70, 0.36, 0.36, 0.10, 0.20, 32.0),
            LightingPreset::Dramatic => (0.95, 0.08, 0.10, 0.24, 0.45, 96.0),
        };
        let (azimuth, elevation, exposure, contrast) = match self {
            // The key light of earlier releases: (-0.35, 0.55, 0.76).
            LightingPreset::Default => (-24.7, 33.3, 0.0, 1.0),
            LightingPreset::Studio => (-35.0, 40.0, 0.15, 1.05),
            LightingPreset::Bright => (-25.0, 35.0, 0.35, 0.95),
            LightingPreset::Dramatic => (-60.0, 25.0, 0.0, 1.3),
        };
        Lighting {
            key,
            fill,
            ambient,
            rim,
            specular,
            shininess,
            azimuth,
            elevation,
            exposure,
            contrast,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Editor {
    pub font_size: f32,
    /// Re-run the script whenever its file changes on disk.
    pub autoreload: bool,
    /// Wait this long after a change, so the file is completely written.
    pub autoreload_delay_ms: u64,
    pub watch_imported_modules: bool,
    pub line_wrap: bool,
    /// Column of the vertical ruler; 0 hides it.
    pub max_line_length: usize,
    pub highlight_current_line: bool,
    pub auto_indent: bool,
    /// Offer completions after this many characters of a name; 0 offers
    /// them only after `.` and on Ctrl+Space.
    pub completion_min_chars: usize,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            autoreload: false,
            autoreload_delay_ms: 50,
            watch_imported_modules: false,
            line_wrap: false,
            max_line_length: 79,
            highlight_current_line: true,
            auto_indent: true,
            completion_min_chars: 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Objects {
    /// Keep visibility and colors of objects across runs (matched by name).
    pub preserve_properties: bool,
    pub clear_before_run: bool,
    /// Show assemblies as one object instead of one per part.
    pub merge_assemblies: bool,
    /// STL export chordal tolerance, model units.
    pub stl_tolerance: f64,
    /// STL export angular tolerance, radians.
    pub stl_angular_tolerance: f64,
}

impl Default for Objects {
    fn default() -> Self {
        Self {
            preserve_properties: false,
            clear_before_run: true,
            merge_assemblies: false,
            stl_tolerance: 0.1,
            stl_angular_tolerance: 0.1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Execution {
    pub reload_cq: bool,
    pub add_script_dir_to_path: bool,
    pub change_working_dir: bool,
    pub reload_imported_modules: bool,
}

impl Default for Execution {
    fn default() -> Self {
        Self {
            reload_cq: false,
            add_script_dir_to_path: true,
            change_working_dir: true,
            reload_imported_modules: true,
        }
    }
}

#[cfg(test)]
#[path = "../tests/rust/app/test_settings.rs"]
mod tests;
