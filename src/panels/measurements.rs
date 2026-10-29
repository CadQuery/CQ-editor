use egui_phosphor::regular as icon;
use serde_json::json;

use crate::app::AppState;
use crate::commands::{Action, MeasurementFormat};
use crate::scene::{Scene, TopologySelection};
use crate::settings::Units;
use crate::worker::protocol::{
    BoundingBox, EntityMeasurement, Measurement, ObjectMeasurement, PairMeasurement, RequestId,
    SelectorCandidate, SelectorReply, TopologyKind,
};

#[derive(Default)]
pub struct MeasurementState {
    pub result: Option<Measurement>,
    pub loading: bool,
    pub error: Option<String>,
    pub request_id: Option<RequestId>,
}

impl MeasurementState {
    pub fn clear(&mut self) {
        self.result = None;
        self.loading = false;
        self.error = None;
        self.request_id = None;
    }
}

/// What to do with a selector once the worker found it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorUse {
    Copy,
    Insert,
}

/// The CadQuery selector of the current topology selection.
#[derive(Default)]
pub struct SelectorState {
    pub reply: Option<SelectorReply>,
    /// The candidate in use, an index into `reply.candidates`.
    pub chosen: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub request_id: Option<RequestId>,
    /// Asked for before the reply arrived; carried out on arrival.
    pub then: Option<SelectorUse>,
}

/// A read-only selector preview tied to the caret of the last successful run.
#[derive(Default)]
pub struct SelectorPreviewState {
    pub cursor: Option<(RequestId, usize, usize)>,
    pub due_at: Option<f64>,
    pub request_id: Option<RequestId>,
    pub active: bool,
    pub count: usize,
    pub shown: usize,
    pub message: Option<String>,
}

impl SelectorPreviewState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

impl SelectorState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn candidate(&self) -> Option<&SelectorCandidate> {
        let candidates = &self.reply.as_ref()?.candidates;
        candidates.get(self.chosen).or_else(|| candidates.first())
    }
}

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    let mut actions = Vec::new();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.strong("Exact CAD measurements");
            if state.measurements.loading {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has_result = state.measurements.result.is_some();
                if ui
                    .add_enabled(
                        has_result,
                        egui::Button::new(format!("{} Copy as code", icon::CLIPBOARD_TEXT)),
                    )
                    .on_hover_text(
                        "Python measuring the same entities from the script's variables, \
                         checked against these values",
                    )
                    .clicked()
                {
                    actions.push(Action::CopyMeasurementCode);
                }
                ui.add_enabled_ui(has_result, |ui| {
                    ui.menu_button(format!("{} Copy data", icon::COPY), |ui| {
                        for format in MeasurementFormat::ALL {
                            if ui
                                .button(format!("Copy as {}", format.label()))
                                .clicked()
                            {
                                actions.push(Action::CopyMeasurement(format));
                                ui.close();
                            }
                        }
                    })
                    .response
                    .on_hover_text("Copy measurement values as structured Text, JSON or CSV");
                });
            });
        });
        ui.small(format!(
            "Selection mode: {} · Ctrl+click adds entities",
            state.viewer.selection_mode.label()
        ));
        ui.add_space(6.0);

        if state.selector_preview.active {
            ui.separator();
            let preview = &state.selector_preview;
            ui.strong(format!("Selector preview: {} matching entities", preview.count));
            if preview.shown < preview.count {
                ui.weak(format!("Showing {} highlights", preview.shown));
            }
            if let Some(message) = &preview.message {
                ui.colored_label(ui.visuals().warn_fg_color, message);
            }
            ui.separator();
        }

        if !state.scene.topology_selections.is_empty() {
            selector_ui(ui, &mut state.selector, &mut actions);
            ui.separator();
        }

        if let Some(error) = &state.measurements.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
            return;
        }
        let Some(result) = &state.measurements.result else {
            super::empty_state(
                ui,
                icon::RULER,
                "Select an object, or a face, edge or vertex, in the 3D view",
            );
            return;
        };

        for (index, entity) in result.entities.iter().enumerate() {
            if index > 0 {
                ui.separator();
            }
            entity_ui(ui, state, entity, index + 1);
        }
        for (index, object) in result.objects.iter().enumerate() {
            if index > 0 {
                ui.separator();
            }
            object_ui(ui, state, object, index + 1);
        }
        if let Some(pair) = &result.pair {
            ui.separator();
            pair_ui(ui, pair);
        }
        if result.entities.len() + result.objects.len() > 1
            && let Some(bounds) = &result.bounding_box
        {
            ui.separator();
            ui.strong("Selection bounds");
            egui::Grid::new("measurement_selection_bounds")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| bounds_rows(ui, bounds));
        }

        ui.separator();
        ui.weak(
            "Topology IDs belong to this render only. Rebuilding or remodeling the object may change them.",
        );
    });
    state.queue(actions);
}

/// The selector of the picked entities, with Copy and Insert.
pub fn selector_ui(ui: &mut egui::Ui, selector: &mut SelectorState, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.label(format!("{} Selector", icon::SELECTION));
        if selector.loading {
            ui.spinner();
        }
    });
    if let Some(error) = &selector.error {
        ui.colored_label(ui.visuals().warn_fg_color, error);
        return;
    }
    let Some(reply) = &selector.reply else {
        return;
    };
    let Some(candidate) = selector.candidate().cloned() else {
        return;
    };
    ui.add(egui::Label::new(egui::RichText::new(&candidate.expression).monospace()).wrap());
    if !candidate.description.is_empty() {
        ui.weak(&candidate.description);
    }
    if candidate.index_based {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!(
                "{} No geometric selector matches exactly; this index may select another entity once the model changes",
                icon::WARNING
            ),
        );
    }
    ui.horizontal(|ui| {
        if ui
            .button(format!("{} Copy", icon::COPY))
            .on_hover_text("Copy the selector to the clipboard")
            .clicked()
        {
            actions.push(Action::CopySelector);
        }
        let insert = ui
            .button(format!("{} Insert", icon::CURSOR_TEXT))
            .on_hover_text("Insert the selector at the editor's cursor");
        if insert.clicked() {
            actions.push(Action::InsertSelector);
        }
        if reply.candidates.len() > 1 {
            let mut chosen = selector.chosen.min(reply.candidates.len() - 1);
            egui::ComboBox::from_id_salt("selector_alternatives")
                .selected_text("Alternatives")
                .show_ui(ui, |ui| {
                    for (index, alternative) in reply.candidates.iter().enumerate() {
                        ui.selectable_value(&mut chosen, index, &alternative.expression)
                            .on_hover_text(&alternative.description);
                    }
                });
            selector.chosen = chosen;
        }
    });
}

/// The measurement of `selection` in `result`.
pub fn measurement_of<'a>(
    result: &'a Measurement,
    selection: &TopologySelection,
) -> Option<&'a EntityMeasurement> {
    result.entities.iter().find(|entity| {
        entity.key == selection.node
            && entity.part == selection.part
            && entity.kind == selection.entity.topology_kind()
            && entity.identifier == selection.entity.identifier()
    })
}

fn entity_ui(ui: &mut egui::Ui, state: &AppState, entity: &EntityMeasurement, number: usize) {
    let name = state
        .scene
        .find(&entity.key)
        .map_or(entity.key.as_str(), |node| node.name.as_str());
    let attribute = state.scene.find(&entity.key).and_then(|node| {
        (entity.kind == TopologyKind::Face)
            .then(|| node.parts.get(entity.part))
            .flatten()
            .and_then(|part| {
                part.face_attributes
                    .iter()
                    .find(|face| face.id == entity.identifier)
            })
    });
    let face_label = attribute.and_then(|face| face.name.as_deref()).map_or_else(
        || format!("{} {}", kind_label(entity.kind), entity.identifier),
        str::to_owned,
    );
    ui.strong(format!("{number}. {face_label} · {name}"));
    egui::Grid::new(("measurement_entity", number))
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            row(ui, "Geometry", &title_case(&entity.geometry_type));
            if let Some(face) = attribute
                && let Some(layer) = &face.layer
            {
                row(ui, "Layer", layer);
            }
            row(ui, "Part", &(entity.part + 1).to_string());
            row(ui, "Center", &format_point(entity.center));
            if let Some(position) = entity.position {
                row(ui, "Position", &format_point(position));
            }
            if let Some(value) = entity.length {
                row(ui, "Length", &format_number(value));
            }
            if let Some(value) = entity.area {
                row(ui, "Area", &format_number(value));
            }
            if let Some(value) = entity.radius {
                row(ui, "Radius", &format_number(value));
            }
            if let Some(value) = entity.diameter {
                row(ui, "Diameter", &format_number(value));
            }
            if let Some(bounds) = &entity.bounding_box {
                bounds_rows(ui, bounds);
            }
        });
}

fn object_ui(ui: &mut egui::Ui, state: &AppState, object: &ObjectMeasurement, number: usize) {
    let name = state
        .scene
        .find(&object.key)
        .map_or(object.key.as_str(), |node| node.name.as_str());
    ui.strong(format!("{number}. {name}"));
    egui::Grid::new(("measurement_object", number))
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            row(ui, "Shape", &title_case(&object.shape_type));
            if let Some(value) = object.volume {
                row(ui, "Volume", &format_number(value));
            }
            if let Some(value) = object.mass_g {
                row(ui, "Mass (g)", &format_number(value));
            }
            if !object.materials.is_empty() {
                row(ui, "Materials", &object.materials.join(", "));
            }
            if let Some(value) = object.area {
                row(ui, "Area", &format_number(value));
            }
            if let Some(value) = object.length {
                row(ui, "Length", &format_number(value));
            }
            row(ui, "Center of mass", &format_point(object.center));
            row(
                ui,
                "Topology",
                &format!(
                    "{} solids · {} faces · {} edges · {} vertices",
                    object.solids, object.faces, object.edges, object.vertices
                ),
            );
            if let Some(bounds) = &object.bounding_box {
                bounds_rows(ui, bounds);
            }
        });
}

/// Size, minimum and maximum of an exact bounding box.
fn bounds_rows(ui: &mut egui::Ui, bounds: &BoundingBox) {
    let [x, y, z] = bounds.size;
    row(
        ui,
        "Bounding box",
        &format!(
            "{} × {} × {}",
            format_number(x),
            format_number(y),
            format_number(z)
        ),
    );
    row(ui, "Min", &format_point(bounds.min));
    row(ui, "Max", &format_point(bounds.max));
}

fn pair_ui(ui: &mut egui::Ui, pair: &PairMeasurement) {
    ui.strong("Relationship");
    egui::Grid::new("measurement_pair")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            row(ui, "Distance", &format_number(pair.distance));
            row(ui, "Closest point 1", &format_point(pair.first_point));
            row(ui, "Closest point 2", &format_point(pair.second_point));
            if let Some(angle) = pair.angle_degrees {
                let label = match pair.angle_kind.as_deref() {
                    Some("edge_edge") => "Edge angle",
                    Some("face_face") => "Face angle",
                    Some("edge_face") => "Edge/plane angle",
                    _ => "Angle",
                };
                row(ui, label, &format!("{}°", format_number(angle)));
            }
        });
}

fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(label);
    ui.monospace(value);
    ui.end_row();
}

fn kind_label(kind: TopologyKind) -> &'static str {
    match kind {
        TopologyKind::Face => "Face",
        TopologyKind::Edge => "Edge",
        TopologyKind::Vertex => "Vertex",
    }
}

pub fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

pub fn format_point(point: [f64; 3]) -> String {
    format!(
        "({}, {}, {})",
        format_number(point[0]),
        format_number(point[1]),
        format_number(point[2])
    )
}

pub fn format_number(value: f64) -> String {
    let absolute = value.abs();
    if absolute != 0.0 && !(1e-5..1e9).contains(&absolute) {
        return format!("{value:.8e}");
    }
    let text = format!("{value:.9}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Formats `result` as structured text, JSON, or CSV with units.
pub fn format_measurement(
    result: &Measurement,
    scene: &Scene,
    units: Units,
    format: MeasurementFormat,
) -> String {
    match format {
        MeasurementFormat::Text => format_measurement_text(result, scene, units),
        MeasurementFormat::Json => format_measurement_json(result, scene, units),
        MeasurementFormat::Csv => format_measurement_csv(result, scene, units),
    }
}

fn unit_strings(units: Units) -> (&'static str, String, String) {
    let linear = units.suffix();
    if linear.is_empty() {
        ("", String::new(), String::new())
    } else {
        (linear, format!("{linear}²"), format!("{linear}³"))
    }
}

fn with_unit(value: &str, unit: &str) -> String {
    if unit.is_empty() {
        value.to_owned()
    } else {
        format!("{value} {unit}")
    }
}

fn entity_label_for(entity: &EntityMeasurement, scene: &Scene) -> (String, String) {
    let object_name = scene
        .find(&entity.key)
        .map_or(entity.key.as_str(), |node| node.name.as_str())
        .to_owned();
    let attribute = scene.find(&entity.key).and_then(|node| {
        (entity.kind == TopologyKind::Face)
            .then(|| node.parts.get(entity.part))
            .flatten()
            .and_then(|part| {
                part.face_attributes
                    .iter()
                    .find(|face| face.id == entity.identifier)
            })
    });
    let label = attribute.and_then(|face| face.name.as_deref()).map_or_else(
        || format!("{} {}", kind_label(entity.kind), entity.identifier),
        str::to_owned,
    );
    (object_name, label)
}

fn format_measurement_text(result: &Measurement, scene: &Scene, units: Units) -> String {
    let (u_lin, u_area, u_vol) = unit_strings(units);
    let mut lines = Vec::new();

    if let Some(pair) = &result.pair {
        let delta = [
            pair.second_point[0] - pair.first_point[0],
            pair.second_point[1] - pair.first_point[1],
            pair.second_point[2] - pair.first_point[2],
        ];
        lines.push(format!(
            "Distance: {}",
            with_unit(&format_number(pair.distance), u_lin)
        ));
        lines.push(format!("Delta: {}", with_unit(&format_point(delta), u_lin)));
        lines.push(format!(
            "Closest point 1: {}",
            with_unit(&format_point(pair.first_point), u_lin)
        ));
        lines.push(format!(
            "Closest point 2: {}",
            with_unit(&format_point(pair.second_point), u_lin)
        ));
        if let Some(angle) = pair.angle_degrees {
            lines.push(format!("Angle: {}°", format_number(angle)));
        }
    }

    for entity in &result.entities {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        let (object_name, label) = entity_label_for(entity, scene);
        lines.push(format!(
            "{label} · {object_name} ({})",
            title_case(&entity.geometry_type)
        ));
        lines.push(format!(
            "Center: {}",
            with_unit(&format_point(entity.center), u_lin)
        ));
        if let Some(position) = entity.position {
            lines.push(format!(
                "Position: {}",
                with_unit(&format_point(position), u_lin)
            ));
        }
        if let Some(length) = entity.length {
            lines.push(format!(
                "Length: {}",
                with_unit(&format_number(length), u_lin)
            ));
        }
        if let Some(area) = entity.area {
            lines.push(format!(
                "Area: {}",
                with_unit(&format_number(area), &u_area)
            ));
        }
        if let Some(radius) = entity.radius {
            lines.push(format!(
                "Radius: {}",
                with_unit(&format_number(radius), u_lin)
            ));
        }
        if let Some(diameter) = entity.diameter {
            lines.push(format!(
                "Diameter: {}",
                with_unit(&format_number(diameter), u_lin)
            ));
        }
        if let Some(bounds) = &entity.bounding_box {
            let [x, y, z] = bounds.size;
            lines.push(format!(
                "Bounding box: {}",
                with_unit(
                    &format!(
                        "{} × {} × {}",
                        format_number(x),
                        format_number(y),
                        format_number(z)
                    ),
                    u_lin
                )
            ));
        }
    }

    for object in &result.objects {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        let name = scene
            .find(&object.key)
            .map_or(object.key.as_str(), |node| node.name.as_str());
        lines.push(format!("{name} ({})", title_case(&object.shape_type)));
        if let Some(volume) = object.volume {
            lines.push(format!(
                "Volume: {}",
                with_unit(&format_number(volume), &u_vol)
            ));
        }
        if let Some(mass) = object.mass_g {
            lines.push(format!("Mass: {} g", format_number(mass)));
        }
        if let Some(area) = object.area {
            lines.push(format!(
                "Area: {}",
                with_unit(&format_number(area), &u_area)
            ));
        }
        if let Some(length) = object.length {
            lines.push(format!(
                "Length: {}",
                with_unit(&format_number(length), u_lin)
            ));
        }
        lines.push(format!(
            "Center of mass: {}",
            with_unit(&format_point(object.center), u_lin)
        ));
        if let Some(bounds) = &object.bounding_box {
            let [x, y, z] = bounds.size;
            lines.push(format!(
                "Bounding box: {}",
                with_unit(
                    &format!(
                        "{} × {} × {}",
                        format_number(x),
                        format_number(y),
                        format_number(z)
                    ),
                    u_lin
                )
            ));
            lines.push(format!(
                "Min: {}",
                with_unit(&format_point(bounds.min), u_lin)
            ));
            lines.push(format!(
                "Max: {}",
                with_unit(&format_point(bounds.max), u_lin)
            ));
        }
    }

    if result.entities.len() + result.objects.len() > 1
        && let Some(bounds) = &result.bounding_box
    {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        let [x, y, z] = bounds.size;
        lines.push("Selection bounds".to_owned());
        lines.push(format!(
            "Bounding box: {}",
            with_unit(
                &format!(
                    "{} × {} × {}",
                    format_number(x),
                    format_number(y),
                    format_number(z)
                ),
                u_lin
            )
        ));
        lines.push(format!(
            "Min: {}",
            with_unit(&format_point(bounds.min), u_lin)
        ));
        lines.push(format!(
            "Max: {}",
            with_unit(&format_point(bounds.max), u_lin)
        ));
    }

    lines.join("\n")
}

fn format_measurement_json(result: &Measurement, scene: &Scene, units: Units) -> String {
    let (u_lin, u_area, u_vol) = unit_strings(units);
    let entities: Vec<_> = result
        .entities
        .iter()
        .map(|entity| {
            let (object_name, label) = entity_label_for(entity, scene);
            json!({
                "object": object_name,
                "label": label,
                "kind": kind_label(entity.kind).to_ascii_lowercase(),
                "id": entity.identifier,
                "part": entity.part + 1,
                "geometry_type": entity.geometry_type,
                "center": entity.center,
                "position": entity.position,
                "length": entity.length,
                "area": entity.area,
                "radius": entity.radius,
                "diameter": entity.diameter,
                "bounding_box": entity.bounding_box.as_ref().map(|b| json!({
                    "size": b.size,
                    "min": b.min,
                    "max": b.max,
                })),
            })
        })
        .collect();

    let objects: Vec<_> = result
        .objects
        .iter()
        .map(|object| {
            let name = scene
                .find(&object.key)
                .map_or(object.key.as_str(), |node| node.name.as_str());
            json!({
                "name": name,
                "shape_type": object.shape_type,
                "volume": object.volume,
                "mass_g": object.mass_g,
                "materials": object.materials,
                "area": object.area,
                "length": object.length,
                "center": object.center,
                "topology": {
                    "solids": object.solids,
                    "faces": object.faces,
                    "edges": object.edges,
                    "vertices": object.vertices,
                },
                "bounding_box": object.bounding_box.as_ref().map(|b| json!({
                    "size": b.size,
                    "min": b.min,
                    "max": b.max,
                })),
            })
        })
        .collect();

    let pair = result.pair.as_ref().map(|pair| {
        let delta = [
            pair.second_point[0] - pair.first_point[0],
            pair.second_point[1] - pair.first_point[1],
            pair.second_point[2] - pair.first_point[2],
        ];
        json!({
            "distance": pair.distance,
            "delta": delta,
            "first_point": pair.first_point,
            "second_point": pair.second_point,
            "angle_degrees": pair.angle_degrees,
            "angle_kind": pair.angle_kind,
        })
    });

    let payload = json!({
        "unit": u_lin,
        "area_unit": u_area,
        "volume_unit": u_vol,
        "pair": pair,
        "entities": entities,
        "objects": objects,
        "bounding_box": result.bounding_box.as_ref().map(|b| json!({
            "size": b.size,
            "min": b.min,
            "max": b.max,
        })),
    });

    serde_json::to_string_pretty(&payload).unwrap_or_default()
}

fn csv_cell(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

fn push_csv_row(rows: &mut Vec<String>, target: &str, property: &str, value: &str, unit: &str) {
    rows.push(format!(
        "{},{},{},{}",
        csv_cell(target),
        csv_cell(property),
        csv_cell(value),
        csv_cell(unit)
    ));
}

fn format_measurement_csv(result: &Measurement, scene: &Scene, units: Units) -> String {
    let (u_lin, u_area, u_vol) = unit_strings(units);
    let mut rows = vec!["target,property,value,unit".to_owned()];

    if let Some(pair) = &result.pair {
        let delta = [
            pair.second_point[0] - pair.first_point[0],
            pair.second_point[1] - pair.first_point[1],
            pair.second_point[2] - pair.first_point[2],
        ];
        push_csv_row(
            &mut rows,
            "pair",
            "distance",
            &format_number(pair.distance),
            u_lin,
        );
        push_csv_row(&mut rows, "pair", "dx", &format_number(delta[0]), u_lin);
        push_csv_row(&mut rows, "pair", "dy", &format_number(delta[1]), u_lin);
        push_csv_row(&mut rows, "pair", "dz", &format_number(delta[2]), u_lin);
        if let Some(angle) = pair.angle_degrees {
            push_csv_row(&mut rows, "pair", "angle", &format_number(angle), "deg");
        }
    }

    for entity in &result.entities {
        let (object_name, label) = entity_label_for(entity, scene);
        let target = format!("{object_name}:{label}");
        push_csv_row(&mut rows, &target, "geometry", &entity.geometry_type, "");
        push_csv_row(
            &mut rows,
            &target,
            "center",
            &format_point(entity.center),
            u_lin,
        );
        if let Some(position) = entity.position {
            push_csv_row(
                &mut rows,
                &target,
                "position",
                &format_point(position),
                u_lin,
            );
        }
        if let Some(length) = entity.length {
            push_csv_row(&mut rows, &target, "length", &format_number(length), u_lin);
        }
        if let Some(area) = entity.area {
            push_csv_row(&mut rows, &target, "area", &format_number(area), &u_area);
        }
        if let Some(radius) = entity.radius {
            push_csv_row(&mut rows, &target, "radius", &format_number(radius), u_lin);
        }
        if let Some(diameter) = entity.diameter {
            push_csv_row(
                &mut rows,
                &target,
                "diameter",
                &format_number(diameter),
                u_lin,
            );
        }
        if let Some(bounds) = &entity.bounding_box {
            push_csv_row(
                &mut rows,
                &target,
                "bbox_size",
                &format_point(bounds.size),
                u_lin,
            );
        }
    }

    for object in &result.objects {
        let name = scene
            .find(&object.key)
            .map_or(object.key.as_str(), |node| node.name.as_str());
        push_csv_row(&mut rows, name, "shape_type", &object.shape_type, "");
        if let Some(volume) = object.volume {
            push_csv_row(&mut rows, name, "volume", &format_number(volume), &u_vol);
        }
        if let Some(mass) = object.mass_g {
            push_csv_row(&mut rows, name, "mass", &format_number(mass), "g");
        }
        if let Some(area) = object.area {
            push_csv_row(&mut rows, name, "area", &format_number(area), &u_area);
        }
        if let Some(length) = object.length {
            push_csv_row(&mut rows, name, "length", &format_number(length), u_lin);
        }
        push_csv_row(
            &mut rows,
            name,
            "center_of_mass",
            &format_point(object.center),
            u_lin,
        );
        if let Some(bounds) = &object.bounding_box {
            push_csv_row(
                &mut rows,
                name,
                "bbox_size",
                &format_point(bounds.size),
                u_lin,
            );
            push_csv_row(
                &mut rows,
                name,
                "bbox_min",
                &format_point(bounds.min),
                u_lin,
            );
            push_csv_row(
                &mut rows,
                name,
                "bbox_max",
                &format_point(bounds.max),
                u_lin,
            );
        }
    }

    rows.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{format_number, measurement_of};
    use crate::scene::{TopologyEntity, TopologySelection};
    use crate::worker::protocol::{EntityMeasurement, Measurement, TopologyIdentity, TopologyKind};

    #[test]
    fn the_property_sheet_finds_the_measurement_of_its_selection() {
        let entity = |kind, identifier| EntityMeasurement {
            key: "s1-n1".into(),
            part: 0,
            kind,
            identifier,
            geometry_type: "plane".into(),
            center: [0.0; 3],
            position: None,
            length: None,
            area: Some(4.0),
            radius: None,
            diameter: None,
            bounding_box: None,
        };
        let result = Measurement {
            id: 1,
            identity: TopologyIdentity {
                scope: "render".into(),
                persistent: false,
            },
            entities: vec![entity(TopologyKind::Face, 6), entity(TopologyKind::Edge, 6)],
            pair: None,
            objects: Vec::new(),
            bounding_box: None,
        };
        let selection = |entity| TopologySelection {
            node: "s1-n1".into(),
            part: 0,
            entity,
            position: Default::default(),
        };

        let edge = measurement_of(&result, &selection(TopologyEntity::Edge(6)));
        assert_eq!(edge.map(|e| e.kind), Some(TopologyKind::Edge));
        assert!(measurement_of(&result, &selection(TopologyEntity::Face(7))).is_none());
    }

    #[test]
    fn numeric_display_keeps_precision_without_noise() {
        assert_eq!(format_number(5.0), "5");
        assert_eq!(format_number(1.2345678912), "1.234567891");
        assert_eq!(format_number(0.000_001), "1.00000000e-6");
    }

    #[test]
    fn measurements_format_as_structured_text_json_and_csv() {
        use super::{MeasurementFormat, format_measurement};
        use crate::scene::Scene;
        use crate::settings::Units;
        use crate::worker::protocol::PairMeasurement;

        let result = Measurement {
            id: 1,
            identity: TopologyIdentity {
                scope: "render".into(),
                persistent: false,
            },
            entities: vec![EntityMeasurement {
                key: "s1-n1".into(),
                part: 0,
                kind: TopologyKind::Face,
                identifier: 2,
                geometry_type: "plane".into(),
                center: [0.0, 0.0, 5.0],
                position: None,
                length: None,
                area: Some(120.5),
                radius: None,
                diameter: None,
                bounding_box: None,
            }],
            pair: Some(PairMeasurement {
                distance: 25.4,
                first_point: [0.0, 0.0, 0.0],
                second_point: [0.0, 0.0, 25.4],
                angle_degrees: Some(90.0),
                angle_kind: Some("face_face".into()),
            }),
            objects: Vec::new(),
            bounding_box: None,
        };
        let scene = Scene::default();

        let text = format_measurement(&result, &scene, Units::Millimeters, MeasurementFormat::Text);
        assert!(text.contains("Distance: 25.4 mm"));
        assert!(text.contains("Delta: (0, 0, 25.4) mm"));
        assert!(text.contains("Angle: 90°"));
        assert!(text.contains("Area: 120.5 mm²"));

        let json_str =
            format_measurement(&result, &scene, Units::Millimeters, MeasurementFormat::Json);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["unit"], "mm");
        assert_eq!(parsed["area_unit"], "mm²");
        assert_eq!(parsed["pair"]["distance"], 25.4);
        assert_eq!(parsed["entities"][0]["area"], 120.5);

        let csv = format_measurement(&result, &scene, Units::Millimeters, MeasurementFormat::Csv);
        assert!(csv.starts_with("target,property,value,unit\n"));
        assert!(csv.contains("pair,distance,25.4,mm"));
        assert!(csv.contains("s1-n1:Face 2,area,120.5,mm²"));
    }
}
