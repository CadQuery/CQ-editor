//! Message types exchanged with the Python worker (`worker/cq_worker`).
//!
//! Every message is one JSON object per line. Requests carry a `cmd` tag,
//! worker messages a `type` tag. Mesh arrays travel as base64-encoded
//! little-endian binary and are decoded on the reader thread, off the UI
//! thread.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use serde::{Deserialize, Deserializer, Serialize};

/// The worker's protocol version this build understands.
pub const PROTOCOL_VERSION: u32 = 13;

pub type RequestId = u64;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NavigationMode {
    Definition,
    References,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OutlineSymbol {
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub depth: usize,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NavigationLocation {
    pub name: String,
    pub kind: String,
    pub path: Option<String>,
    pub line: usize,
    pub column: usize,
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Run {
        id: RequestId,
        script: String,
        path: Option<String>,
        debug: bool,
        breakpoints: Vec<usize>,
        project_root: Option<String>,
        module_breakpoints: BTreeMap<String, Vec<usize>>,
        settings: RunSettings,
        scene: SceneSettings,
        /// Content-addressed meshes the GUI holds and pins for this request.
        /// The worker may answer with lightweight references to these ids.
        known_geometry: Vec<String>,
        /// Measure the time of each line (not with `debug`).
        profile: bool,
    },
    /// Only valid while a debugging session is paused.
    Debug {
        action: DebugAction,
        breakpoints: Vec<usize>,
        module_breakpoints: BTreeMap<String, Vec<usize>>,
    },
    /// Scene keys the GUI no longer shows; lets the worker free them.
    Release {
        keys: Vec<String>,
    },
    Complete {
        id: RequestId,
        source: String,
        /// 1-based.
        line: usize,
        /// 0-based, in characters.
        column: usize,
        path: Option<String>,
        target: CompletionTarget,
    },
    /// Documentation of the name at a position of the editor's script.
    Describe {
        id: RequestId,
        source: String,
        /// 1-based.
        line: usize,
        /// 0-based, in characters.
        column: usize,
        path: Option<String>,
    },
    Outline {
        id: RequestId,
        source: String,
    },
    Navigate {
        id: RequestId,
        source: String,
        line: usize,
        column: usize,
        path: Option<String>,
        project_root: Option<String>,
        mode: NavigationMode,
    },
    Inspect {
        id: RequestId,
        key: String,
    },
    InspectGeometry {
        id: RequestId,
        key: String,
        frame: usize,
        item: Option<usize>,
        scene: SceneSettings,
    },
    InterferenceGeometry {
        id: RequestId,
        first: InterferencePart,
        second: InterferencePart,
        scene: SceneSettings,
    },
    /// Measures one entity, or the exact relationship between two entities,
    /// against the live OCCT shapes retained by the worker.
    Measure {
        id: RequestId,
        selections: Vec<TopologyReference>,
        /// Measured instead when no entity is selected.
        objects: Vec<String>,
    },
    /// Python that repeats a measurement with the script's variables.
    MeasurementCode {
        id: RequestId,
        selections: Vec<TopologyReference>,
        objects: Vec<String>,
    },
    /// CadQuery selectors that select exactly these entities of one object.
    Selector {
        id: RequestId,
        selections: Vec<TopologyReference>,
    },
    /// Read-only preview of a literal selector at the editor caret.
    SelectorPreview {
        id: RequestId,
        run_id: RequestId,
        line: usize,
        column: usize,
        preferred_key: Option<String>,
    },
    Console {
        id: RequestId,
        source: String,
        scene: SceneSettings,
        known_geometry: Vec<String>,
        /// What `selected()` returns while this command runs.
        selection: ConsoleSelection,
    },
    ConsoleReset {
        id: RequestId,
    },
    Export {
        id: RequestId,
        keys: Vec<String>,
        format: ExportFormat,
        path: String,
        tolerance: f64,
        angular_tolerance: f64,
        step_mode: String,
        step_schema: String,
        stl_ascii: bool,
        flat_parts: bool,
        unit: String,
        on_invalid: String,
        on_interference: String,
    },
    Check {
        id: RequestId,
        keys: Vec<String>,
        flat_parts: bool,
    },
    Upload {
        id: RequestId,
        url: String,
        file_type: String,
        keys: Vec<String>,
        source: String,
    },
    CheckUpdates {
        id: RequestId,
        /// The running CQ-editor's version: its newer releases are listed.
        editor: String,
        /// Also list newer CadQuery releases.
        cadquery: bool,
    },
    Imports {
        id: RequestId,
        path: String,
    },
    /// Stops the meshing of the run `target` (the worker reads it while
    /// busy); the run then finishes as `cancelled`.
    Cancel {
        target: RequestId,
    },
    /// Raises `KeyboardInterrupt` in the run or console command `target`
    /// (the worker reads it while busy), as Ctrl+C would.
    Interrupt {
        target: RequestId,
    },
    Shutdown,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DebugAction {
    Continue,
    Step,
    StepIn,
    StepOut,
    Stop,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompletionTarget {
    Editor,
    Console,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Step,
    Stl,
    Brep,
    #[serde(rename = "3mf")]
    ThreeMf,
    Gltf,
    Glb,
    Amf,
    Vrml,
    Vtkjs,
    Xbf,
    Svg,
    Dxf,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 12] = [
        ExportFormat::Step,
        ExportFormat::Stl,
        ExportFormat::Brep,
        ExportFormat::ThreeMf,
        ExportFormat::Gltf,
        ExportFormat::Glb,
        ExportFormat::Amf,
        ExportFormat::Vrml,
        ExportFormat::Vtkjs,
        ExportFormat::Xbf,
        ExportFormat::Svg,
        ExportFormat::Dxf,
    ];

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Step => "step",
            ExportFormat::Stl => "stl",
            ExportFormat::Brep => "brep",
            ExportFormat::ThreeMf => "3mf",
            ExportFormat::Gltf => "gltf",
            ExportFormat::Glb => "glb",
            ExportFormat::Amf => "amf",
            ExportFormat::Vrml => "vrml",
            ExportFormat::Vtkjs => "vtkjs",
            ExportFormat::Xbf => "xbf",
            ExportFormat::Svg => "svg",
            ExportFormat::Dxf => "dxf",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Step => "STEP",
            ExportFormat::Stl => "STL",
            ExportFormat::Brep => "BREP",
            ExportFormat::ThreeMf => "3MF",
            ExportFormat::Gltf => "glTF",
            ExportFormat::Glb => "GLB",
            ExportFormat::Amf => "AMF",
            ExportFormat::Vrml => "VRML",
            ExportFormat::Vtkjs => "VTK.js",
            ExportFormat::Xbf => "XBF",
            ExportFormat::Svg => "SVG",
            ExportFormat::Dxf => "DXF",
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct RunSettings {
    pub reload_cq: bool,
    pub add_script_dir_to_path: bool,
    pub change_working_dir: bool,
    pub reload_imported_modules: bool,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SceneSettings {
    pub linear_tolerance: f64,
    pub angular_tolerance: f64,
    pub merge_assemblies: bool,
    pub units: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GeometryReport {
    pub key: String,
    pub name: String,
    pub valid: bool,
    pub solids: usize,
    pub open_shells: usize,
    pub non_manifold_edges: usize,
    pub issues: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InterferencePart {
    pub key: String,
    pub name: String,
    pub part: usize,
    pub solid: usize,
}

#[derive(Deserialize, Debug, Clone)]
pub struct InterferenceReport {
    pub first: InterferencePart,
    pub second: InterferencePart,
    pub kind: String,
    pub overlap_volume: Option<f64>,
    pub distance: Option<f64>,
    pub point: Option<[f64; 3]>,
    pub reason: Option<String>,
    #[serde(default)]
    pub allowance_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Ready(ReadyInfo),
    /// The worker could not start (typically: CadQuery is not installed).
    Fatal {
        message: String,
        #[serde(default)]
        detail: Option<String>,
    },
    Output {
        id: Option<RequestId>,
        stream: OutputStream,
        text: String,
    },
    Log {
        level: String,
        source: String,
        message: String,
    },
    RunFinished(Box<RunFinished>),
    /// One mesh and its placement, sent before the final scene hierarchy.
    RunPart {
        id: RequestId,
        name: String,
        part: Box<Part>,
    },
    /// How far a request is, e.g. parts meshed of all.
    Progress {
        id: RequestId,
        stage: String,
        done: u32,
        total: u32,
    },
    DebugPaused {
        id: RequestId,
        line: usize,
        path: Option<String>,
        variables: Vec<Variable>,
        objects: Vec<SceneNode>,
    },
    Completions {
        id: RequestId,
        available: bool,
        items: Vec<CompletionItem>,
        signatures: Vec<String>,
    },
    Description {
        id: RequestId,
        #[serde(flatten)]
        documentation: Documentation,
    },
    Outline {
        id: RequestId,
        symbols: Vec<OutlineSymbol>,
    },
    Navigation {
        id: RequestId,
        available: bool,
        mode: NavigationMode,
        locations: Vec<NavigationLocation>,
        truncated: bool,
    },
    Inspection {
        id: RequestId,
        key: String,
        frames: Vec<StackFrame>,
    },
    InspectionGeometry {
        id: RequestId,
        meshes: Vec<Mesh>,
        plane: Option<Plane>,
    },
    InterferenceGeometry {
        id: RequestId,
        mesh: Box<Mesh>,
    },
    Measurement(Box<Measurement>),
    MeasurementCode {
        id: RequestId,
        code: String,
        /// The worker ran the code and it reproduced every value.
        verified: bool,
        /// Why it is not verified.
        problem: Option<String>,
    },
    Selector(SelectorReply),
    SelectorPreview {
        id: RequestId,
        active: bool,
        count: usize,
        matches: Vec<TopologyReference>,
        message: Option<String>,
    },
    SceneAdd {
        #[serde(default)]
        id: Option<RequestId>,
        objects: Vec<SceneNode>,
    },
    ConsoleResult {
        id: RequestId,
        more: bool,
    },
    /// A rich result of a console command: its text and, perhaps, an image
    /// already decoded (on the reader thread, not the one that draws).
    Display(DisplayOutput),
    ExportFinished {
        id: RequestId,
        path: String,
    },
    GeometryChecked {
        id: RequestId,
        reports: Vec<GeometryReport>,
        #[serde(default)]
        interferences: Vec<InterferenceReport>,
    },
    GeometryInvalid {
        id: RequestId,
        reports: Vec<GeometryReport>,
        #[serde(default)]
        interferences: Vec<InterferenceReport>,
    },
    UploadFinished {
        id: RequestId,
        status: u16,
        body: String,
    },
    Updates {
        id: RequestId,
        /// CadQuery's version, and its newer release tags.
        current: String,
        newer: Vec<String>,
        #[serde(default)]
        editor: Option<EditorUpdates>,
    },
    Imports {
        id: RequestId,
        paths: Vec<String>,
    },
    Error {
        id: Option<RequestId>,
        message: String,
    },
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ReadyInfo {
    pub protocol: u32,
    pub python: String,
    pub cadquery: String,
    #[serde(default = "unknown_version")]
    pub ocp: String,
    pub executable: String,
    pub completion: bool,
}

fn unknown_version() -> String {
    "unknown".to_owned()
}

#[derive(Deserialize, Debug)]
pub struct RunFinished {
    pub id: RequestId,
    pub ok: bool,
    pub debug: bool,
    pub stopped: bool,
    /// Meshing was cancelled; the current scene stays.
    #[serde(default)]
    pub cancelled: bool,
    /// Stop interrupted the script; `error` says where.
    #[serde(default)]
    pub interrupted: bool,
    /// Where the time went, for a run asked to `profile`.
    #[serde(default)]
    pub profile: Option<RunProfile>,
    /// `None` when a failed run should keep the current scene.
    pub objects: Option<Vec<SceneNode>>,
    pub variables: Option<Vec<Variable>>,
    pub error: Option<ScriptError>,
    /// Requirements declared with expect() during this run.
    #[serde(default)]
    pub checks: Vec<CadCheck>,
    pub duration: f64,
    #[serde(default)]
    pub timings: Option<RunTimings>,
    /// Filled in by the reader thread, not sent by the worker.
    #[serde(skip)]
    pub receipt: Receipt,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CadCheck {
    pub name: String,
    pub kind: String,
    pub ok: bool,
    pub detail: String,
    pub line: Option<usize>,
}

/// The largest image the console shows, on either side, in pixels.
pub const MAX_DISPLAY_SIDE: u32 = 4096;
/// Decoded image bytes accepted, matching the worker-side limit.
const MAX_DISPLAY_IMAGE_BYTES: usize = 4 * 1024 * 1024;
/// Largest base64 string that can encode [`MAX_DISPLAY_IMAGE_BYTES`].
const MAX_DISPLAY_DATA: usize = 4 * MAX_DISPLAY_IMAGE_BYTES.div_ceil(3);

/// A console result as the worker sends it.
#[derive(Deserialize)]
struct RawDisplay {
    id: RequestId,
    #[serde(default)]
    text: String,
    #[serde(default)]
    image: Option<RawImage>,
}

#[derive(Deserialize)]
struct RawImage {
    format: String,
    data: String,
}

/// A console result: text, and an image checked and decoded, or a note in
/// the text saying why it is not shown.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(from = "RawDisplay")]
pub struct DisplayOutput {
    pub id: RequestId,
    pub text: String,
    pub image: Option<DisplayImage>,
}

/// Decoded pixels, RGBA, row by row.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayImage {
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
}

impl From<RawDisplay> for DisplayOutput {
    fn from(raw: RawDisplay) -> Self {
        let mut text = raw.text;
        let image = raw
            .image
            .and_then(|image| match decode_display_image(&image) {
                Ok(image) => Some(image),
                Err(problem) => {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&format!("[image not shown: {problem}]"));
                    None
                }
            });
        Self {
            id: raw.id,
            text,
            image,
        }
    }
}

/// Only PNG and JPEG, within [`MAX_DISPLAY_SIDE`] and a memory limit: the
/// worker runs user code, so its images are not trusted.
fn decode_display_image(image: &RawImage) -> Result<DisplayImage, String> {
    let format = match image.format.as_str() {
        "png" => image::ImageFormat::Png,
        "jpeg" => image::ImageFormat::Jpeg,
        other => return Err(format!("{other} is not shown")),
    };
    if image.data.len() > MAX_DISPLAY_DATA {
        return Err("too large".to_owned());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&image.data)
        .map_err(|error| format!("bad data ({error})"))?;
    if bytes.len() > MAX_DISPLAY_IMAGE_BYTES {
        return Err("too large".to_owned());
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DISPLAY_SIDE);
    limits.max_image_height = Some(MAX_DISPLAY_SIDE);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let size = [decoded.width() as usize, decoded.height() as usize];
    Ok(DisplayImage {
        size,
        rgba: decoded.into_raw(),
    })
}

/// CQ-editor's releases newer than the running one, newest first.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct EditorUpdates {
    pub current: String,
    pub newer: Vec<Release>,
}

/// A published release of CQ-editor; also kept in the preferences.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub name: String,
    /// Its page, with the installers.
    pub url: String,
}

/// The time a profiled run spent on each line of the script, with
/// everything the line called, and on its calls into CadQuery.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct RunProfile {
    /// In line order.
    pub lines: Vec<LineTime>,
    /// The slowest calls of each line, slowest first.
    pub operations: Vec<OperationTime>,
    pub sources: BTreeMap<String, String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct LineTime {
    #[serde(default)]
    pub file: Option<String>,
    /// 1-based.
    pub line: usize,
    pub seconds: f64,
    /// How many times the line started.
    pub hits: u64,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct OperationTime {
    #[serde(default)]
    pub file: Option<String>,
    pub line: usize,
    /// For example `Workplane.fillet`.
    pub name: String,
    pub seconds: f64,
    pub calls: u64,
}

/// Where the worker spent a run's time, in seconds, and what it sent.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct RunTimings {
    pub script: f64,
    /// Content ids: hashing each shape's BREP.
    pub identify: f64,
    pub tessellate: f64,
    /// Base64 encoding of the mesh arrays.
    pub encode: f64,
    pub meshed: usize,
    pub cached: usize,
    pub referenced: usize,
    pub mesh_bytes: usize,
    /// Worker wall clock (seconds since the Unix epoch) just before sending.
    pub sent_at: f64,
}

/// When the GUI read a message and how long parsing it took.
#[derive(Debug, Clone, Default)]
pub struct Receipt {
    pub received_at: Option<SystemTime>,
    pub parse: Duration,
}

impl RunTimings {
    /// Time from the worker finishing the reply until the GUI read it:
    /// serialization and the pipe. `None` if the clocks disagree.
    pub fn transfer(&self, receipt: &Receipt) -> Option<Duration> {
        let sent = UNIX_EPOCH.checked_add(Duration::try_from_secs_f64(self.sent_at).ok()?)?;
        receipt.received_at?.duration_since(sent).ok()
    }

    /// One log line covering every stage of a run.
    pub fn report(&self, receipt: &Receipt, decode: Duration) -> String {
        let transfer = self
            .transfer(receipt)
            .map_or_else(|| "?".to_owned(), |d| format!("{:.3}", d.as_secs_f64()));
        format!(
            "Run timings (s): script {:.3}, content ids {:.3}, meshing {:.3}, encoding {:.3}, \
             transfer {transfer}, parsing {:.3}, scene {:.3}; meshes: {} meshed, {} cached, \
             {} reused by reference, {:.2} MB sent",
            self.script,
            self.identify,
            self.tessellate,
            self.encode,
            receipt.parse.as_secs_f64(),
            decode.as_secs_f64(),
            self.meshed,
            self.cached,
            self.referenced,
            self.mesh_bytes as f64 / 1e6,
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Variable {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub value: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ScriptError {
    pub type_name: String,
    pub message: String,
    pub frames: Vec<TracebackFrame>,
    /// Last line of the editor buffer involved, if any.
    pub line: Option<usize>,
    /// Python's own formatting of the traceback.
    pub text: String,
    /// Conservative troubleshooting hints tied to a CAD call in the script.
    #[serde(default)]
    pub diagnostic: Option<CadDiagnostic>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CadDiagnostic {
    pub operation: String,
    pub receiver: Option<String>,
    pub line: usize,
    pub hints: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TracebackFrame {
    pub file: String,
    pub line: usize,
    pub function: String,
    pub code: String,
}

/// Pseudo file name of the editor buffer in worker tracebacks.
pub const SCRIPT_FILENAME: &str = "<cq-editor>";

/// Documentation of a name, for the editor's hover tooltip. Both parts are
/// empty when there is nothing to show.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Documentation {
    /// Call signatures, or `name: type` for a variable.
    #[serde(default)]
    pub signatures: Vec<String>,
    /// The docstring, cut to a tooltip's length.
    #[serde(default)]
    pub text: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CompletionItem {
    pub name: String,
    /// Text to insert at the cursor to complete `name`.
    pub insert: String,
    pub kind: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StackFrame {
    #[serde(rename = "type")]
    pub type_name: String,
    pub value: String,
    pub items: Vec<StackItem>,
    /// The script line that made this step.
    #[serde(default)]
    pub line: Option<usize>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StackItem {
    #[serde(rename = "type")]
    pub type_name: String,
    pub value: String,
    pub displayable: bool,
    #[serde(default)]
    pub line: Option<usize>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Plane {
    pub origin: [f32; 3],
    pub x_dir: [f32; 3],
    pub z_dir: [f32; 3],
    pub size: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TopologyKind {
    Face,
    Edge,
    Vertex,
}

/// The GUI's selection for the console's `selected()`.
#[derive(Serialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct ConsoleSelection {
    /// Picked faces, edges and vertices, in the order they were picked.
    pub entities: Vec<TopologyReference>,
    /// Selected objects, outermost first; used when no entity is picked.
    pub objects: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TopologyReference {
    pub key: String,
    pub part: usize,
    pub kind: TopologyKind,
    pub identifier: u32,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Measurement {
    pub id: RequestId,
    pub identity: TopologyIdentity,
    pub entities: Vec<EntityMeasurement>,
    pub pair: Option<PairMeasurement>,
    /// The selected objects, measured when no entity is selected.
    #[serde(default)]
    pub objects: Vec<ObjectMeasurement>,
    /// Around everything measured.
    #[serde(default)]
    pub bounding_box: Option<BoundingBox>,
}

/// An axis-aligned box from the exact geometry, in world coordinates.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub size: [f64; 3],
    pub center: [f64; 3],
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ObjectMeasurement {
    pub key: String,
    /// `solid`, `compound`, `face` …
    pub shape_type: String,
    pub volume: Option<f64>,
    pub area: Option<f64>,
    /// Of the edges, for objects without faces.
    pub length: Option<f64>,
    /// Center of mass.
    pub center: [f64; 3],
    pub bounding_box: Option<BoundingBox>,
    pub solids: usize,
    pub faces: usize,
    pub edges: usize,
    pub vertices: usize,
    #[serde(default)]
    pub mass_g: Option<f64>,
    #[serde(default)]
    pub materials: Vec<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TopologyIdentity {
    pub scope: String,
    pub persistent: bool,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct EntityMeasurement {
    pub key: String,
    pub part: usize,
    pub kind: TopologyKind,
    pub identifier: u32,
    pub geometry_type: String,
    pub center: [f64; 3],
    pub position: Option<[f64; 3]>,
    pub length: Option<f64>,
    pub area: Option<f64>,
    pub radius: Option<f64>,
    pub diameter: Option<f64>,
    #[serde(default)]
    pub bounding_box: Option<BoundingBox>,
}

/// Selector expressions for a topology selection, verified by the worker
/// against CadQuery itself.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct SelectorReply {
    pub id: RequestId,
    /// `faces`, `edges` or `vertices`.
    pub method: String,
    /// Best first; never empty.
    pub candidates: Vec<SelectorCandidate>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct SelectorCandidate {
    /// Ready to append to the object, e.g. `.faces(">Z")`.
    pub expression: String,
    /// What it means, e.g. "the faces with the largest Z".
    #[serde(default)]
    pub description: String,
    /// Relies on CadQuery's entity order rather than on geometry, so it
    /// can select another entity once the model changes.
    #[serde(default)]
    pub index_based: bool,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct PairMeasurement {
    pub distance: f64,
    pub first_point: [f64; 3],
    pub second_point: [f64; 3],
    pub angle_degrees: Option<f64>,
    pub angle_kind: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct SceneNode {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub inspectable: bool,
    /// The script line that created the object (or showed it).
    #[serde(default)]
    pub line: Option<usize>,
    pub color: Option<[f32; 3]>,
    pub alpha: Option<f32>,
    pub edge_color: Option<[f32; 3]>,
    /// The `material` option of `show_object`, as `Finish::name` spells it.
    #[serde(default)]
    pub finish: Option<String>,
    pub parts: Vec<Part>,
    pub children: Vec<SceneNode>,
}

#[derive(Deserialize, Debug)]
pub struct Part {
    pub mesh: Mesh,
    #[serde(default = "identity_transform")]
    pub transform: [f32; 16],
    pub color: Option<[f32; 3]>,
    pub alpha: Option<f32>,
    #[serde(default)]
    pub face_attributes: Vec<FaceAttribute>,
    pub material: Option<Material>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct FaceAttribute {
    pub id: u32,
    pub name: Option<String>,
    pub layer: Option<String>,
    pub color: Option<[f32; 3]>,
    pub alpha: Option<f32>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Material {
    pub name: String,
    pub density: f64,
    pub density_unit: String,
}

/// Triangulated geometry, already decoded.
#[derive(Deserialize, Debug, Default)]
pub struct Mesh {
    /// Stable digest of the local CAD geometry and tessellation settings.
    #[serde(default)]
    pub id: String,
    #[serde(default, deserialize_with = "vec3_f32")]
    pub positions: Vec<[f32; 3]>,
    #[serde(default, deserialize_with = "vec3_f32")]
    pub normals: Vec<[f32; 3]>,
    #[serde(default, deserialize_with = "u32_array")]
    pub indices: Vec<u32>,
    /// Line segments, two points each.
    #[serde(default, deserialize_with = "vec3_f32")]
    pub edges: Vec<[f32; 3]>,
    #[serde(default, deserialize_with = "vec3_f32")]
    pub points: Vec<[f32; 3]>,
    /// Degrees between the faces that meet at each segment's edge.
    #[serde(default, deserialize_with = "f32_array")]
    pub segment_angles: Vec<f32>,
    #[serde(default)]
    pub topology: MeshTopology,
    /// Absent only for a content-addressed reference.
    #[serde(default)]
    pub bbox: Option<[[f32; 3]; 2]>,
}

/// OCCT subshape associations for precise face, edge and vertex picking.
#[derive(Deserialize, Debug, Default)]
pub struct MeshTopology {
    #[serde(default, deserialize_with = "vec3_f32")]
    pub face_positions: Vec<[f32; 3]>,
    #[serde(default, deserialize_with = "u32_array")]
    pub face_indices: Vec<u32>,
    #[serde(default, deserialize_with = "u32_array")]
    pub triangle_faces: Vec<u32>,
    #[serde(default, deserialize_with = "u32_array")]
    pub segment_edges: Vec<u32>,
    #[serde(default, deserialize_with = "vec3_f32")]
    pub vertices: Vec<[f32; 3]>,
}

fn identity_transform() -> [f32; 16] {
    glam::Mat4::IDENTITY.to_cols_array()
}

fn base64_bytes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
    base64::engine::general_purpose::STANDARD
        .decode(text.as_bytes())
        .map_err(serde::de::Error::custom)
}

fn vec3_f32<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<[f32; 3]>, D::Error> {
    let bytes = base64_bytes(deserializer)?;
    if bytes.len() % 12 != 0 {
        return Err(serde::de::Error::custom(
            "vector array length is not a multiple of 12",
        ));
    }
    Ok(bytes
        .as_chunks::<12>()
        .0
        .iter()
        .map(|c| {
            let f = |i: usize| f32::from_le_bytes([c[i], c[i + 1], c[i + 2], c[i + 3]]);
            [f(0), f(4), f(8)]
        })
        .collect())
}

fn f32_array<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<f32>, D::Error> {
    let bytes = base64_bytes(deserializer)?;
    if bytes.len() % 4 != 0 {
        return Err(serde::de::Error::custom(
            "float array length is not a multiple of 4",
        ));
    }
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn u32_array<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u32>, D::Error> {
    let bytes = base64_bytes(deserializer)?;
    if bytes.len() % 4 != 0 {
        return Err(serde::de::Error::custom(
            "index array length is not a multiple of 4",
        ));
    }
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

#[cfg(test)]
#[path = "../../tests/rust/protocol/test_protocol.rs"]
mod tests;
