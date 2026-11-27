//! Public introduction to the Clacks Log desktop application.

use leptos::prelude::*;

/// Introduce the desktop application and link to the installers.
#[component]
pub(crate) fn Landing() -> impl IntoView {
    view! {
        <section class="public-page section landing">
            <h1>"One amateur radio logbook across your computers."</h1>
            <p class="landing-intro">
                "Log contacts on your shack computer and your field laptop. "
                "Keep logging on either one when you’re disconnected. "
                "When they reconnect, both computers sync to the same logbook."
            </p>
            <a class="public-action" href="/download">"Download Clacks Log"</a>
            <p class="sync-setup">
                "To sync, open "<strong>"Settings → Sync → Connect → Sign in"</strong>"."
            </p>
        </section>
    }
}
