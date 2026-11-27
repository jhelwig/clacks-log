//! Public download page and browser-owned release lookup.

use clacks_desktop_release::ReleaseVersion;
use leptos::prelude::*;

#[cfg(any(feature = "hydrate", test))]
mod compatibility;
#[cfg(any(feature = "hydrate", test))]
mod release;

/// Public GitHub release history and base URL of the installer links.
const RELEASES_URL: &str = "https://github.com/jhelwig/clacks-log/releases";

/// Installer links and notes selected from the same published desktop release.
#[derive(Clone)]
struct DesktopRelease {
    /// Validated identity of the selected release, including its exact canonical tag.
    version: ReleaseVersion,
    /// GitHub URL of the Windows installer, if available.
    windows: Option<String>,
    /// GitHub URL of the macOS disk image, if available.
    macos: Option<String>,
    /// Nonempty release notes in Markdown, if provided.
    notes: Option<String>,
}

/// Offer the latest published desktop version and link to release history.
#[component]
pub(crate) fn Download() -> impl IntoView {
    let latest = LocalResource::new(load_latest);

    view! {
        <section class="public-page section">
            <h1>"Download Clacks Log"</h1>
            <Suspense fallback=|| view! { <p role="status">"Loading release information…"</p> }>
                {move || latest.get().map(|result| match result {
                    Ok(Some(release)) => view! { <ReleaseDetails release/> }.into_any(),
                    Ok(None) => view! { <p role="status">"No final desktop release is available yet."</p> }.into_any(),
                    Err(()) => view! { <p class="error" role="alert">"Couldn’t load release information."</p> }.into_any(),
                })}
            </Suspense>
            <a href=RELEASES_URL>"View release history on GitHub"</a>
        </section>
    }
}

/// Look up GitHub Latest from the browser, distinguishing absence from failure.
#[cfg(feature = "hydrate")]
async fn load_latest() -> Result<Option<DesktopRelease>, ()> {
    let response = gloo_net::http::Request::get(
        "https://api.github.com/repos/jhelwig/clacks-log/releases/latest",
    )
    .header("Accept", "application/vnd.github+json")
    .send()
    .await
    .map_err(|_| ())?;

    match response.status() {
        404 => Ok(None),
        200 => {
            let release = response
                .json::<release::GitHubRelease>()
                .await
                .map_err(|_| ())?;
            release.into_desktop_release().map(Some)
        }
        _ => Err(()),
    }
}

/// Leave the browser-owned lookup pending during server rendering.
#[cfg(not(feature = "hydrate"))]
async fn load_latest() -> Result<Option<DesktopRelease>, ()> {
    // LocalResource only runs on the client. The server renders the loading state.
    std::future::pending().await
}

/// Present one release with its installer choices and formatted notes.
#[component]
fn ReleaseDetails(release: DesktopRelease) -> impl IntoView {
    let tag = release.version.tag();
    let compatibility = LocalResource::new(move || {
        let tag = tag.clone();
        async move {
            #[cfg(feature = "hydrate")]
            {
                compatibility::load_compatibility(&tag).await.message()
            }
            #[cfg(not(feature = "hydrate"))]
            {
                let _ = tag;
                std::future::pending::<&'static str>().await
            }
        }
    });
    view! {
        <p class="muted download-version">"Current release "<strong>{release.version.as_str().to_owned()}</strong></p>
        <Suspense fallback=|| view! { <p class="muted" role="status">"Checking sync compatibility with this server…"</p> }>
            {move || compatibility.get().map(|status| view! { <p class="muted" role="status">{status}</p> })}
        </Suspense>
        <div class="download-platforms">
            <Installer
                platform="Windows"
                requirements="Windows 11 · x64"
                format="MSI installer (.msi)"
                url=release.windows
            />
            <Installer
                platform="macOS"
                requirements="macOS 26 Tahoe or newer · Apple Silicon"
                format="Disk image (.dmg)"
                url=release.macos
            />
        </div>
        {release.notes.map(|notes| view! {
            <section class="release-notes">
                <h2>"Release notes"</h2>
                <div inner_html=comrak::markdown_to_html(&notes, &comrak::Options::default())></div>
            </section>
        })}
    }
}

/// Show one platform's requirements and its download or unavailable state.
#[component]
fn Installer(
    platform: &'static str,
    requirements: &'static str,
    format: &'static str,
    url: Option<String>,
) -> impl IntoView {
    view! {
        <section class="download-platform">
            <h2>{platform}</h2>
            <p class="muted">{requirements}</p>
            {match url {
                Some(url) => view! {
                    <a class="public-action" href=url>{format!("Download {platform} installer")}</a>
                    <p class="muted installer-format">{format}</p>
                }.into_any(),
                None => view! {
                    <p class="muted">{format!("{platform} installer unavailable.")}</p>
                }.into_any(),
            }}
        </section>
    }
}
