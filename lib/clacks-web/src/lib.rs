//! Server-rendered public and account pages with Rust browser hydration.

pub mod actions;
mod pages;

use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_meta::MetaTags;
use leptos_meta::{Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::use_location,
    path,
};

/// Render the web application with client-side navigation support.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Stylesheet id="clacks-style" href="/pkg/clacks-web.css"/>
        <Title text="Clacks Log"/>
        <Router>
            <Navigation/>
            <main><Routes fallback=|| view! { <p>"Page not found. "<a href="/sign-in">"Sign in"</a></p> }>
                <Route path=path!("/") view=pages::landing::Landing/>
                <Route path=path!("/download") view=pages::download::Download/>
                <Route path=path!("/sign-in") view=pages::SignIn/>
                <Route path=path!("/account/identities") view=pages::Identities/>
                <Route path=path!("/account/directory") view=pages::directory::DirectorySettings/>
                <Route path=path!("/account/clients") view=pages::clients::Clients/>
                <Route path=path!("/account/clients/approve") view=pages::clients::Approval/>
            </Routes></main>
        </Router>
    }
}

/// Show the public navigation on the landing and download pages.
#[component]
fn Navigation() -> impl IntoView {
    let location = use_location();
    view! {
        <header class="app-bar">
            <Show
                when=move || location.pathname.with(|path| matches!(path.as_str(), "/" | "/download" | "/download/"))
                fallback=|| view! {
                    <a href="/account/identities">"Clacks Log"</a>
                    <span>"Account"</span>
                    <a href="/account/clients">"Desktop clients"</a>
                    <a href="/account/directory">"Directory"</a>
                }
            >
                <a href="/">"Clacks Log"</a>
                <a href="/download">"Download"</a>
                <a href="/sign-in">"Sign in"</a>
            </Show>
        </header>
    }
}

/// Build the full HTML document consumed by server rendering and hydration.
#[cfg(feature = "ssr")]
fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en"><head><meta charset="utf-8"/>
            <meta name="viewport" content="width=device-width, initial-scale=1"/>
            <AutoReload options=options.clone()/><HydrationScripts options/>
            <MetaTags/>
        </head><body><App/></body></html>
    }
}

/// Mount rendered pages and standard server-function handling into the server policies.
#[cfg(feature = "ssr")]
pub fn routes(
    state: clacks_server::runtime::ServerState,
    options: LeptosOptions,
) -> axum::Router<clacks_server::runtime::ServerState> {
    use leptos_axum::LeptosRoutes;

    let asset_route = leptos_axum::site_pkg_dir_service_route_path(&options);
    let assets = leptos_axum::site_pkg_dir_service(&options);
    let paths = leptos_axum::generate_route_list(App);
    let shell_options = options.clone();
    axum::Router::new()
        .route_service(&asset_route, assets)
        .leptos_routes_with_context(
            &options,
            paths,
            move || provide_context(state.clone()),
            move || shell(shell_options.clone()),
        )
        .with_state(options)
}

/// Hydrate the server-rendered application with Leptos's browser runtime.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos::mount::hydrate_body(App);
}
