//! Browser comparison of a selected desktop release with its serving deployment.

/// Public identifiers remain opaque even when the web build implements older sync code.
#[derive(serde::Deserialize)]
struct ContractDeclaration {
    /// Contracts offered by the release or accepted by the deployment.
    sync_contracts: Vec<String>,
}

/// Result of retrieving and comparing both declarations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Compatibility {
    /// At least one exact contract identifier is shared.
    Compatible,
    /// Valid declarations have no shared contract.
    Incompatible,
    /// Retrieval or decoding did not establish compatibility.
    Unknown,
}

#[cfg(feature = "hydrate")]
impl Compatibility {
    /// Status beside the selected release, independent of download availability.
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::Compatible => "This release is sync-compatible with this server.",
            Self::Incompatible => {
                "This release cannot sync with this server. Local offline use remains available."
            }
            Self::Unknown => "Sync compatibility with this server could not be verified.",
        }
    }
}

/// Match the two public declarations without restricting IDs to this web build's code.
fn compare(
    release: Option<&ContractDeclaration>,
    deployment: Option<&ContractDeclaration>,
) -> Compatibility {
    let (Some(release), Some(deployment)) = (release, deployment) else {
        return Compatibility::Unknown;
    };

    if release
        .sync_contracts
        .iter()
        .any(|id| deployment.sync_contracts.contains(id))
    {
        Compatibility::Compatible
    } else {
        Compatibility::Incompatible
    }
}

/// Contents request pinned to the tag already selected for displayed installers and notes.
fn metadata_url(tag: &str) -> String {
    format!(
        "https://api.github.com/repos/jhelwig/clacks-log/contents/deploy/desktop/sync-contracts.json?ref={tag}"
    )
}

/// Fetch both declarations without credentials or an application discovery cache.
#[cfg(feature = "hydrate")]
pub(super) async fn load_compatibility(tag: &str) -> Compatibility {
    let release = retrieve(&metadata_url(tag), false).await;
    let deployment = retrieve("/api/sync/capabilities", true).await;
    compare(release.as_ref().ok(), deployment.as_ref().ok())
}

/// Retrieve one public declaration, bypassing caches for deployment discovery.
#[cfg(feature = "hydrate")]
async fn retrieve(url: &str, deployment: bool) -> Result<ContractDeclaration, ()> {
    let mut request =
        gloo_net::http::Request::get(url).credentials(web_sys::RequestCredentials::Omit);
    if deployment {
        request = request.cache(web_sys::RequestCache::NoStore);
    } else {
        request = request.header("Accept", "application/vnd.github.raw+json");
    }

    let response = request.send().await.map_err(|_| ())?;
    if response.status() != 200 {
        return Err(());
    }

    response.json().await.map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use clacks_desktop_release::ReleaseVersion;
    #[cfg(feature = "ssr")]
    use leptos::prelude::*;

    /// Pending compatibility must not activate the release lookup's outer fallback.
    #[cfg(feature = "ssr")]
    #[tokio::test]
    async fn pending_compatibility_keeps_installers_and_notes_visible() -> anyhow::Result<()> {
        let _ = any_spawner::Executor::init_tokio();
        let version = ReleaseVersion::parse("26.11.1")?;

        let render = leptos_axum::render_app_to_stream(move || {
            let release = super::super::DesktopRelease {
                version,
                windows: Some("https://example.test/installer.msi".into()),
                macos: None,
                notes: Some("Candidate notes".into()),
            };
            view! {
                <Suspense fallback=|| view! { <p>"Loading release information…"</p> }>
                    <super::super::ReleaseDetails release/>
                </Suspense>
            }
        });
        let response = render(axum::http::Request::new(axum::body::Body::empty())).await;
        let html = String::from_utf8(
            axum::body::to_bytes(response.into_body(), usize::MAX)
                .await?
                .to_vec(),
        )?;

        assert!(
            html.contains("Checking sync compatibility with this server"),
            "{html}"
        );
        assert!(html.contains("https://example.test/installer.msi"));
        assert!(html.contains("Candidate notes"));
        assert!(!html.contains("Loading release information"));
        Ok(())
    }

    /// Selected-release metadata stays pinned and permits contracts newer than this web build.
    #[test]
    fn selected_release_compatibility_keeps_future_ids_and_tag_association() -> anyhow::Result<()> {
        let installer_url = format!(
            "{}/download/desktop-v26.11.1/Clacks-Log-26.11.1-Windows-x64.msi",
            super::super::RELEASES_URL
        );
        let selected: super::super::release::GitHubRelease =
            serde_json::from_value(serde_json::json!({
                "tag_name": "desktop-v26.11.1",
                "draft": false,
                "prerelease": false,
                "assets": [{
                    "name": "Clacks-Log-26.11.1-Windows-x64.msi",
                    "browser_download_url": installer_url
                }]
            }))?;
        let selected = selected
            .into_desktop_release()
            .map_err(|()| anyhow::anyhow!("fixture is a final desktop release"))?;
        assert!(metadata_url(&selected.version.tag()).ends_with("?ref=desktop-v26.11.1"));
        let release: ContractDeclaration =
            serde_json::from_str(r#"{"sync_contracts":["future-contract"]}"#)?;
        let shared: ContractDeclaration =
            serde_json::from_str(r#"{"sync_contracts":["future-contract"]}"#)?;
        let disjoint: ContractDeclaration =
            serde_json::from_str(r#"{"sync_contracts":["another-contract"]}"#)?;

        assert_eq!(
            compare(Some(&release), Some(&shared)),
            Compatibility::Compatible
        );
        assert_eq!(
            compare(Some(&release), Some(&disjoint)),
            Compatibility::Incompatible
        );

        let unavailable = serde_json::from_str::<ContractDeclaration>("{}").ok();
        assert_eq!(
            compare(Some(&release), unavailable.as_ref()),
            Compatibility::Unknown
        );
        Ok(())
    }
}
