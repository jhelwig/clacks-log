//! Server-rendered account pages and Rust browser hydration.

pub mod actions;
mod pages;

use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_meta::MetaTags;
use leptos_meta::{Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

/// Render the account application with client-side navigation support.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Stylesheet id="clacks-style" href="/pkg/clacks-web.css"/>
        <Title text="Clacks Log"/>
        <Router>
            <header class="app-bar"><a href="/account/identities">"Clacks Log"</a><span>"Account"</span></header>
            <main><Routes fallback=|| view! { <p>"Page not found. "<a href="/sign-in">"Sign in"</a></p> }>
                <Route path=path!("/sign-in") view=pages::SignIn/>
                <Route path=path!("/account/identities") view=pages::Identities/>
            </Routes></main>
        </Router>
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
    use axum::{response::Redirect, routing::get};
    use leptos_axum::LeptosRoutes;

    let asset_route = leptos_axum::site_pkg_dir_service_route_path(&options);
    let assets = leptos_axum::site_pkg_dir_service(&options);
    let paths = leptos_axum::generate_route_list(App);
    let shell_options = options.clone();
    axum::Router::new()
        .route("/", get(|| async { Redirect::to("/account/identities") }))
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
