//! Installed-family selection and off-thread font requests on the desktop session.

#[cfg(test)]
mod tests;

use super::preferences::{AppearancePreferences, FontChoice};
use crate::session::SessionCommand;
use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

/// Native font work admitted and drained by the existing application session.
#[derive(Debug)]
pub(crate) enum FontOperation {
    /// Discover actual installed families without blocking drawing.
    Enumerate {
        /// Reply owned by the appearance view requesting the catalog.
        reply: oneshot::Sender<anyhow::Result<Vec<String>>>,
    },
    /// Load the default face of one selected native family.
    Load {
        /// Native family name, used locally and not emitted to telemetry.
        family: String,
        /// Reply owned by the requesting font role.
        reply: oneshot::Sender<anyhow::Result<FontData>>,
    },
}

impl FontOperation {
    /// Explain rejected admission without leaving a live view waiting for a reply.
    pub(crate) fn reject(self, message: &str) {
        match self {
            Self::Enumerate { reply } => {
                let _ = reply.send(Err(anyhow::anyhow!("{message}")));
            }
            Self::Load { reply, .. } => {
                let _ = reply.send(Err(anyhow::anyhow!("{message}")));
            }
        }
    }

    /// Finish an admitted blocking operation, skipping requests whose view moved on.
    pub(crate) fn run(self) {
        match self {
            Self::Enumerate { reply } => {
                if !reply.is_closed() {
                    let result = enumerate();
                    if result.is_err() {
                        tracing::warn!(event = "desktop.font_catalog_failed");
                    }
                    let _ = reply.send(result);
                }
            }
            Self::Load { family, reply } => {
                if !reply.is_closed() {
                    let result = load(&family);
                    if result.is_err() {
                        tracing::warn!(event = "desktop.selected_font_unavailable");
                    }
                    let _ = reply.send(result);
                }
            }
        }
    }
}

/// Enumerate native families for the real font picker on a blocking worker.
#[tracing::instrument(skip_all, name = "desktop.fonts.enumerate")]
fn enumerate() -> anyhow::Result<Vec<String>> {
    let mut collection = fontique::Collection::new(Default::default());
    let mut names: Vec<_> = collection.family_names().map(str::to_owned).collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    Ok(names)
}

/// Load a native family's default face, preserving its index in a font collection.
#[tracing::instrument(skip_all, name = "desktop.fonts.load")]
fn load(family: &str) -> anyhow::Result<FontData> {
    let mut collection = fontique::Collection::new(Default::default());
    let family = collection
        .family_by_name(family)
        .ok_or_else(|| anyhow::anyhow!("The selected font family is no longer available."))?;
    let font = family
        .default_font()
        .ok_or_else(|| anyhow::anyhow!("The selected family has no usable default face."))?;
    let mut sources = fontique::SourceCache::default();
    let data = sources
        .get(font.source())
        .ok_or_else(|| anyhow::anyhow!("The selected font could not be read."))?;
    let (bytes, _) = data.into_raw_parts();
    Ok(FontData::from_blob(bytes, font.index()))
}

/// Choice and response remain associated even if the operator selects another family.
struct FontRequest {
    /// Exact family requested by this operation.
    choice: FontChoice,
    /// Completion from the session-owned blocking job.
    reply: oneshot::Receiver<anyhow::Result<FontData>>,
}

/// One role's pending request, usable face and optional fallback explanation.
#[derive(Default)]
struct RoleState {
    /// Last choice attempted, including a failed choice that must not retry per frame.
    requested: Option<FontChoice>,
    /// Only this response may supply the current role's font.
    request: Option<FontRequest>,
    /// Loaded selected face; absent means bundled role fallback.
    data: Option<Arc<FontData>>,
    /// Visible operational explanation, not a validation or domain error.
    notice: Option<String>,
}

/// Installed catalog and role requests shared across application views.
#[derive(Default)]
pub(super) struct FontsUi {
    /// Available native families, populated after the first appearance-editor open.
    families: Vec<String>,
    /// Catalog completion, if discovery has been admitted.
    catalog: Option<oneshot::Receiver<anyhow::Result<Vec<String>>>>,
    /// Prevent repeated discovery on every drawing pass after completion/failure.
    catalog_requested: bool,
    /// Actionable catalog failure separate from an empty successful enumeration.
    catalog_notice: Option<String>,
    /// Normal text role.
    ui: RoleState,
    /// Radio-data role.
    radio: RoleState,
    /// Whether named role aliases have been installed in this context.
    installed: bool,
}

impl FontsUi {
    /// Advance actual native requests and install only faces matching current choices.
    pub fn update(
        &mut self,
        context: &egui::Context,
        preferences: &AppearancePreferences,
        commands: &mpsc::Sender<SessionCommand>,
        discover: bool,
    ) {
        if discover && !self.catalog_requested {
            let (reply, response) = oneshot::channel();
            self.catalog_requested = true;
            if commands
                .try_send(SessionCommand::Font(FontOperation::Enumerate { reply }))
                .is_ok()
            {
                self.catalog = Some(response);
            } else {
                self.catalog_notice = Some(
                    "Could not request font discovery. Retry when the application is ready.".into(),
                );
            }
        }
        if let Some(response) = &mut self.catalog {
            match response.try_recv() {
                Ok(Ok(names)) => {
                    self.families = names;
                    self.catalog = None;
                }
                Ok(Err(error)) => {
                    self.catalog_notice = Some(error.to_string());
                    self.catalog = None;
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.catalog_notice = Some(
                        "Font discovery did not complete. Retry when the application is ready."
                            .into(),
                    );
                    self.catalog = None;
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            }
        }
        let ui_changed = update_role(&mut self.ui, &preferences.ui_font, commands);
        let radio_changed = update_role(&mut self.radio, &preferences.radio_font, commands);
        if !self.installed || ui_changed || radio_changed {
            let mut definitions = FontDefinitions::default();
            install_role(
                &mut definitions,
                "clacks.ui",
                FontFamily::Proportional,
                &self.ui,
            );
            install_role(
                &mut definitions,
                "clacks.radio",
                FontFamily::Monospace,
                &self.radio,
            );
            context.set_fonts(definitions);
            self.installed = true;
        }
        if self.catalog.is_some()
            || self.ui.request.is_some()
            || self.radio.request.is_some()
            || (discover && !self.catalog_requested)
        {
            context.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    /// Present actual font choices and fallback/loading information in Appearance.
    pub fn show(&mut self, ui: &mut egui::Ui, preferences: &mut AppearancePreferences) {
        selector(
            ui,
            "appearance.ui-font",
            "UI font",
            &mut preferences.ui_font,
            &self.families,
            &self.ui,
        );
        selector(
            ui,
            "appearance.radio-font",
            "Radio-data font",
            &mut preferences.radio_font,
            &self.families,
            &self.radio,
        );
        if self.catalog.is_some() {
            ui.label("Loading installed fonts…");
        }
        if let Some(notice) = &self.catalog_notice {
            ui.label(notice);
        }
        if (self.catalog_notice.is_some()
            || self.ui.notice.is_some()
            || self.radio.notice.is_some())
            && ui.button("Retry font loading").clicked()
        {
            self.catalog_requested = false;
            self.catalog_notice = None;
            self.ui.requested = None;
            self.radio.requested = None;
        }
        ui.monospace("W7O0I · 14.074000 MHz · 18:32:15 UTC");
    }
}

/// Update one role without allowing an earlier choice's response to displace it.
fn update_role(
    state: &mut RoleState,
    choice: &FontChoice,
    commands: &mpsc::Sender<SessionCommand>,
) -> bool {
    let mut changed = false;
    if state.requested.as_ref() != Some(choice) {
        state.request = None;
        state.data = None;
        state.notice = None;
        changed = true;
        match choice {
            FontChoice::Default => state.requested = Some(choice.clone()),
            FontChoice::Installed { family } => {
                let (reply, response) = oneshot::channel();
                state.requested = Some(choice.clone());
                if commands
                    .try_send(SessionCommand::Font(FontOperation::Load {
                        family: family.clone(),
                        reply,
                    }))
                    .is_ok()
                {
                    state.request = Some(FontRequest {
                        choice: choice.clone(),
                        reply: response,
                    });
                } else {
                    state.notice = Some("Could not request font loading. Using the bundled default; retry when the application is ready.".into());
                }
            }
        }
    }
    if let Some(request) = &mut state.request {
        match request.reply.try_recv() {
            Ok(result) => {
                if &request.choice == choice {
                    match result {
                        Ok(data) => {
                            state.data = Some(Arc::new(data));
                            changed = true;
                        }
                        Err(error) => {
                            state.notice = Some(format!(
                                "{error} Using the bundled default; your requested family is retained."
                            ))
                        }
                    }
                }
                state.request = None;
            }
            Err(oneshot::error::TryRecvError::Closed) => {
                state.notice =
                    Some("Font loading did not complete. Using the bundled default.".into());
                state.request = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
        }
    }
    changed
}

/// Register a selected face ahead of the role's bundled glyph fallback chain.
fn install_role(
    definitions: &mut FontDefinitions,
    name: &str,
    fallback: FontFamily,
    state: &RoleState,
) {
    let mut chain = definitions
        .families
        .get(&fallback)
        .cloned()
        .unwrap_or_default();
    if let Some(data) = &state.data {
        definitions.font_data.insert(name.into(), data.clone());
        chain.insert(0, name.into());
    }
    definitions
        .families
        .insert(FontFamily::Name(name.into()), chain);
}

/// Select a native family while preserving unavailable requested names for recovery.
fn selector(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    choice: &mut FontChoice,
    families: &[String],
    state: &RoleState,
) {
    ui.label(label);
    let current = match choice {
        FontChoice::Default => "Bundled default".to_owned(),
        FontChoice::Installed { family } => family.clone(),
    };
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .show_ui(ui, |ui| {
            ui.selectable_value(choice, FontChoice::Default, "Bundled default");
            for family in families {
                ui.selectable_value(
                    choice,
                    FontChoice::Installed {
                        family: family.clone(),
                    },
                    family,
                );
            }
        });
    if state.request.is_some() {
        ui.label("Loading selected font…");
    }
    if let Some(notice) = &state.notice {
        ui.label(notice);
    }
}
