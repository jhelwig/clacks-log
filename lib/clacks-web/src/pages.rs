//! Account screens built from native forms and standard Leptos actions.

use crate::actions::{self, Account, SignOut, StartLink, StartSignIn, StartUnlink};
use leptos::form::ActionForm;
use leptos::prelude::*;

/// Offer the deployment's configured providers without exposing credentials.
#[component]
pub fn SignIn() -> impl IntoView {
    let query = leptos_router::hooks::use_query_map();
    let providers = Resource::new(|| (), |_| actions::providers());
    let action = ServerAction::<StartSignIn>::new();
    view! {
        <section class="section sign-in"><h1>"Sign in"</h1>
            <p class="error" role="alert">{move || query.with(|params| params.get("error"))}</p>
            <p>"Use a configured identity provider to sign in or create your Clacks account."</p>
            <Suspense fallback=|| view! { <p role="status">"Loading providers…"</p> }>
                {move || providers.get().map(|result| match result {
                    Ok(providers) => view! {
                        <ActionForm action>
                            <label>"Identity provider"<select name="provider" required>
                                {providers.into_iter().map(|provider| view! { <option value=provider.key>{provider.label}</option> }).collect_view()}
                            </select></label>
                            <button class="primary" disabled=move || action.pending().get()>"Continue"</button>
                        </ActionForm>
                    }.into_any(),
                    Err(error) => view! { <p class="error" role="alert">{error.to_string()}</p> }.into_any(),
                })}
            </Suspense>
            <p class="error" role="alert">{move || action.value().get().and_then(Result::err).map(|error| error.to_string())}</p>
            <p class="muted">"Cancelled or expired sign-in? Start again here. Your existing account is unchanged."</p>
        </section>
    }
}

/// Show the current account and its independently linked login identities.
#[component]
pub fn Identities() -> impl IntoView {
    let account = Resource::new(|| (), |_| actions::load_account());
    view! {
        <Suspense fallback=|| view! { <p role="status">"Loading account…"</p> }>
            {move || account.get().map(|result| match result {
                Ok(account) => view! { <AccountDetails account/> }.into_any(),
                Err(error) => view! { <section class="section"><h1>"Account unavailable"</h1><p class="error" role="alert">{error.to_string()}</p><a href="/sign-in">"Return to sign-in"</a></section> }.into_any(),
            })}
        </Suspense>
    }
}

/// Present owned identities and explicit link/removal actions.
#[component]
fn AccountDetails(account: Account) -> impl IntoView {
    let sign_out = ServerAction::<SignOut>::new();
    let link = ServerAction::<StartLink>::new();
    let unlink = ServerAction::<StartUnlink>::new();
    let last_identity = account.identities.len() <= 1;
    let identities = account.identities.clone();
    let authorizers = identities.clone();
    let remaining = identities.clone();
    let initial_target = identities
        .first()
        .map(|identity| identity.id.clone())
        .unwrap_or_default();
    let (target, set_target) = signal(initial_target);

    view! {
        <div class="heading"><div><h1>"Login identities"</h1><p class="muted">"Account "<code>{account.id}</code></p></div>
            <ActionForm action=sign_out><button disabled=move || sign_out.pending().get()>"Sign out"</button></ActionForm>
        </div>
        {account.administrator.then(|| view! { <p class="badge">"Server administrator"</p> })}
        <p class="error" role="alert">{move || sign_out.value().get().and_then(Result::err).map(|error| error.to_string())}</p>
        <section class="section"><h2>"Linked identities"</h2><div class="table-scroll"><table><thead><tr><th>"Provider issuer"</th><th>"Subject"</th><th>"This browser"</th></tr></thead>
            <tbody>{identities.into_iter().map(|identity| {
                let current = identity.id == account.current_identity;
                view! { <tr><td><code>{identity.issuer}</code></td><td><code>{identity.subject}</code></td><td>{if current { "Signed in" } else { "" }}</td></tr> }
            }).collect_view()}</tbody>
        </table></div></section>
        <div class="action-grid">
            <section class="section"><h2>"Add login identity"</h2>
                <p>"First authenticate through an existing identity, then select the identity to add. Accounts are never merged."</p>
                <ActionForm action=link>
                    <label>"Authenticate through"<select name="authorizing_identity" required>{authorizers.into_iter().map(|identity| view! { <option value=identity.id>{format!("{} — {}", identity.issuer, identity.subject)}</option> }).collect_view()}</select></label>
                    <label>"New identity provider"<select name="provider" required>{account.providers.into_iter().map(|provider| view! { <option value=provider.key>{provider.label}</option> }).collect_view()}</select></label>
                    <button class="primary" disabled=move || link.pending().get()>"Add identity…"</button>
                </ActionForm>
                <p class="error" role="alert">{move || link.value().get().and_then(Result::err).map(|error| error.to_string())}</p>
            </section>
            <section class="section"><h2>"Remove login identity"</h2><p>"Authenticate through an identity that will remain linked. Removing an identity signs out its Clacks browser logins."</p>
                <ActionForm action=unlink>
                    <label>"Identity to remove"<select name="target" required disabled=last_identity prop:value=target on:change=move |event| set_target.set(event_target_value(&event))>{account.identities.into_iter().map(|identity| view! { <option value=identity.id>{format!("{} — {}", identity.issuer, identity.subject)}</option> }).collect_view()}</select></label>
                    <label>"Authenticate through a different identity"<select name="authorizing_identity" required disabled=last_identity>{move || remaining.iter().filter(|identity| identity.id != target.get()).cloned().map(|identity| view! { <option value=identity.id>{format!("{} — {}", identity.issuer, identity.subject)}</option> }).collect_view()}</select></label>
                    <button disabled=move || last_identity || unlink.pending().get()>"Remove identity…"</button>
                </ActionForm>
                {last_identity.then(|| view! { <p class="muted">"Add another identity before removing your last login identity."</p> })}
                <p class="error" role="alert">{move || unlink.value().get().and_then(Result::err).map(|error| error.to_string())}</p>
            </section>
        </div>
    }
}
