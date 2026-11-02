//! Goal-oriented directory selection and provider credential forms.

use crate::actions::directory::{self, DirectorySaveOutcome, DirectorySetup, SaveDirectory};
use clacks_model::directory::values::{DirectoryConfiguration, DirectoryProvider};
use leptos::{form::ActionForm, prelude::*};

/// Edit provider selection and each provider's login independently.
#[component]
pub fn DirectorySettings() -> impl IntoView {
    let setup = Resource::new(|| (), |_| directory::load_directory());
    view! {
        <section class="section"><h1>"Directory"</h1>
            <p>"Directory lookup runs directly on enrolled desktops."</p>
            <Suspense fallback=|| view! { <p role="status">"Loading directory…"</p> }>
                {move || setup.get().map(|result| match result {
                    Ok(setup) => view! { <DirectoryForms setup/> }.into_any(),
                    Err(error) => view! { <p class="error" role="alert">{error.to_string()}</p> }.into_any(),
                })}
            </Suspense>
        </section>
    }
}

/// Share the current revision without remounting editable forms after submission.
#[component]
fn DirectoryForms(setup: DirectorySetup) -> impl IntoView {
    let configuration = RwSignal::new(setup.configuration);
    let selection = ServerAction::<SaveDirectory>::new();
    observe_result(selection, configuration);
    let selected = configuration
        .get_untracked()
        .selected
        .map(|p| p.as_str())
        .unwrap_or("");
    view! {
        <ActionForm action=selection>
            <input type="hidden" name="expected_revision" value=move || configuration.get().revision/>
            <input type="hidden" name="operation" value="keep"/>
            <input type="hidden" name="provider" value=""/>
            <input type="hidden" name="username" value=""/>
            <input type="hidden" name="password" value=""/>
            <label>"Lookup provider"<select name="selected" prop:value=selected>
                <option value="" selected=selected.is_empty()>"Disabled"</option>
                <option value="qrz" selected=selected == "qrz">"QRZ"</option>
                <option value="hamqth" selected=selected == "hamqth">"HamQTH"</option>
            </select></label>
            <button class="primary" disabled=move || selection.pending().get()>"Save selection"</button>
            <SubmissionStatus action=selection/>
        </ActionForm>
        <CredentialForm provider=DirectoryProvider::Qrz username=setup.usernames.get(&DirectoryProvider::Qrz).cloned().unwrap_or_default() configuration/>
        <CredentialForm provider=DirectoryProvider::HamQth username=setup.usernames.get(&DirectoryProvider::HamQth).cloned().unwrap_or_default() configuration/>
    }
}

/// Keep public revision/availability current while preserving unsaved input values.
fn observe_result(
    action: ServerAction<SaveDirectory>,
    configuration: RwSignal<DirectoryConfiguration>,
) {
    Effect::new(move |_| {
        if let Some(Ok(
            DirectorySaveOutcome::Committed(current) | DirectorySaveOutcome::Unchanged(current),
        )) = action.value().get()
        {
            configuration.update(|previous| {
                if current.revision >= previous.revision {
                    *previous = current;
                }
            });
        }
    });
}

/// A provider's ordinary editable username and write-only password fields.
#[component]
fn CredentialForm(
    provider: DirectoryProvider,
    username: String,
    configuration: RwSignal<DirectoryConfiguration>,
) -> impl IntoView {
    let action = ServerAction::<SaveDirectory>::new();
    observe_result(action, configuration);
    let username = RwSignal::new(username);
    let password = RwSignal::new(String::new());
    let saved = move || configuration.get().configured.contains(&provider);
    Effect::new(move |_| {
        if let Some(Ok(
            DirectorySaveOutcome::Committed(current) | DirectorySaveOutcome::Unchanged(current),
        )) = action.value().get()
        {
            password.set(String::new());
            if !current.configured.contains(&provider) {
                username.set(String::new());
            }
        }
    });
    view! {
        <section class="section">
            <h2>{match provider { DirectoryProvider::Qrz => "QRZ", DirectoryProvider::HamQth => "HamQTH" }}</h2>
            <ActionForm action>
                <input type="hidden" name="expected_revision" value=move || configuration.get().revision/>
                <input type="hidden" name="selected" value=""/>
                <input type="hidden" name="provider" value=provider.as_str()/>
                <label>"Username or callsign"<input type="text" name="username" autocomplete="username"
                    value=move || username.get() prop:value=move || username.get() on:input=move |event| username.set(event_target_value(&event))/></label>
                <label>"Password"<input type="password" name="password" autocomplete="new-password"
                    placeholder=move || if saved() { "••••••••" } else { "" }
                    prop:value=move || password.get() on:input=move |event| password.set(event_target_value(&event))/></label>
                <div class="actions">
                    <button class="primary" name="operation" value="save" disabled=move || action.pending().get()>"Save credentials"</button>
                    <Show when=saved><button name="operation" value="remove" disabled=move || action.pending().get()>"Remove credentials"</button></Show>
                </div>
                <SubmissionStatus action/>
            </ActionForm>
        </section>
    }
}

/// Display submission feedback beside the form that produced it.
#[component]
fn SubmissionStatus(action: ServerAction<SaveDirectory>) -> impl IntoView {
    view! {
        <p class="error" role="alert">{move || action.value().get().and_then(Result::err).map(|error| error.to_string())}</p>
        <p role="status">{move || action.value().get().and_then(Result::ok).map(|outcome| match outcome {
            DirectorySaveOutcome::Committed(_) => "Saved.",
            DirectorySaveOutcome::Unchanged(_) => "Already saved.",
            DirectorySaveOutcome::Stale(_) => "Directory settings changed elsewhere. Reload this page to see the saved settings before trying again.",
        })}</p>
    }
}
