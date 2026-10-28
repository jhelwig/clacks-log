//! Native integration scenarios share fixtures and one linked executable.

mod support;

#[path = "support/logbooks.rs"]
mod logbook_support;
#[path = "support/settings.rs"]
mod settings_support;
#[path = "support/telemetry.rs"]
mod telemetry;

mod contact_pages;
mod initialization;
mod local_saves;
mod logbook_deletion;
mod logbook_observability;
mod logbook_transfer;
mod logbooks;
mod observability;
mod opening;
mod operating;
mod preparation;
mod reads;
mod recovery;
mod settings;
mod settings_deletion;
mod settings_observability;
mod tags;
mod transfers;

mod history_queries;

