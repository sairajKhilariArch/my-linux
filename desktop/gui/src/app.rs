//! The native graphical application.
//!
//! Every screen is a thin view over [`GuiState`], which calls the same
//! `desktop-core` functions as the CLI. There is no second theme engine here.

use std::path::{Path, PathBuf};

use desktop_core::ThemeColors;
use desktop_core::assets::AssetSettings;
use desktop_core::format_timestamp;
use desktop_core::transaction::FileState;
use eframe::egui::{self, Color32, RichText, Ui};

use crate::state::{GuiState, ThemeCard, display_name, format_bytes};

const ACCENT: Color32 = Color32::from_rgb(0, 230, 255);
const GOOD: Color32 = Color32::from_rgb(0x6e, 0xbe, 0x5a);
const BAD: Color32 = Color32::from_rgb(0xff, 0x6b, 0x6b);
const WARN: Color32 = Color32::from_rgb(0xff, 0xc2, 0x4b);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    Gallery,
    Editor,
    Applications,
    Wallpapers,
    Profiles,
    Recovery,
}

/// Editable copy of a theme's colours.
struct DraftTheme {
    base: ThemeCard,
    id: String,
    name: String,
    background: [u8; 3],
    surface: [u8; 3],
    primary: [u8; 3],
    secondary: [u8; 3],
    text: [u8; 3],
    border: [u8; 3],
    success: [u8; 3],
    warning: [u8; 3],
    error: [u8; 3],
}

impl DraftTheme {
    fn new(base: &ThemeCard) -> Self {
        Self {
            base: base.clone(),
            id: format!("{}-custom", base.id),
            name: format!("{} Custom", base.name),
            background: to_rgb(base.colors.background),
            surface: to_rgb(base.colors.surface),
            primary: to_rgb(base.colors.primary),
            secondary: to_rgb(base.colors.secondary),
            text: to_rgb(base.colors.text),
            border: to_rgb(base.colors.border),
            success: to_rgb(base.colors.success),
            warning: to_rgb(base.colors.warning),
            error: to_rgb(base.colors.error),
        }
    }

    /// Builds a complete palette; the component states are derived from the
    /// colours the user edits.
    fn colors(&self) -> ThemeColors {
        let background = from_rgb(self.background);
        let surface = from_rgb(self.surface);
        let primary = from_rgb(self.primary);
        let text = from_rgb(self.text);

        ThemeColors {
            background,
            surface,
            surface_alt: surface.lighten(0.04),
            primary,
            secondary: from_rgb(self.secondary),
            text,
            text_muted: text.mix(background, 0.45),
            border: from_rgb(self.border),
            success: from_rgb(self.success),
            warning: from_rgb(self.warning),
            error: from_rgb(self.error),
            active: primary.lighten(0.10),
            hover: surface.lighten(0.08),
            selected: primary.darken(0.18),
        }
    }
}

fn to_rgb(color: desktop_core::Color) -> [u8; 3] {
    [color.red, color.green, color.blue]
}

fn from_rgb(rgb: [u8; 3]) -> desktop_core::Color {
    desktop_core::Color {
        red: rgb[0],
        green: rgb[1],
        blue: rgb[2],
    }
}

fn color32(color: desktop_core::Color) -> Color32 {
    Color32::from_rgb(color.red, color.green, color.blue)
}

fn sanitize_id(input: &str) -> String {
    let mut id: String = input
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();

    while id.contains("--") {
        id = id.replace("--", "-");
    }

    id.trim_matches('-').to_string()
}

pub struct MyLinuxApp {
    state: GuiState,
    tab: Tab,
    dark: bool,
    status: Option<(bool, String)>,
    filter: String,
    selected_theme: Option<String>,
    draft: Option<DraftTheme>,
    import_path: String,
    export_path: String,
    wallpaper_palette: Option<desktop_core::assets::Proposal>,
    palette_path: Option<PathBuf>,
    palette_id: String,
    palette_name: String,
    enabled: Vec<&'static str>,
    new_profile_id: String,
    new_profile_name: String,
    new_profile_theme: String,
    asset_font: String,
    asset_font_size: u32,
    asset_icon: String,
    asset_cursor: String,
    asset_cursor_size: u32,
}

impl MyLinuxApp {
    pub fn new(cc: &eframe::CreationContext<'_>, root: PathBuf) -> Self {
        let state = GuiState::new(root);
        let enabled = state
            .integrations
            .iter()
            .filter(|row| row.available())
            .map(|row| row.id)
            .collect();
        let new_profile_theme = state
            .themes
            .first()
            .map(|theme| theme.id.clone())
            .unwrap_or_default();
        let selected_theme = state
            .active_theme
            .clone()
            .or_else(|| state.themes.first().map(|theme| theme.id.clone()));

        let mut app = Self {
            state,
            tab: Tab::Dashboard,
            dark: true,
            status: None,
            filter: String::new(),
            selected_theme,
            draft: None,
            import_path: String::new(),
            export_path: String::new(),
            wallpaper_palette: None,
            palette_path: None,
            palette_id: String::new(),
            palette_name: String::new(),
            enabled,
            new_profile_id: String::new(),
            new_profile_name: String::new(),
            new_profile_theme,
            asset_font: String::new(),
            asset_font_size: 11,
            asset_icon: String::new(),
            asset_cursor: String::new(),
            asset_cursor_size: 24,
        };

        app.sync_assets();
        app.apply_visuals(&cc.egui_ctx);

        app
    }

    fn apply_visuals(&self, ctx: &egui::Context) {
        ctx.set_visuals(if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(10.0, 5.0);
            style
                .text_styles
                .insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        });
    }

    fn sync_assets(&mut self) {
        self.asset_font = self.state.assets.font.clone().unwrap_or_default();
        self.asset_font_size = self.state.assets.font_size;
        self.asset_icon = self.state.assets.icon_theme.clone().unwrap_or_default();
        self.asset_cursor = self.state.assets.cursor_theme.clone().unwrap_or_default();
        self.asset_cursor_size = self.state.assets.cursor_size;
    }

    fn refresh(&mut self) {
        self.state.refresh();
        self.sync_assets();

        if self.selected_theme.is_none() {
            self.selected_theme = self.state.themes.first().map(|theme| theme.id.clone());
        }

        self.status = Some((true, "Installation state refreshed.".to_string()));
    }

    fn outcome(&mut self, result: Result<String, String>) {
        self.status = Some(match result {
            Ok(message) => (true, message),
            Err(error) => (false, error),
        });
    }

    fn apply_selected_integrations(&mut self, theme_id: &str) {
        let enabled = self.enabled.clone();
        let result = self.state.apply_theme(theme_id, &enabled);

        self.outcome(result);
    }

    fn start_editor(&mut self, theme_id: &str) {
        if let Some(card) = self
            .state
            .themes
            .iter()
            .find(|theme| theme.id == theme_id)
            .cloned()
        {
            self.draft = Some(DraftTheme::new(&card));
            self.tab = Tab::Editor;
        }
    }

    fn header(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("My Linux").size(22.0).strong().color(ACCENT));
            ui.label(RichText::new("desktop customization").weak());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Refresh").clicked() {
                    self.refresh();
                }

                let label = if self.dark { "Light mode" } else { "Dark mode" };

                if ui.button(label).clicked() {
                    self.dark = !self.dark;
                    let ctx = ui.ctx().clone();
                    self.apply_visuals(&ctx);
                }

                ui.label(
                    RichText::new(self.state.root.display().to_string())
                        .small()
                        .weak(),
                );
            });
        });

        ui.add_space(2.0);

        ui.horizontal(|ui| {
            for (tab, label) in [
                (Tab::Dashboard, "Dashboard"),
                (Tab::Gallery, "Themes"),
                (Tab::Editor, "Theme editor"),
                (Tab::Applications, "Applications"),
                (Tab::Wallpapers, "Wallpapers"),
                (Tab::Profiles, "Profiles"),
                (Tab::Recovery, "Recovery"),
            ] {
                if ui.selectable_label(self.tab == tab, label).clicked() {
                    self.tab = tab;
                }
            }
        });

        ui.add_space(6.0);
    }

    fn status_line(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);

        match &self.status {
            Some((true, message)) => {
                ui.colored_label(GOOD, message);
            }
            Some((false, message)) => {
                ui.colored_label(BAD, message);
            }
            None => {
                ui.label(RichText::new("Ready").weak());
            }
        }

        ui.add_space(4.0);
    }

    fn dashboard(&mut self, ui: &mut Ui) {
        ui.heading("Dashboard");

        let available = self
            .state
            .integrations
            .iter()
            .filter(|row| row.available())
            .count();

        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            stat(
                ui,
                "Active theme",
                self.state.active_theme.as_deref().unwrap_or("none"),
            );
            stat(
                ui,
                "Active profile",
                self.state.active_profile.as_deref().unwrap_or("none"),
            );
            stat(
                ui,
                "Installed files",
                &self.state.modifications.len().to_string(),
            );
            stat(
                ui,
                "Integrations",
                &format!("{available} / {}", self.state.integrations.len()),
            );
            stat(ui, "Backups", &self.state.backups.len().to_string());
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        ui.group(|ui| {
            ui.label(RichText::new("Wallpaper").strong());

            match &self.state.wallpaper {
                Some(path) => {
                    ui.label(RichText::new(path.display().to_string()).small());

                    if !path.is_file() {
                        ui.colored_label(BAD, "The recorded wallpaper no longer exists.");
                    }
                }
                None => {
                    ui.label(RichText::new("No wallpaper recorded yet.").weak());
                }
            }

            ui.horizontal(|ui| {
                if ui.button("Open wallpaper manager").clicked() {
                    self.tab = Tab::Wallpapers;
                }

                ui.label(
                    RichText::new(format!(
                        "font {} {} · icons {} · cursor {}",
                        self.state.assets.font.as_deref().unwrap_or("system"),
                        self.state.assets.font_size,
                        self.state.assets.icon_theme.as_deref().unwrap_or("system"),
                        self.state
                            .assets
                            .cursor_theme
                            .as_deref()
                            .unwrap_or("system"),
                    ))
                    .small()
                    .weak(),
                );
            });
        });

        ui.add_space(10.0);
        ui.heading("Quick switch");

        let mut picked: Option<String> = None;

        ui.horizontal_wrapped(|ui| {
            for card in &self.state.themes {
                ui.vertical(|ui| {
                    let active = self.state.active_theme.as_deref() == Some(card.id.as_str());

                    if ui
                        .add_enabled(!active, egui::Button::new(&card.name))
                        .clicked()
                    {
                        picked = Some(card.id.clone());
                    }

                    swatches(ui, &card.colors);
                    ui.label(RichText::new(card.source).small().weak());
                });

                ui.add_space(8.0);
            }
        });

        if let Some(id) = picked {
            self.apply_selected_integrations(&id);
        }

        ui.add_space(10.0);
        ui.heading("Recent activity");

        if self.state.events.is_empty() {
            ui.label(RichText::new("No events recorded yet.").weak());
        } else {
            for event in self.state.events.iter().rev().take(6) {
                let color = match event.level {
                    desktop_core::log::Level::Info => GOOD,
                    desktop_core::log::Level::Warn => WARN,
                    desktop_core::log::Level::Error => BAD,
                };

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format_timestamp(event.timestamp))
                            .small()
                            .weak(),
                    );
                    ui.colored_label(color, RichText::new(&event.action).small().strong());
                    ui.label(RichText::new(&event.detail).small());
                });
            }
        }

        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui.button("Open recovery").clicked() {
                self.tab = Tab::Recovery;
            }

            if ui.button("Open theme gallery").clicked() {
                self.tab = Tab::Gallery;
            }
        });
    }

    fn gallery(&mut self, ui: &mut Ui) {
        ui.heading("Theme gallery");
        ui.label(
            RichText::new("Every theme is loaded from TOML; built-in themes are read-only.").weak(),
        );

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Filter");
            ui.text_edit_singleline(&mut self.filter);
            ui.label("Import from");
            ui.text_edit_singleline(&mut self.import_path);
            ui.label("Export to");
            ui.text_edit_singleline(&mut self.export_path);
        });

        ui.add_space(6.0);

        let mut apply: Option<String> = None;
        let mut edit: Option<String> = None;
        let mut export: Option<String> = None;
        let mut remove: Option<String> = None;
        let filter = self.filter.to_lowercase();

        for card in &self.state.themes {
            if !filter.is_empty()
                && !card.name.to_lowercase().contains(&filter)
                && !card.id.contains(&filter)
            {
                continue;
            }

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&card.name).size(17.0).strong());
                    ui.label(RichText::new(format!("v{}", card.version)).small().weak());
                    ui.label(RichText::new(card.source).small().color(ACCENT));

                    if self.state.active_theme.as_deref() == Some(card.id.as_str()) {
                        ui.label(RichText::new("ACTIVE").small().strong().color(GOOD));
                    }
                });

                swatches(ui, &card.colors);
                ui.label(RichText::new(&card.description).small().weak());

                ui.horizontal(|ui| {
                    let active = self.state.active_theme.as_deref() == Some(card.id.as_str());

                    if ui
                        .add_enabled(!active, egui::Button::new("Apply"))
                        .clicked()
                    {
                        apply = Some(card.id.clone());
                    }

                    if ui.button("Edit").clicked() {
                        edit = Some(card.id.clone());
                    }

                    if ui.button("Export").clicked() {
                        export = Some(card.id.clone());
                    }

                    if card.source == "user" && ui.button("Remove").clicked() {
                        remove = Some(card.id.clone());
                    }

                    if ui.button("Inspect").clicked() {
                        self.selected_theme = Some(card.id.clone());
                    }
                });
            });

            ui.add_space(4.0);
        }

        if let Some(id) = edit {
            self.start_editor(&id);
        }

        if let Some(id) = apply {
            self.apply_selected_integrations(&id);
        }

        if let Some(id) = remove {
            let result = self.state.remove_theme(&id);
            self.outcome(result);
        }

        if let Some(id) = export {
            let target = if self.export_path.trim().is_empty() {
                PathBuf::from(format!("{id}.toml"))
            } else {
                PathBuf::from(self.export_path.trim())
            };
            let result = self.state.export_theme(&id, &target);
            self.outcome(result);
        }

        if !self.import_path.trim().is_empty() && ui.button("Import theme").clicked() {
            let source = PathBuf::from(self.import_path.trim());
            let result = self.state.import_theme(&source, None);
            self.outcome(result);
        }

        if ui.button("Scan for new themes").clicked() {
            self.refresh();
        }

        ui.add_space(8.0);

        if !self.state.broken.is_empty() {
            ui.colored_label(WARN, "Themes that failed to load:");

            for error in &self.state.broken {
                ui.label(RichText::new(error).small().weak());
            }
        }

        if let Some(id) = self.selected_theme.clone()
            && let Ok(theme) = self.state.theme(&id)
        {
            ui.separator();
            ui.add_space(4.0);
            ui.heading(format!("Palette — {}", theme.metadata.name));

            egui::Grid::new("palette")
                .num_columns(3)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for (label, color) in [
                        ("Background", theme.colors.background),
                        ("Surface", theme.colors.surface),
                        ("Surface alt", theme.colors.surface_alt),
                        ("Primary", theme.colors.primary),
                        ("Secondary", theme.colors.secondary),
                        ("Text", theme.colors.text),
                        ("Text muted", theme.colors.text_muted),
                        ("Border", theme.colors.border),
                        ("Success", theme.colors.success),
                        ("Warning", theme.colors.warning),
                        ("Error", theme.colors.error),
                        ("Active", theme.colors.active),
                        ("Hover", theme.colors.hover),
                        ("Selected", theme.colors.selected),
                    ] {
                        ui.label(RichText::new("██").color(color32(color)));
                        ui.label(label);
                        ui.label(RichText::new(color.to_hex()).monospace().small());
                        ui.end_row();
                    }
                });
        }
    }

    fn editor(&mut self, ui: &mut Ui) {
        ui.heading("Theme editor");
        ui.label(
            RichText::new(
                "Start from an existing theme, adjust the colours, then save a custom theme. \
                 Applying is a separate action.",
            )
            .weak(),
        );

        ui.add_space(4.0);

        let mut base: Option<String> = None;
        let current = self
            .draft
            .as_ref()
            .map(|draft| draft.base.name.clone())
            .unwrap_or_else(|| "choose a base theme".to_string());

        ui.horizontal(|ui| {
            ui.label("Base theme");

            egui::ComboBox::from_id_salt("editor-base")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for card in &self.state.themes {
                        let selected = self
                            .draft
                            .as_ref()
                            .map(|draft| draft.base.id == card.id)
                            .unwrap_or(false);

                        if ui.selectable_label(selected, &card.name).clicked() {
                            base = Some(card.id.clone());
                        }
                    }
                });
        });

        if let Some(id) = base {
            self.start_editor(&id);
        }

        let Some(draft) = self.draft.as_mut() else {
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "No draft yet. Pick a base theme above or press Edit in the gallery.",
                )
                .weak(),
            );
            return;
        };

        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label("Id");
            ui.text_edit_singleline(&mut draft.id);
            ui.label("Name");
            ui.text_edit_singleline(&mut draft.name);
        });

        ui.add_space(6.0);

        egui::Grid::new("editor-colors")
            .num_columns(3)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                for (label, value) in [
                    ("Background", &mut draft.background),
                    ("Surface", &mut draft.surface),
                    ("Primary", &mut draft.primary),
                    ("Secondary", &mut draft.secondary),
                    ("Text", &mut draft.text),
                    ("Border", &mut draft.border),
                    ("Success", &mut draft.success),
                    ("Warning", &mut draft.warning),
                    ("Error", &mut draft.error),
                ] {
                    ui.label(label);
                    ui.color_edit_button_srgb(value);
                    ui.label(
                        RichText::new(format!("#{:02X}{:02X}{:02X}", value[0], value[1], value[2]))
                            .monospace()
                            .small(),
                    );
                    ui.end_row();
                }
            });

        ui.add_space(8.0);
        ui.label(RichText::new("Preview").strong());
        swatches(ui, &draft.colors());

        ui.add_space(8.0);

        let mut save = false;
        let mut save_and_apply = false;

        ui.horizontal(|ui| {
            if ui.button("Save custom theme").clicked() {
                save = true;
            }

            if ui.button("Save and apply").clicked() {
                save_and_apply = true;
            }
        });

        if save || save_and_apply {
            let id = sanitize_id(&draft.id);
            let name = draft.name.trim().to_string();
            let base = draft.base.clone();
            let colors = draft.colors();

            match self.state.save_theme(&id, &name, &base, &colors) {
                Ok(message) => {
                    self.status = Some((true, message));

                    if save_and_apply {
                        self.apply_selected_integrations(&id);
                    }
                }
                Err(error) => self.status = Some((false, error)),
            }
        }
    }

    fn applications(&mut self, ui: &mut Ui) {
        ui.heading("Applications");
        ui.label(
            RichText::new(
                "Detection comes from PATH. The configuration column reports the generated file \
                 that My Linux installed for each application.",
            )
            .weak(),
        );

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.button("Select detected").clicked() {
                self.enabled = self
                    .state
                    .integrations
                    .iter()
                    .filter(|row| row.available())
                    .map(|row| row.id)
                    .collect();
            }

            if ui.button("Select all").clicked() {
                self.enabled = self.state.integrations.iter().map(|row| row.id).collect();
            }

            if ui.button("Clear").clicked() {
                self.enabled.clear();
            }

            ui.label(
                RichText::new(format!("{} selected", self.enabled.len()))
                    .small()
                    .weak(),
            );
        });

        ui.add_space(6.0);

        egui::Grid::new("applications")
            .num_columns(4)
            .striped(true)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Integration").strong());
                ui.label(RichText::new("Detected").strong());
                ui.label(RichText::new("Configuration").strong());
                ui.label(RichText::new("Include").strong());
                ui.end_row();

                for row in &self.state.integrations {
                    ui.label(row.name);

                    match &row.detected {
                        Some(path) => {
                            ui.label(RichText::new(path.display().to_string()).small());
                        }
                        None => {
                            ui.label(
                                RichText::new(format!("not found ({})", row.programs.join(", ")))
                                    .small()
                                    .color(BAD),
                            );
                        }
                    }

                    let color = match row.state {
                        Some(FileState::Unchanged) => GOOD,
                        Some(FileState::Modified) => WARN,
                        Some(FileState::Missing) => BAD,
                        None => Color32::GRAY,
                    };

                    ui.label(
                        RichText::new(format!("{} ({})", row.destination, row.state_label()))
                            .small()
                            .color(color),
                    );

                    let mut checked = self.enabled.contains(&row.id);

                    if ui.checkbox(&mut checked, "").changed() {
                        if checked {
                            self.enabled.push(row.id);
                        } else {
                            self.enabled.retain(|id| *id != row.id);
                        }
                    }

                    ui.end_row();
                }
            });

        ui.add_space(8.0);

        let target = self
            .state
            .active_theme
            .clone()
            .or_else(|| self.selected_theme.clone())
            .or_else(|| self.state.themes.first().map(|theme| theme.id.clone()));

        if let Some(theme) = target
            && ui
                .button(format!(
                    "Apply {} integration(s) with {theme}",
                    self.enabled.len()
                ))
                .clicked()
        {
            self.apply_selected_integrations(&theme);
        }

        let changed: Vec<&str> = self
            .state
            .modifications
            .iter()
            .filter(|modification| modification.state != FileState::Unchanged)
            .map(|modification| modification.relative_path.as_str())
            .collect();

        if !changed.is_empty() {
            ui.add_space(6.0);
            ui.colored_label(WARN, "Configuration files changed outside My Linux:");

            for path in changed {
                ui.label(RichText::new(format!("  {path}")).small());
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(4.0);
        ui.heading("Appearance assets");
        ui.label(
            RichText::new(format!(
                "Hyprland snippet: {}",
                self.state.asset_snippet().display()
            ))
            .small()
            .weak(),
        );

        ui.horizontal(|ui| {
            ui.label("Font");
            ui.text_edit_singleline(&mut self.asset_font);
            ui.label("Size");
            ui.add(egui::DragValue::new(&mut self.asset_font_size));
        });

        ui.horizontal(|ui| {
            ui.label("Icon theme");
            ui.text_edit_singleline(&mut self.asset_icon);
            ui.label("Cursor theme");
            ui.text_edit_singleline(&mut self.asset_cursor);
            ui.label("Size");
            ui.add(egui::DragValue::new(&mut self.asset_cursor_size));
        });

        if ui.button("Save asset settings").clicked() {
            let settings = AssetSettings {
                font: Some(self.asset_font.trim().to_string()).filter(|font| !font.is_empty()),
                font_size: self.asset_font_size,
                icon_theme: Some(self.asset_icon.trim().to_string())
                    .filter(|icons| !icons.is_empty()),
                cursor_theme: Some(self.asset_cursor.trim().to_string())
                    .filter(|cursor| !cursor.is_empty()),
                cursor_size: self.asset_cursor_size,
            };
            let result = self.state.save_assets(&settings);
            self.outcome(result);
        }
    }

    fn wallpapers(&mut self, ui: &mut Ui) {
        ui.heading("Wallpapers");
        ui.label(
            RichText::new(format!(
                "{} wallpaper(s) discovered. Applying copies the file into the My Linux library \
                 and records it in the state directory.",
                self.state.wallpapers.len()
            ))
            .weak(),
        );

        ui.add_space(6.0);

        let mut preview: Option<PathBuf> = None;
        let mut apply: Option<PathBuf> = None;

        if self.state.wallpapers.is_empty() {
            ui.label(
                RichText::new(
                    "No wallpapers found. Add PNG or JPEG files to a wallpaper library directory.",
                )
                .weak(),
            );
        } else {
            egui::Grid::new("wallpapers")
                .num_columns(4)
                .striped(true)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Wallpaper").strong());
                    ui.label(RichText::new("Size").strong());
                    ui.label(RichText::new("Actions").strong());
                    ui.label(RichText::new("").strong());
                    ui.end_row();

                    for wallpaper in &self.state.wallpapers {
                        ui.label(&wallpaper.name);
                        ui.label(RichText::new(format_bytes(wallpaper.bytes)).small());

                        ui.horizontal(|ui| {
                            if ui.button("Palette").clicked() {
                                preview = Some(wallpaper.path.clone());
                            }

                            if ui.button("Apply").clicked() {
                                apply = Some(wallpaper.path.clone());
                            }
                        });

                        ui.label(
                            RichText::new(wallpaper.path.display().to_string())
                                .small()
                                .weak(),
                        );
                        ui.end_row();
                    }
                });
        }

        if let Some(path) = preview {
            match self.state.proposal(&path) {
                Ok(proposal) => {
                    let stem = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("wallpaper");
                    self.palette_id = sanitize_id(stem);
                    self.palette_name = display_name(&self.palette_id);
                    self.palette_path = Some(path);
                    self.wallpaper_palette = Some(proposal);
                }
                Err(error) => self.status = Some((false, error)),
            }
        }

        if let Some(path) = apply {
            let result = self.state.apply_wallpaper(&path);
            self.outcome(result);
        }

        let view = self.wallpaper_palette.as_ref().map(|proposal| {
            (
                proposal.wallpaper.clone(),
                proposal.width,
                proposal.height,
                proposal.dominant.clone(),
                proposal.palette.clone(),
            )
        });

        if let Some((path, width, height, dominant, palette)) = view {
            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label(
                    RichText::new(format!(
                        "Palette from {} ({}×{})",
                        path.display(),
                        width,
                        height
                    ))
                    .strong(),
                );

                swatches(ui, &palette);

                ui.label(RichText::new("Dominant colours").small().weak());
                ui.horizontal(|ui| {
                    for (color, count) in dominant.iter().take(6) {
                        ui.label(RichText::new("██").color(color32(*color)))
                            .on_hover_text(format!("{} ({} px)", color.to_hex(), count));
                    }
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Theme id");
                    ui.text_edit_singleline(&mut self.palette_id);
                    ui.label("Name");
                    ui.text_edit_singleline(&mut self.palette_name);
                });

                let mut save = false;
                let mut save_and_apply = false;

                ui.horizontal(|ui| {
                    if ui.button("Save theme").clicked() {
                        save = true;
                    }

                    if ui.button("Save and apply").clicked() {
                        save_and_apply = true;
                    }
                });

                if save || save_and_apply {
                    let id = sanitize_id(&self.palette_id);
                    let name = self.palette_name.trim().to_string();

                    if let Some(source) = self.palette_path.clone() {
                        match self.state.save_wallpaper_theme(&source, &id, &name) {
                            Ok(message) => {
                                self.status = Some((true, message));

                                if save_and_apply {
                                    self.apply_selected_integrations(&id);
                                }
                            }
                            Err(error) => self.status = Some((false, error)),
                        }
                    }
                }
            });
        }

        ui.add_space(10.0);
        ui.heading("Wallpaper history");

        let history = desktop_core::assets::WallpaperState::new(&self.state.root).history();

        if history.is_empty() {
            ui.label(RichText::new("No wallpaper changes recorded.").weak());
        } else {
            for entry in history.iter().rev().take(8) {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format_timestamp(entry.timestamp))
                            .small()
                            .weak(),
                    );
                    ui.label(RichText::new(&entry.action).small().strong());
                    ui.label(RichText::new(&entry.path).small());
                });
            }
        }
    }

    fn profiles(&mut self, ui: &mut Ui) {
        ui.heading("Desktop profiles");
        ui.label(
            RichText::new(
                "A profile applies a theme, wallpaper, integrations, assets and a Hyprland layout \
                 in one transaction.",
            )
            .weak(),
        );

        ui.add_space(6.0);

        let profiles = self.state.profiles_list.clone();
        let mut apply: Option<String> = None;
        let mut export: Option<String> = None;

        egui::Grid::new("profiles")
            .num_columns(5)
            .striped(true)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Profile").strong());
                ui.label(RichText::new("Theme").strong());
                ui.label(RichText::new("Wallpaper").strong());
                ui.label(RichText::new("Source").strong());
                ui.label(RichText::new("Actions").strong());
                ui.end_row();

                for profile in &profiles {
                    let active = self.state.active_profile.as_deref() == Some(profile.id.as_str());
                    let mut name = profile.name.clone();

                    if active {
                        name.push_str("  (active)");
                    }

                    ui.label(name);
                    ui.label(profile.theme.clone());
                    ui.label(
                        RichText::new(
                            profile
                                .wallpaper
                                .clone()
                                .unwrap_or_else(|| "none".to_string()),
                        )
                        .small(),
                    );
                    ui.label(RichText::new(profile.source).small().weak());

                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!active, egui::Button::new("Apply"))
                            .clicked()
                        {
                            apply = Some(profile.id.clone());
                        }

                        if ui.button("Export").clicked() {
                            export = Some(profile.id.clone());
                        }
                    });

                    ui.end_row();
                }
            });

        if let Some(id) = apply {
            let result = self.state.apply_profile(&id);
            self.outcome(result);
        }

        if let Some(id) = export {
            let target = if self.export_path.trim().is_empty() {
                PathBuf::from(format!("{id}.toml"))
            } else {
                PathBuf::from(self.export_path.trim())
            };
            let result = self.state.export_profile(&id, &target);
            self.outcome(result);
        }

        ui.add_space(10.0);
        ui.heading("Create a profile");

        ui.horizontal(|ui| {
            ui.label("Id");
            ui.text_edit_singleline(&mut self.new_profile_id);
            ui.label("Name");
            ui.text_edit_singleline(&mut self.new_profile_name);

            egui::ComboBox::from_id_salt("profile-theme")
                .selected_text(self.new_profile_theme.clone())
                .show_ui(ui, |ui| {
                    for card in &self.state.themes {
                        ui.selectable_value(
                            &mut self.new_profile_theme,
                            card.id.clone(),
                            &card.name,
                        );
                    }
                });

            if ui.button("Create").clicked() {
                let id = sanitize_id(&self.new_profile_id);
                let name = if self.new_profile_name.trim().is_empty() {
                    display_name(&id)
                } else {
                    self.new_profile_name.trim().to_string()
                };
                let theme = self.new_profile_theme.clone();
                let result = self.state.create_profile(&id, &name, &theme);
                self.outcome(result);
            }
        });

        ui.add_space(8.0);
        ui.heading("Import / restore");

        ui.horizontal(|ui| {
            ui.label("Import profile");
            ui.text_edit_singleline(&mut self.import_path);

            if ui.button("Import").clicked() {
                let source = PathBuf::from(self.import_path.trim());
                let result = self.state.import_profile(&source);
                self.outcome(result);
            }
        });

        if ui
            .add_enabled(
                self.state.previous_profile.is_some(),
                egui::Button::new("Restore previous profile"),
            )
            .clicked()
        {
            let result = self.state.restore_previous_profile();
            self.outcome(result);
        }
    }

    fn recovery(&mut self, ui: &mut Ui) {
        ui.heading("Recovery and diagnostics");

        ui.horizontal(|ui| {
            if ui.button("Run doctor").clicked() {
                self.refresh();
            }

            if ui.button("Rollback last apply").clicked() {
                let result = self.state.rollback();
                self.outcome(result);
            }

            if ui.button("Restore originals").clicked() {
                let result = self.state.restore();
                self.outcome(result);
            }

            if ui.button("Recover interrupted operation").clicked() {
                let result = self.state.recover();
                self.outcome(result);
            }
        });

        ui.add_space(8.0);

        let diagnostics = self.state.diagnostics();
        let problems = diagnostics.iter().filter(|check| !check.ok).count();

        ui.colored_label(
            if problems == 0 { GOOD } else { BAD },
            format!("{problems} problem(s) found"),
        );

        egui::Grid::new("diagnostics")
            .num_columns(3)
            .striped(true)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for check in &diagnostics {
                    ui.label(
                        RichText::new(if check.ok { "ok" } else { "FAIL" })
                            .strong()
                            .color(if check.ok { GOOD } else { BAD }),
                    );
                    ui.label(&check.label);
                    ui.label(RichText::new(&check.detail).small().weak());
                    ui.end_row();
                }
            });

        ui.add_space(10.0);
        ui.heading("Installed files");

        if self.state.modifications.is_empty() {
            ui.label(RichText::new("Nothing is installed yet.").weak());
        } else {
            for modification in &self.state.modifications {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&modification.relative_path)
                            .monospace()
                            .small(),
                    );
                    ui.label(RichText::new(modification.state.label()).small().color(
                        match modification.state {
                            FileState::Unchanged => GOOD,
                            FileState::Modified => WARN,
                            FileState::Missing => BAD,
                        },
                    ));
                });
            }
        }

        ui.add_space(10.0);
        ui.heading("Backups");

        if self.state.backups.is_empty() {
            ui.label(RichText::new("No backups yet; nothing original has been replaced.").weak());
        } else {
            egui::Grid::new("backups")
                .num_columns(4)
                .striped(true)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("File").strong());
                    ui.label(RichText::new("Size").strong());
                    ui.label(RichText::new("Written").strong());
                    ui.label(RichText::new("Digest").strong());
                    ui.end_row();

                    for backup in &self.state.backups {
                        ui.label(&backup.name);
                        ui.label(format_bytes(backup.bytes));
                        ui.label(
                            RichText::new(format_timestamp(backup.written))
                                .small()
                                .weak(),
                        );
                        ui.label(RichText::new(&backup.digest).monospace().small());
                        ui.end_row();
                    }
                });
        }

        ui.add_space(10.0);
        ui.heading("Installation history");

        if self.state.history.is_empty() {
            ui.label(RichText::new("No installation history recorded.").weak());
        } else {
            for entry in self.state.history.iter().rev().take(10) {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format_timestamp(entry.timestamp))
                            .small()
                            .weak(),
                    );
                    ui.label(RichText::new(&entry.action).small().strong());
                    ui.label(RichText::new(&entry.theme).small());
                    ui.label(RichText::new(entry.files.join(", ")).small().weak());
                });
            }
        }

        ui.add_space(10.0);
        ui.heading("Event log");

        if self.state.events.is_empty() {
            ui.label(RichText::new("No events recorded.").weak());
        } else {
            for event in self.state.events.iter().rev().take(10) {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format_timestamp(event.timestamp))
                            .small()
                            .weak(),
                    );
                    ui.label(
                        RichText::new(event.level.label().to_uppercase())
                            .small()
                            .strong()
                            .color(match event.level {
                                desktop_core::log::Level::Info => GOOD,
                                desktop_core::log::Level::Warn => WARN,
                                desktop_core::log::Level::Error => BAD,
                            }),
                    );
                    ui.label(RichText::new(&event.action).small());
                    ui.label(RichText::new(&event.detail).small().weak());
                });
            }
        }

        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "Events: {} · backups: {} · journal: {}",
                self.state.events.len(),
                self.state.backups.len(),
                journal_state(&self.state.root),
            ))
            .small()
            .weak(),
        );
    }
}

impl eframe::App for MyLinuxApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("header").show(ui, |ui| self.header(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_line(ui));

        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(4.0);

                    match self.tab {
                        Tab::Dashboard => self.dashboard(ui),
                        Tab::Gallery => self.gallery(ui),
                        Tab::Editor => self.editor(ui),
                        Tab::Applications => self.applications(ui),
                        Tab::Wallpapers => self.wallpapers(ui),
                        Tab::Profiles => self.profiles(ui),
                        Tab::Recovery => self.recovery(ui),
                    }

                    ui.add_space(16.0);
                });
        });
    }
}

fn stat(ui: &mut Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).small().weak());
        ui.label(RichText::new(value).size(18.0).strong());
    });

    ui.add_space(18.0);
}

fn swatches(ui: &mut Ui, colors: &ThemeColors) {
    ui.horizontal(|ui| {
        for (label, color) in [
            ("background", colors.background),
            ("surface", colors.surface),
            ("primary", colors.primary),
            ("secondary", colors.secondary),
            ("text", colors.text),
            ("border", colors.border),
        ] {
            ui.label(RichText::new("███").color(color32(color)))
                .on_hover_text(format!("{label} {}", color.to_hex()));
        }
    });
}

fn journal_state(root: &Path) -> &'static str {
    if root.join("state").join("journal.json").exists() {
        "interrupted operation recorded"
    } else {
        "clean"
    }
}
