//! The script's top-level numbers as spin boxes and sliders. A change
//! rewrites the literal in the script and renders again: the script stays
//! the source of truth, and undo works as for any edit.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::RangeInclusive;

use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::Action;
use crate::parameters::{self, Parameter};

#[derive(Default)]
pub struct ParametersState {
    /// The parameters of the text with this hash.
    parsed: Option<(u64, Vec<Parameter>)>,
    /// Reference values parsed from the document's saved/opened baseline.
    reference_parsed: Option<(u64, HashMap<String, f64>)>,
    /// Initial values of parameters added during the current editing session.
    session_initial: HashMap<String, f64>,
    /// Slider ranges of parameters without a commented range, chosen from
    /// their first value so that the slider does not move under a drag.
    ranges: HashMap<String, RangeInclusive<f64>>,
}

impl ParametersState {
    /// The parameters of `text`, parsed once per version of the text.
    pub fn of(&mut self, text: &str) -> &[Parameter] {
        let hash = text_hash(text);
        if self
            .parsed
            .as_ref()
            .is_none_or(|(parsed, _)| *parsed != hash)
        {
            self.parsed = Some((hash, parameters::parameters(text)));
        }
        &self.parsed.as_ref().expect("parsed above").1
    }

    /// Updates reference values from `baseline_text` (the opened or last-saved
    /// script) and records first-seen values for newly added parameters.
    pub fn update_references(&mut self, baseline_text: &str, current: &[Parameter]) {
        let hash = text_hash(baseline_text);
        if self
            .reference_parsed
            .as_ref()
            .is_none_or(|(parsed, _)| *parsed != hash)
        {
            let map: HashMap<String, f64> = parameters::parameters(baseline_text)
                .into_iter()
                .map(|parameter| (parameter.name, parameter.value))
                .collect();
            for (name, value) in &map {
                self.session_initial.insert(name.clone(), *value);
            }
            self.reference_parsed = Some((hash, map));
        }
        for parameter in current {
            self.session_initial
                .entry(parameter.name.clone())
                .or_insert(parameter.value);
        }
    }

    /// Reference value for `name` (from the saved baseline, or its first seen value).
    pub fn reference_value(&self, name: &str) -> Option<f64> {
        self.reference_parsed
            .as_ref()
            .and_then(|(_, map)| map.get(name).copied())
            .or_else(|| self.session_initial.get(name).copied())
    }

    /// True when `parameter` differs from its reference value.
    pub fn is_modified(&self, parameter: &Parameter) -> bool {
        self.reference_value(&parameter.name)
            .is_some_and(|reference| (reference - parameter.value).abs() > 1e-12)
    }

    /// True when any parameter in `current` differs from its reference value.
    pub fn any_modified(&self, current: &[Parameter]) -> bool {
        current.iter().any(|parameter| self.is_modified(parameter))
    }

    /// The slider range of `parameter`: its commented range, or one around
    /// its first value, widened to keep the current value inside.
    pub fn slider_range(&mut self, parameter: &Parameter) -> RangeInclusive<f64> {
        if let Some(range) = &parameter.range {
            return range.clone();
        }
        let value = parameter.value;
        let range = self
            .ranges
            .entry(parameter.name.clone())
            .or_insert_with(|| automatic_range(value, parameter.integer));
        if !range.contains(&value) {
            *range = range.start().min(value)..=range.end().max(value);
        }
        range.clone()
    }
}

fn text_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Zero to twice the value (or twice the value to zero), at least 10 wide.
fn automatic_range(value: f64, integer: bool) -> RangeInclusive<f64> {
    let span = (2.0 * value.abs()).max(10.0);
    let span = if integer { span.ceil() } else { span };
    if value < 0.0 {
        -span..=0.0
    } else if value == 0.0 {
        -span / 2.0..=span / 2.0
    } else {
        0.0..=span
    }
}

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    let mut actions = Vec::new();
    let editable = !state.is_debugging();
    // Those of the script F5 runs, which may be another tab's.
    let target = state.run_target();
    let (text, saved_text) = state.document_of(target).map_or_else(
        || (String::new(), String::new()),
        |document| (document.text.clone(), document.saved_text().to_owned()),
    );
    let found = state.parameters.of(&text).to_vec();
    state.parameters.update_references(&saved_text, &found);

    if found.is_empty() {
        super::empty_state(
            ui,
            icon::SLIDERS_HORIZONTAL,
            "Top-level numbers of the script, such as width = 40, appear here",
        );
        return;
    }

    let any_modified = state.parameters.any_modified(&found);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_enabled_ui(editable, |ui| {
                if any_modified {
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(format!("{} Restore all", icon::ARROW_COUNTER_CLOCKWISE))
                                .on_hover_text(
                                    "Restore all modified parameters to their initial values",
                                )
                                .clicked()
                            {
                                actions.push(Action::ResetParameters);
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
                egui::Grid::new("parameters_grid")
                    .num_columns(4)
                    .spacing([10.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for parameter in &found {
                            row(ui, state, parameter, &mut actions);
                        }
                    });
            });
            ui.add_space(6.0);
            if editable {
                ui.weak(
                    "A change rewrites the number in the script and renders again. \
                     A comment such as # [10, 100] or # [0, 5, 0.5] sets the slider's \
                     range and step.",
                );
            } else {
                ui.weak("Parameters can be changed once debugging ends.");
            }
        });
    state.queue(actions);
}

fn row(ui: &mut egui::Ui, state: &mut AppState, parameter: &Parameter, actions: &mut Vec<Action>) {
    let modified = state.parameters.is_modified(parameter);
    let label = if modified {
        format!("{}*", parameter.name)
    } else {
        parameter.name.clone()
    };
    let name = ui
        .link(label)
        .on_hover_text(format!("Line {}: go to it", parameter.line));
    if name.clicked() {
        actions.push(Action::GoToLine(parameter.line));
    }

    let precision = parameter.precision();
    let mut value = parameter.value;
    let mut drag = egui::DragValue::new(&mut value)
        .speed(parameter.drag_speed())
        .min_decimals(precision)
        .max_decimals(precision);
    if let Some(range) = &parameter.range {
        drag = drag.range(range.clone());
    }
    let mut changed = ui.add(drag).changed();

    let range = state.parameters.slider_range(parameter);
    let mut slider = egui::Slider::new(&mut value, range).show_value(false);
    if parameter.integer {
        slider = slider.integer();
    } else if let Some(step) = parameter.step {
        slider = slider.step_by(step);
    }
    changed |= ui.add(slider).changed();

    if let Some(reference) = state
        .parameters
        .reference_value(&parameter.name)
        .filter(|_| modified)
    {
        if ui
            .small_button(icon::ARROW_COUNTER_CLOCKWISE)
            .on_hover_text(format!("Restore to {}", parameter.literal_for(reference)))
            .clicked()
        {
            actions.push(Action::SetParameter {
                name: parameter.name.clone(),
                line: parameter.line,
                value: reference,
            });
        }
    } else {
        ui.label("");
    }
    ui.end_row();

    if changed && value != parameter.value {
        actions.push(Action::SetParameter {
            name: parameter.name.clone(),
            line: parameter.line,
            value,
        });
    }
}

#[cfg(test)]
#[path = "../../tests/rust/panels/test_parameters.rs"]
mod tests;
