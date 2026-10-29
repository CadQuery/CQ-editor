//! Screenshot and turntable animation options (Tools > Screenshot…).

use egui_phosphor::regular as icon;

use crate::commands::Action;
use crate::viewer::capture::{
    AnimationFormat, CaptureOptions, MAX_FRAMES_PER_SECOND, MAX_IMAGE_SIDE, MAX_TURNTABLE_FRAMES,
    ResolutionPreset, TurntableOptions, aspect_ratio_label,
};

pub struct CaptureDialog {
    pub open: bool,
    pub options: CaptureOptions,
    pub turntable: TurntableOptions,
    /// The size typed for a custom size, kept while another is chosen.
    custom: [u32; 2],
}

impl Default for CaptureDialog {
    fn default() -> Self {
        Self {
            open: false,
            options: CaptureOptions::default(),
            turntable: TurntableOptions::default(),
            custom: [1920, 1080],
        }
    }
}

/// `view` is the size of the 3D view in pixels; `progress` the frames of a
/// turntable being exported.
pub fn window(
    ctx: &egui::Context,
    dialog: &mut CaptureDialog,
    view: [u32; 2],
    progress: Option<(u32, u32)>,
    actions: &mut Vec<Action>,
) {
    let mut open = dialog.open;
    egui::Window::new(format!("{} Screenshot", icon::CAMERA))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("capture_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    image_rows(ui, dialog, view);
                    ui.label("");
                    ui.horizontal(|ui| {
                        if ui
                            .button(format!("{} Save Image…", icon::CAMERA))
                            .on_hover_text("Save the view as a PNG image")
                            .clicked()
                        {
                            actions.push(Action::SaveImage);
                        }
                        if ui
                            .button(format!("{} Copy to Clipboard", icon::COPY))
                            .on_hover_text("Copy the rendered image directly to the clipboard")
                            .clicked()
                        {
                            actions.push(Action::CopyImage);
                        }
                    });
                    ui.end_row();
                });
            ui.separator();
            ui.strong("Turntable animation");
            egui::Grid::new("turntable_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    turntable_rows(ui, &mut dialog.turntable);
                    ui.label("");
                    match progress {
                        Some((written, frames)) => {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::ProgressBar::new(written as f32 / frames.max(1) as f32)
                                        .desired_width(160.0)
                                        .text(format!("{written}/{frames}")),
                                );
                                if ui.button("Cancel").clicked() {
                                    actions.push(Action::CancelTurntable);
                                }
                            });
                        }
                        None => {
                            if ui
                                .button(format!("{} Export Turntable…", icon::FILM_STRIP))
                                .on_hover_text(
                                    "One turn of the camera around the model, \
                                     from the current view, with the size above",
                                )
                                .clicked()
                            {
                                actions.push(Action::ExportTurntable);
                            }
                        }
                    }
                    ui.end_row();
                });
        });
    dialog.open = open;
}

fn image_rows(ui: &mut egui::Ui, dialog: &mut CaptureDialog, view: [u32; 2]) {
    let options = &mut dialog.options;
    ui.label("Size");
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            for scale in [1, 2, 4] {
                let selected = options.size.is_none() && options.scale == scale;
                if ui
                    .selectable_label(selected, format!("View ×{scale}"))
                    .on_hover_text("The view's size in pixels, times this")
                    .clicked()
                {
                    options.size = None;
                    options.scale = scale;
                }
            }
            if ui
                .selectable_label(options.size.is_some(), "Custom / Preset")
                .clicked()
            {
                options.size = Some(dialog.custom);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.weak("Presets:");
            for preset in ResolutionPreset::ALL {
                let selected = options.size == Some(preset.size());
                if ui
                    .selectable_label(selected, preset.short_label())
                    .on_hover_text(preset.label())
                    .clicked()
                {
                    options.size = Some(preset.size());
                    dialog.custom = preset.size();
                }
            }
        });
        if let Some(size) = &mut options.size {
            ui.horizontal(|ui| {
                for (index, side) in size.iter_mut().enumerate() {
                    if index == 1 {
                        ui.label("×");
                    }
                    ui.add(egui::DragValue::new(side).range(16..=MAX_IMAGE_SIDE));
                }
                ui.label("px");
            });
            dialog.custom = *size;
        }
        let [width, height] = options.image_size(view);
        let ratio = aspect_ratio_label([width, height]);
        ui.weak(format!("{width} × {height} pixels ({ratio})"));
    });
    ui.end_row();

    ui.label("Background");
    ui.checkbox(&mut options.transparent, "Transparent")
        .on_hover_text("Only the model; the background is left out of the PNG");
    ui.end_row();
}

fn turntable_rows(ui: &mut egui::Ui, turntable: &mut TurntableOptions) {
    ui.label("Frames");
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut turntable.frames).range(4..=MAX_TURNTABLE_FRAMES));
        ui.label("at");
        ui.add(
            egui::DragValue::new(&mut turntable.frames_per_second)
                .range(1..=MAX_FRAMES_PER_SECOND)
                .suffix(" fps"),
        );
        ui.weak(format!(
            "{:.1} s per turn",
            turntable.frames as f32 / turntable.frames_per_second.max(1) as f32
        ));
    });
    ui.end_row();

    ui.label("Direction");
    ui.horizontal(|ui| {
        ui.radio_value(&mut turntable.clockwise, false, "Counterclockwise");
        ui.radio_value(&mut turntable.clockwise, true, "Clockwise");
    })
    .response
    .on_hover_text("How the model turns, seen from above");
    ui.end_row();

    ui.label("Format");
    ui.vertical(|ui| {
        ui.radio_value(
            &mut turntable.format,
            AnimationFormat::AnimatedPng,
            "Animated PNG (one file, plays in browsers)",
        );
        ui.radio_value(
            &mut turntable.format,
            AnimationFormat::PngFiles,
            "PNG files (name_0001.png …, for video tools)",
        );
    });
    ui.end_row();
}
