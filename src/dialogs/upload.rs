//! Uploading models to online services (manufacturing, analysis, ...).

use egui_phosphor::regular as icon;

use crate::commands::Action;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileType {
    Step,
    Stl,
    /// The script itself.
    CadQuery,
}

impl FileType {
    const ALL: [FileType; 3] = [FileType::Step, FileType::Stl, FileType::CadQuery];

    fn label(self) -> &'static str {
        match self {
            FileType::Step => "STEP",
            FileType::Stl => "STL",
            FileType::CadQuery => "CadQuery script",
        }
    }

    /// The worker's name for it.
    fn wire_name(self) -> &'static str {
        match self {
            FileType::Step => "step",
            FileType::Stl => "stl",
            FileType::CadQuery => "py",
        }
    }
}

pub struct Preset {
    pub name: &'static str,
    pub url: &'static str,
    pub file_type: FileType,
    /// JSON field of the response holding a link for the user.
    pub result_key: &'static str,
}

pub const PRESETS: &[Preset] = &[Preset {
    name: "PCBWay",
    url: "https://www.pcbway.com/common/CadQueryUpFile",
    file_type: FileType::Step,
    result_key: "redirect",
}];

enum Outcome {
    Link { service: &'static str, url: String },
    Text(String),
    Error(String),
}

pub struct UploadDialog {
    url: String,
    file_type: FileType,
    preset: Option<&'static Preset>,
    pub busy: bool,
    outcome: Option<Outcome>,
}

impl Default for UploadDialog {
    fn default() -> Self {
        Self {
            url: String::new(),
            file_type: FileType::Step,
            preset: None,
            busy: false,
            outcome: None,
        }
    }
}

impl UploadDialog {
    pub fn finished(&mut self, status: u16, body: &str) {
        self.busy = false;
        let json: Option<serde_json::Value> = serde_json::from_str(body).ok();
        let link = self.preset.and_then(|preset| {
            json.as_ref()?
                .get(preset.result_key)?
                .as_str()
                .map(|url| Outcome::Link {
                    service: preset.name,
                    url: url.to_owned(),
                })
        });
        self.outcome = Some(link.unwrap_or_else(|| {
            let text = json
                .and_then(|v| serde_json::to_string_pretty(&v).ok())
                .unwrap_or_else(|| body.to_owned());
            Outcome::Text(format!("HTTP {status}\n{text}"))
        }));
    }

    pub fn failed(&mut self, message: &str) {
        self.busy = false;
        self.outcome = Some(Outcome::Error(message.to_owned()));
    }

    /// Returns `false` once the window was closed.
    pub fn window(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) -> bool {
        let mut open = true;
        egui::Window::new(format!("{} Upload Model", icon::UPLOAD_SIMPLE))
            .open(&mut open)
            .collapsible(false)
            .default_size([480.0, 320.0])
            .show(ctx, |ui| {
                egui::Grid::new("upload_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("URL");
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut self.url)
                                    .desired_width(320.0)
                                    .hint_text("https://…"),
                            )
                            .changed()
                        {
                            self.preset = None;
                        }
                        ui.end_row();

                        ui.label("Presets");
                        ui.horizontal(|ui| {
                            for preset in PRESETS {
                                if ui.button(preset.name).clicked() {
                                    self.url = preset.url.to_owned();
                                    self.file_type = preset.file_type;
                                    self.preset = Some(preset);
                                }
                            }
                        });
                        ui.end_row();

                        ui.label("File type");
                        egui::ComboBox::from_id_salt("upload_type")
                            .selected_text(self.file_type.label())
                            .show_ui(ui, |ui| {
                                for file_type in FileType::ALL {
                                    ui.selectable_value(
                                        &mut self.file_type,
                                        file_type,
                                        file_type.label(),
                                    );
                                }
                            });
                        ui.end_row();
                    });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ready = !self.url.trim().is_empty() && !self.busy;
                    if ui
                        .add_enabled(
                            ready,
                            egui::Button::new(format!("{} Upload", icon::UPLOAD_SIMPLE)),
                        )
                        .clicked()
                    {
                        self.busy = true;
                        self.outcome = None;
                        actions.push(Action::StartUpload {
                            url: self.url.trim().to_owned(),
                            file_type: self.file_type.wire_name().to_owned(),
                        });
                    }
                    if self.busy {
                        ui.spinner();
                        ui.weak("Uploading…");
                    }
                });
                ui.weak(
                    "The selected objects are uploaded, or all objects when nothing is selected.",
                );
                ui.separator();

                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| match &self.outcome {
                        None => {
                            ui.weak("The response will appear here");
                        }
                        Some(Outcome::Link { service, url }) => {
                            ui.label(format!(
                                "Click the link below to view your model on {service}'s website."
                            ));
                            ui.hyperlink(url);
                        }
                        Some(Outcome::Text(text)) => {
                            ui.add(
                                egui::Label::new(egui::RichText::new(text).monospace())
                                    .selectable(true),
                            );
                        }
                        Some(Outcome::Error(message)) => {
                            ui.colored_label(
                                ui.visuals().error_fg_color,
                                format!("Error: {message}"),
                            );
                        }
                    });
            });
        open
    }
}
