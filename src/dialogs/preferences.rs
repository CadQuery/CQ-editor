//! The preferences window. Changes apply immediately.

use egui_phosphor::regular as icon;

use crate::commands::Action;
use crate::settings::{
    EdgeStyle, Finish, Lighting, LightingPreset, OrbitMethod, Projection, Rendering, Settings,
    Theme,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Page {
    #[default]
    General,
    Viewer,
    Rendering,
    Presentation,
    Editor,
    Objects,
    Execution,
}

impl Page {
    const ALL: [Page; 7] = [
        Page::General,
        Page::Viewer,
        Page::Rendering,
        Page::Presentation,
        Page::Editor,
        Page::Objects,
        Page::Execution,
    ];

    fn label(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Viewer => "3D View",
            Page::Rendering => "Rendering",
            Page::Presentation => "Presentation",
            Page::Editor => "Editor",
            Page::Objects => "Objects",
            Page::Execution => "Execution",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Page::General => icon::SLIDERS_HORIZONTAL,
            Page::Viewer => icon::CUBE,
            Page::Rendering => icon::SUN,
            Page::Presentation => icon::SPARKLE,
            Page::Editor => icon::CODE,
            Page::Objects => icon::TREE_STRUCTURE,
            Page::Execution => icon::PLAY,
        }
    }
}

pub fn window(
    ctx: &egui::Context,
    open: &mut bool,
    page: &mut Page,
    settings: &mut Settings,
    actions: &mut Vec<Action>,
) {
    egui::Window::new(format!("{} Preferences", icon::GEAR_SIX))
        .open(open)
        .collapsible(false)
        .resizable(true)
        .default_size([620.0, 420.0])
        .show(ctx, |ui| {
            egui::Panel::left("preferences_pages")
                .resizable(false)
                .exact_size(150.0)
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    for candidate in Page::ALL {
                        let text = format!("{}  {}", candidate.icon(), candidate.label());
                        if ui.selectable_label(*page == candidate, text).clicked() {
                            *page = candidate;
                        }
                    }
                });
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                    left: 16,
                    ..Default::default()
                }))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.heading(page.label());
                            ui.add_space(6.0);
                            egui::Grid::new(("preferences", *page))
                                .num_columns(2)
                                .spacing([18.0, 10.0])
                                .striped(false)
                                .show(ui, |ui| match page {
                                    Page::General => general(ui, settings, actions),
                                    Page::Viewer => viewer(ui, settings),
                                    Page::Rendering => rendering(ui, settings),
                                    Page::Presentation => presentation(ui, settings),
                                    Page::Editor => editor(ui, settings),
                                    Page::Objects => objects(ui, settings),
                                    Page::Execution => execution(ui, settings),
                                });
                            ui.add_space(12.0);
                            if ui
                                .button(format!(
                                    "{} Restore defaults",
                                    icon::ARROW_COUNTER_CLOCKWISE
                                ))
                                .clicked()
                            {
                                restore_defaults(*page, settings);
                            }
                        });
                });
        });
}

fn restore_defaults(page: Page, settings: &mut Settings) {
    let defaults = Settings::default();
    match page {
        Page::General => settings.general = defaults.general,
        Page::Viewer => settings.viewer = defaults.viewer,
        // Presentation has a page of its own.
        Page::Rendering => {
            settings.rendering = Rendering {
                presentation: settings.rendering.presentation.clone(),
                ..defaults.rendering
            }
        }
        Page::Presentation => settings.rendering.presentation = defaults.rendering.presentation,
        Page::Editor => settings.editor = defaults.editor,
        Page::Objects => settings.objects = defaults.objects,
        Page::Execution => settings.execution = defaults.execution,
    }
}

fn row(ui: &mut egui::Ui, label: &str, hint: &str, add: impl FnOnce(&mut egui::Ui)) {
    let response = ui.label(label);
    if !hint.is_empty() {
        response.on_hover_text(hint);
    }
    add(ui);
    ui.end_row();
}

fn general(ui: &mut egui::Ui, settings: &mut Settings, actions: &mut Vec<Action>) {
    let general = &mut settings.general;
    row(ui, "Theme", "", |ui| {
        egui::ComboBox::from_id_salt("theme")
            .selected_text(general.theme.label())
            .show_ui(ui, |ui| {
                for theme in Theme::ALL {
                    ui.selectable_value(&mut general.theme, theme, theme.label());
                }
            });
    });
    row(
        ui,
        "Check for updates",
        "Look for a newer CQ-editor on GitHub once a day, when Python starts; \
         only the releases list is fetched, nothing is sent about you",
        |ui| {
            ui.checkbox(&mut general.check_updates, "");
        },
    );
    row(
        ui,
        "Interface scale",
        "Zoom of the whole interface (Ctrl + / Ctrl - also work)",
        |ui| {
            ui.add(egui::Slider::new(&mut general.ui_scale, 0.75..=2.0).step_by(0.05));
        },
    );
    row(
        ui,
        "Python interpreter",
        "A Python with CadQuery installed. Leave empty to detect it: CQ_EDITOR_PYTHON, \
         the active virtual/conda environment, a .venv folder, then python on PATH.",
        |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut general.python)
                            .hint_text("auto-detect")
                            .desired_width(260.0),
                    );
                    if ui
                        .button(icon::FOLDER_OPEN)
                        .on_hover_text("Browse…")
                        .clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .set_title("Python interpreter")
                            .pick_file()
                    {
                        general.python = path.display().to_string();
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .button(format!(
                            "{} Restart Python with this interpreter",
                            icon::ARROW_CLOCKWISE
                        ))
                        .clicked()
                    {
                        actions.push(Action::RestartPython);
                    }
                    ui.menu_button(format!("{} Detected", icon::LIST_BULLETS), |ui| {
                        let candidates = crate::worker::launch::candidates();
                        if candidates.is_empty() {
                            ui.weak("No Python found");
                        }
                        for candidate in candidates {
                            let text = candidate.display().to_string();
                            if ui.button(&text).clicked() {
                                general.python = text;
                                ui.close();
                            }
                        }
                    })
                    .response
                    .on_hover_text("Pythons found on this computer");
                });
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("{} Create Environment…", icon::PLUS_CIRCLE))
                        .on_hover_text(
                            "A new virtual environment with CadQuery and jedi, \
                             to use with CQ-editor",
                        )
                        .clicked()
                    {
                        actions.push(Action::CreateEnvironment);
                    }
                    if ui
                        .button(format!("{} Install Packages…", icon::PACKAGE))
                        .on_hover_text("pip install into the Python in use")
                        .clicked()
                    {
                        actions.push(Action::InstallPackages);
                    }
                });
            });
        },
    );
}

fn color_row(ui: &mut egui::Ui, label: &str, color: &mut [u8; 3]) {
    row(ui, label, "", |ui| {
        ui.color_edit_button_srgb(color);
    });
}

fn viewer(ui: &mut egui::Ui, settings: &mut Settings) {
    let viewer = &mut settings.viewer;
    row(
        ui,
        "Fit automatically",
        "Frame the model after every run",
        |ui| {
            ui.checkbox(&mut viewer.fit_automatically, "");
        },
    );
    row(ui, "Show navigation cube", "", |ui| {
        ui.checkbox(&mut viewer.show_navigation_cube, "");
    });
    row(ui, "Show axes indicator", "", |ui| {
        ui.checkbox(&mut viewer.show_axes_indicator, "");
    });
    row(ui, "Animate view changes", "", |ui| {
        ui.checkbox(&mut viewer.animate, "");
    });
    row(
        ui,
        "Projection",
        "Stereo shows two eyes as the stereo mode says; the mono eyes show one of them",
        |ui| {
            egui::ComboBox::from_id_salt("projection")
                .selected_text(viewer.projection.label())
                .show_ui(ui, |ui| {
                    for projection in Projection::ALL {
                        ui.selectable_value(&mut viewer.projection, projection, projection.label());
                    }
                });
        },
    );
    row(
        ui,
        "Stereo mode",
        "How the Stereo projection shows its two images. Quad Buffer needs a stereo \
         display this renderer cannot drive, so it shows the red-cyan anaglyph",
        |ui| {
            ui.add_enabled_ui(viewer.projection == Projection::Stereo, |ui| {
                egui::ComboBox::from_id_salt("stereo_mode")
                    .selected_text(viewer.stereo_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in crate::settings::StereoMode::ALL {
                            ui.selectable_value(&mut viewer.stereo_mode, mode, mode.label());
                        }
                    });
            });
        },
    );
    row(
        ui,
        "Orbit method",
        "Turntable keeps Z up; Trackball rotates freely",
        |ui| {
            egui::ComboBox::from_id_salt("orbit")
                .selected_text(viewer.orbit_method.label())
                .show_ui(ui, |ui| {
                    for method in OrbitMethod::ALL {
                        ui.selectable_value(&mut viewer.orbit_method, method, method.label());
                    }
                });
        },
    );
    color_row(ui, "Background color", &mut viewer.background);
    row(
        ui,
        "Use gradient",
        "Blend to the second color at the bottom",
        |ui| {
            ui.checkbox(&mut viewer.use_gradient, "");
        },
    );
    color_row(ui, "Background color (aux)", &mut viewer.background_aux);
    color_row(ui, "Default object color", &mut viewer.default_color);
    row(ui, "Show edges", "", |ui| {
        ui.checkbox(&mut viewer.show_edges, "");
    });
    row(
        ui,
        "Hidden edges",
        "Show the edges behind faces as faint dashes in the Hidden Line display mode",
        |ui| {
            ui.checkbox(&mut viewer.hidden_edges, "");
        },
    );
    color_row(ui, "Edge color", &mut viewer.edge_color);
    row(
        ui,
        "Ground grid",
        "A grid on the XY plane (View > Ground Grid)",
        |ui| {
            ui.checkbox(&mut viewer.ground_grid, "");
        },
    );
    row(
        ui,
        "Scale bar",
        "A round length at the bottom right of the view",
        |ui| {
            ui.checkbox(&mut viewer.scale_bar, "");
        },
    );
    row(
        ui,
        "Units",
        "What one model unit means in the scale bar and exported files; CadQuery itself has no units",
        |ui| {
            egui::ComboBox::from_id_salt("units")
                .selected_text(viewer.units.label())
                .show_ui(ui, |ui| {
                    for units in crate::settings::Units::ALL {
                        ui.selectable_value(&mut viewer.units, units, units.label());
                    }
                });
        },
    );
    row(
        ui,
        "Large models",
        "Draw parts that are small on screen with a simplified mesh: automatically from 2 million triangles shown, always, or never",
        |ui| {
            egui::ComboBox::from_id_salt("large_models")
                .selected_text(viewer.large_models.label())
                .show_ui(ui, |ui| {
                    for mode in crate::settings::LargeModels::ALL {
                        ui.selectable_value(&mut viewer.large_models, mode, mode.label());
                    }
                });
        },
    );
    row(ui, "Edge width", "In pixels", |ui| {
        ui.add(egui::Slider::new(&mut viewer.edge_width, 0.5..=5.0).step_by(0.25));
    });
    row(
        ui,
        "Deviation",
        "Mesh chordal deviation relative to the object size; smaller is finer (applies on the next run)",
        |ui| {
            ui.add(
                egui::DragValue::new(&mut viewer.linear_tolerance)
                    .range(1e-5..=0.05)
                    .speed(1e-5)
                    .max_decimals(5),
            );
        },
    );
    row(
        ui,
        "Angular deviation",
        "Mesh angular deviation in radians; smaller is finer (applies on the next run)",
        |ui| {
            ui.add(
                egui::DragValue::new(&mut viewer.angular_tolerance)
                    .range(0.02..=1.0)
                    .speed(0.005)
                    .max_decimals(3),
            );
        },
    );
    row(
        ui,
        "Selection mode shortcuts",
        "Keys that switch between Auto, Object, Face, Edge and Vertex selection modes \
         when no text field has keyboard focus (Alt+1..Alt+5 also work anywhere)",
        |ui| {
            ui.horizontal_wrapped(|ui| {
                for mode in crate::viewer::SelectionMode::ALL {
                    ui.label(format!("{}:", mode.label()));
                    ui.add(
                        egui::TextEdit::singleline(viewer.selection_shortcuts.get_mut(mode))
                            .desired_width(48.0),
                    );
                }
            });
        },
    );
}

fn rendering(ui: &mut egui::Ui, settings: &mut Settings) {
    let rendering = &mut settings.rendering;
    lighting_rows(ui, "lighting", &mut rendering.lighting);
    row(
        ui,
        "Edges",
        "Which edges the Shaded, X-Ray and Hidden Line modes draw (when shown at all, see \
         3D View > Show edges). Sharp edges leaves out the edges between faces that continue \
         smoothly, such as a fillet's borders and the seams of cylinders; Outline adds the \
         outline of curved surfaces, as a drawing has it. Wireframe always draws every edge",
        |ui| {
            egui::ComboBox::from_id_salt("edge_style")
                .selected_text(rendering.edges.label())
                .show_ui(ui, |ui| {
                    for style in EdgeStyle::ALL {
                        ui.selectable_value(&mut rendering.edges, style, style.label());
                    }
                });
        },
    );
    row(
        ui,
        "Sharp edge angle",
        "Faces meeting at less than this many degrees continue smoothly: Sharp edges and \
         Outline leave the edge between them out",
        |ui| {
            ui.add_enabled(
                rendering.edges != EdgeStyle::All,
                egui::Slider::new(&mut rendering.sharp_angle, 1.0..=90.0)
                    .step_by(1.0)
                    .suffix("°"),
            );
        },
    );
}

/// The Presentation display mode's own look.
fn presentation(ui: &mut egui::Ui, settings: &mut Settings) {
    let presentation = &mut settings.rendering.presentation;
    row(
        ui,
        "Material",
        "The material of objects whose script and Objects panel choose none",
        |ui| {
            egui::ComboBox::from_id_salt("presentation_material")
                .selected_text(presentation.material.label())
                .show_ui(ui, |ui| {
                    for finish in Finish::ALL {
                        ui.selectable_value(&mut presentation.material, finish, finish.label());
                    }
                });
        },
    );
    row(
        ui,
        "Show edges",
        "Off, the shapes speak alone; each object can still override it in the Objects panel",
        |ui| {
            ui.checkbox(&mut presentation.show_edges, "");
        },
    );
    row(
        ui,
        "Edges",
        "Which edges, when they are shown: Outline gives the contour of curved surfaces \
         and the sharp edges, as an illustration has them",
        |ui| {
            ui.add_enabled_ui(presentation.show_edges, |ui| {
                egui::ComboBox::from_id_salt("presentation_edges")
                    .selected_text(presentation.edges.label())
                    .show_ui(ui, |ui| {
                        for style in EdgeStyle::ALL {
                            ui.selectable_value(&mut presentation.edges, style, style.label());
                        }
                    });
            });
        },
    );
    color_row(ui, "Background, top", &mut presentation.background_top);
    color_row(
        ui,
        "Background, bottom",
        &mut presentation.background_bottom,
    );
    row(
        ui,
        "Axes and grid",
        "Draw the axes, the ground grid and the scale bar in this mode too",
        |ui| {
            ui.checkbox(&mut presentation.helpers, "");
        },
    );
    row(
        ui,
        "Shadow",
        "A soft shadow on the ground under the model, as in a product photograph",
        |ui| {
            ui.checkbox(&mut presentation.shadow, "");
        },
    );
    let shadow = presentation.shadow;
    row(
        ui,
        "Shadow strength",
        "How dark the shadow is where the model touches the ground",
        |ui| {
            ui.add_enabled(
                shadow,
                egui::Slider::new(&mut presentation.shadow_strength, 0.0..=1.0).step_by(0.01),
            );
        },
    );
    row(
        ui,
        "Shadow softness",
        "How far the shadow spreads around the model: 0 is crisp, 1 very soft",
        |ui| {
            ui.add_enabled(
                shadow,
                egui::Slider::new(&mut presentation.shadow_softness, 0.0..=1.0).step_by(0.01),
            );
        },
    );
    lighting_rows(ui, "presentation_lighting", &mut presentation.lighting);
}

/// The rows of one set of lights, starting with a preset.
fn lighting_rows(ui: &mut egui::Ui, id: &str, lighting: &mut Lighting) {
    row(
        ui,
        "Lighting",
        "A starting point; the values below fine-tune it",
        |ui| {
            let current = lighting.preset().map_or("Custom", LightingPreset::label);
            egui::ComboBox::from_id_salt((id, "preset"))
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for preset in LightingPreset::ALL {
                        if ui
                            .selectable_label(lighting.preset() == Some(preset), preset.label())
                            .clicked()
                        {
                            *lighting = preset.lighting();
                        }
                    }
                });
        },
    );
    let mut slider = |label: &str, hint: &str, value: &mut f32, range, step: f64| {
        row(ui, label, hint, |ui| {
            ui.add(egui::Slider::new(value, range).step_by(step));
        });
    };
    slider(
        "Key light",
        "The main light",
        &mut lighting.key,
        0.0..=1.5,
        0.01,
    );
    slider(
        "Fill light",
        "A softer light from the lower right, which keeps shadows readable",
        &mut lighting.fill,
        0.0..=1.0,
        0.01,
    );
    slider(
        "Ambient light",
        "Light from everywhere: the darkest a face can be",
        &mut lighting.ambient,
        0.0..=1.0,
        0.01,
    );
    slider(
        "Rim light",
        "Light grazing the outline of curved surfaces",
        &mut lighting.rim,
        0.0..=1.0,
        0.01,
    );
    slider(
        "Highlights",
        "Strength of the reflections of the key light",
        &mut lighting.specular,
        0.0..=1.0,
        0.01,
    );
    slider(
        "Shininess",
        "Higher gives smaller, sharper highlights, as on a glossy surface",
        &mut lighting.shininess,
        2.0..=256.0,
        1.0,
    );
    slider(
        "Light direction",
        "Where the key light comes from, in degrees around the line of sight: negative is left",
        &mut lighting.azimuth,
        -180.0..=180.0,
        1.0,
    );
    slider(
        "Light height",
        "How far above the line of sight the key light is, in degrees",
        &mut lighting.elevation,
        -90.0..=90.0,
        1.0,
    );
    slider(
        "Exposure",
        "Overall brightness in photographic stops: +1 doubles the light",
        &mut lighting.exposure,
        -3.0..=3.0,
        0.05,
    );
    slider(
        "Contrast",
        "Above 1 darkens the shadows and brightens the lights; below 1 flattens them",
        &mut lighting.contrast,
        0.5..=2.0,
        0.01,
    );
}

fn editor(ui: &mut egui::Ui, settings: &mut Settings) {
    let editor = &mut settings.editor;
    row(ui, "Font size", "", |ui| {
        ui.add(egui::Slider::new(&mut editor.font_size, 8.0..=28.0).step_by(1.0));
    });
    row(ui, "Line wrap", "", |ui| {
        ui.checkbox(&mut editor.line_wrap, "");
    });
    row(
        ui,
        "Maximum line length",
        "Column of the vertical ruler (0 hides it)",
        |ui| {
            ui.add(egui::DragValue::new(&mut editor.max_line_length).range(0..=300));
        },
    );
    row(ui, "Highlight current line", "", |ui| {
        ui.checkbox(&mut editor.highlight_current_line, "");
    });
    row(
        ui,
        "Auto-indent",
        "Keep and extend indentation on Enter",
        |ui| {
            ui.checkbox(&mut editor.auto_indent, "");
        },
    );
    row(
        ui,
        "Complete names after",
        "Offer completions once this many characters of a name are typed (0: only after . and on Ctrl+Space)",
        |ui| {
            ui.add(
                egui::DragValue::new(&mut editor.completion_min_chars)
                    .range(0..=10)
                    .suffix(" characters"),
            );
        },
    );
    row(
        ui,
        "Autoreload",
        "Re-run the script whenever its file changes on disk",
        |ui| {
            ui.checkbox(&mut editor.autoreload, "");
        },
    );
    row(
        ui,
        "Autoreload delay",
        "Wait after a change before reloading, in milliseconds",
        |ui| {
            ui.add(
                egui::DragValue::new(&mut editor.autoreload_delay_ms)
                    .range(0..=5000)
                    .suffix(" ms"),
            );
        },
    );
    row(
        ui,
        "Autoreload: watch imported modules",
        "Also re-run when local modules the script imports change",
        |ui| {
            ui.checkbox(&mut editor.watch_imported_modules, "");
        },
    );
}

fn objects(ui: &mut egui::Ui, settings: &mut Settings) {
    let objects = &mut settings.objects;
    row(
        ui,
        "Preserve properties on reload",
        "Keep visibility and colors of objects (matched by name)",
        |ui| {
            ui.checkbox(&mut objects.preserve_properties, "");
        },
    );
    row(ui, "Clear all before each run", "", |ui| {
        ui.checkbox(&mut objects.clear_before_run, "");
    });
    row(
        ui,
        "Merge assemblies",
        "Show an assembly as one object instead of one per part",
        |ui| {
            ui.checkbox(&mut objects.merge_assemblies, "");
        },
    );
    row(
        ui,
        "STL precision",
        "Chordal tolerance of STL exports, in model units",
        |ui| {
            ui.add(
                egui::DragValue::new(&mut objects.stl_tolerance)
                    .range(1e-4..=10.0)
                    .speed(0.001)
                    .max_decimals(4),
            );
        },
    );
    row(
        ui,
        "STL angular precision",
        "Angular tolerance of STL exports, in radians",
        |ui| {
            ui.add(
                egui::DragValue::new(&mut objects.stl_angular_tolerance)
                    .range(0.01..=1.0)
                    .speed(0.005)
                    .max_decimals(3),
            );
        },
    );
}

fn execution(ui: &mut egui::Ui, settings: &mut Settings) {
    let execution = &mut settings.execution;
    row(
        ui,
        "Reload CQ",
        "Re-import CadQuery before each run (for CadQuery development)",
        |ui| {
            ui.checkbox(&mut execution.reload_cq, "");
        },
    );
    row(
        ui,
        "Add script dir to path",
        "So scripts can import modules next to them",
        |ui| {
            ui.checkbox(&mut execution.add_script_dir_to_path, "");
        },
    );
    row(
        ui,
        "Change working dir to script dir",
        "So relative file paths resolve next to the script",
        |ui| {
            ui.checkbox(&mut execution.change_working_dir, "");
        },
    );
    row(
        ui,
        "Reload imported modules",
        "Pick up changes to local modules on every run",
        |ui| {
            ui.checkbox(&mut execution.reload_imported_modules, "");
        },
    );
}
