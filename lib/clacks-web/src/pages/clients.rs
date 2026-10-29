//! Desktop approval and client management using the shared account presentation.

use crate::actions::clients::{self, DecideApproval, RevokeClient};
use leptos::{form::ActionForm, prelude::*};

/// Present the retained request and the exact effect of authorizing this desktop.
#[component]
pub fn Approval() -> impl IntoView {
    let query = leptos_router::hooks::use_query_map();
    let request = move || {
        query
            .with(|parameters| parameters.get("request"))
            .unwrap_or_default()
    };
    let summary = Resource::new(request, clients::load_approval);
    let action = ServerAction::<DecideApproval>::new();
    view! {
        <section class="section"><h1>"Authorize desktop client"</h1>
            <Suspense fallback=|| view! { <p role="status">"Loading authorization request…"</p> }>
                {move || summary.get().map(|result| match result {
                    Ok(summary) => view! {
                        <dl><dt>"Server"</dt><dd>{summary.server}</dd><dt>"Account"</dt><dd><code>{summary.account}</code></dd>
                            <dt>"Client"</dt><dd>{summary.label}</dd></dl>
                        <p>{summary.consequence}</p>
                        <ActionForm action><input type="hidden" name="request" value=request/>
                            <button class="primary" name="decision" value="approve" disabled=move || action.pending().get()>"Approve"</button>
                            <button name="decision" value="deny" disabled=move || action.pending().get()>"Deny"</button>
                        </ActionForm>
                    }.into_any(),
                    Err(error) => view! { <p class="error" role="alert">{error.to_string()}</p><a href="/sign-in">"Sign in"</a> }.into_any(),
                })}
            </Suspense>
            <p class="error" role="alert">{move || action.value().get().and_then(Result::err).map(|error| error.to_string())}</p>
        </section>
    }
}

/// List real admitted clients and offer explicit revocation within the current account.
#[component]
pub fn Clients() -> impl IntoView {
    let revoke = ServerAction::<RevokeClient>::new();
    let clients = Resource::new(move || revoke.version().get(), |_| clients::list_clients());
    view! {
        <section class="section"><h1>"Desktop clients"</h1>
            <p>"Revoking a client stops its authorization. Shared contacts and other clients are unchanged."</p>
            <Suspense fallback=|| view! { <p role="status">"Loading clients…"</p> }>
                {move || clients.get().map(|result| match result {
                    Ok(clients) if clients.is_empty() => view! { <p>"No desktop clients are enrolled."</p> }.into_any(),
                    Ok(clients) => view! { <div class="table-scroll"><table><thead><tr><th>"Client"</th><th>"Client ID"</th><th>"Authorized"</th><th>"Status"</th><th>"Action"</th></tr></thead>
                        <tbody>{clients.into_iter().map(|client| {
                            let active = client.closed_at.is_none();
                            view! { <tr><td>{client.label}</td><td><code>{client.actor.clone()}</code></td><td>{client.authorized_at}</td>
                                <td>{client.closed_at.map(|time| format!("Revoked {time}")).unwrap_or_else(|| "Authorized; awaiting synchronization".into())}</td>
                                <td>{active.then(|| view! { <ActionForm action=revoke><input type="hidden" name="actor" value=client.actor/>
                                    <button disabled=move || revoke.pending().get()>"Revoke client"</button></ActionForm> })}</td></tr> }
                        }).collect_view()}</tbody></table></div> }.into_any(),
                    Err(error) => view! { <p class="error" role="alert">{error.to_string()}</p> }.into_any(),
                })}
            </Suspense>
            <p class="error" role="alert">{move || revoke.value().get().and_then(Result::err).map(|error| error.to_string())}</p>
        </section>
    }
}
