//! Directory setup through real browser authorization and production server actions.

use crate::support;
use leptos::server_fn::ServerFn;

#[tokio::test]
async fn directory_web_setup_never_returns_saved_password() -> anyhow::Result<()> {
    let provider = support::provider::start(None).await?;
    let temporary = tempfile::tempdir()?;
    let database = temporary.path().join("database");
    let cookie = temporary.path().join("cookie");
    let wrapping = temporary.path().join("wrapping");
    std::fs::write(
        &database,
        std::env::var("CLACKS_TEST_RUNTIME_DATABASE_URL")?,
    )?;
    std::fs::write(&cookie, [42u8; 64])?;
    std::fs::write(&wrapping, [19u8; 32])?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let config = serde_json::from_value(serde_json::json!({
        "public_url":base, "allow_http":true,"database_url_file":database,"cookie_key_file":cookie,
        "wrapping_keys":{"active":"directory-web","files":{"directory-web":wrapping}},
        "providers":{"test":{"issuer":provider.issuer,"label":"Fixture","mode":"preconfigured","client_id":"registered-clacks"}}
    }))?;
    let state = clacks_server::runtime::ServerState::connect(config).await?;
    let options = leptos::config::LeptosOptions::builder()
        .output_name("clacks-web")
        .build();
    let pages = clacks_web::routes(state.clone(), options);
    let router = clacks_server::http::router(state, pages)?;
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let start = client
        .post(format!("{base}{}", clacks_web::actions::StartSignIn::PATH))
        .form(&[("provider", "test")])
        .header("Origin", &base)
        .header("Accept", "text/html")
        .send()
        .await?;
    let authorization = url::Url::parse(start.headers().get("location").unwrap().to_str()?)?;
    let callback = provider.authorize(&authorization, &ulid::Ulid::generate().to_string())?;
    let signed = client
        .get(format!("{base}/auth/callback/test"))
        .query(&[("code", callback.code), ("state", callback.state)])
        .send()
        .await?;
    assert!(signed.status().is_redirection());
    let secret = format!("directory-web-secret-{}", ulid::Ulid::generate());
    let saved = client
        .post(format!(
            "{base}{}",
            clacks_web::actions::directory::SaveDirectory::PATH
        ))
        .header("Origin", &base)
        .form(&[
            ("expected_revision", "0"),
            ("selected", "qrz"),
            ("operation", "save"),
            ("provider", "qrz"),
            ("username", "fixture"),
            ("password", secret.as_str()),
        ])
        .send()
        .await?;
    assert!(saved.status().is_success());
    let response = saved.text().await?;
    assert!(!response.contains(&secret));
    assert!(response.contains("Committed"));
    let loaded = client
        .post(format!(
            "{base}{}",
            clacks_web::actions::directory::LoadDirectory::PATH
        ))
        .header("Origin", &base)
        .body("")
        .send()
        .await?
        .text()
        .await?;
    assert!(loaded.contains("Qrz"));
    assert!(loaded.contains("fixture"));
    assert!(!loaded.contains(&secret));
    let page = client
        .get(format!("{base}/account/directory"))
        .send()
        .await?
        .text()
        .await?;
    assert!(page.contains("Directory"));
    assert!(page.contains("fixture"));
    assert!(page.contains("••••••••"));
    assert!(!page.contains("Credential action"));
    assert!(!page.contains("Credential provider"));
    assert!(!page.contains("Saved credentials:"));
    assert!(!page.contains(&secret));

    // Each form submits only its own intent, preserving the other provider and selection.
    for (revision, selected, operation, provider, username, password) in [
        ("1", "qrz", "keep", "", "", ""),
        ("2", "", "save", "hamqth", "ham-user", "ham-password"),
        ("3", "", "save", "qrz", "renamed-user", ""),
        ("4", "", "save", "qrz", "renamed-user", "new-password"),
    ] {
        let response = client
            .post(format!(
                "{base}{}",
                clacks_web::actions::directory::SaveDirectory::PATH
            ))
            .header("Origin", &base)
            .form(&[
                ("expected_revision", revision),
                ("selected", selected),
                ("operation", operation),
                ("provider", provider),
                ("username", username),
                ("password", password),
            ])
            .send()
            .await?
            .text()
            .await?;
        assert!(response.contains("Committed"), "{response}");
        assert!(!response.contains(&secret));
        assert!(!response.contains("ham-password"));
        assert!(!response.contains("new-password"));
        let outcome: clacks_web::actions::directory::DirectorySaveOutcome =
            serde_json::from_str(&response)?;
        let clacks_web::actions::directory::DirectorySaveOutcome::Committed(configuration) =
            outcome
        else {
            unreachable!()
        };
        assert_eq!(
            configuration.selected,
            Some(clacks_model::directory::values::DirectoryProvider::Qrz)
        );
    }
    let removed = client
        .post(format!(
            "{base}{}",
            clacks_web::actions::directory::SaveDirectory::PATH
        ))
        .header("Origin", &base)
        .form(&[
            ("expected_revision", "5"),
            ("selected", ""),
            ("operation", "remove"),
            ("provider", "qrz"),
            ("username", ""),
            ("password", ""),
        ])
        .send()
        .await?
        .text()
        .await?;
    assert!(removed.contains("Committed"));
    let setup: clacks_web::actions::directory::DirectorySetup = client
        .post(format!(
            "{base}{}",
            clacks_web::actions::directory::LoadDirectory::PATH
        ))
        .header("Origin", &base)
        .body("")
        .send()
        .await?
        .json()
        .await?;
    assert!(
        !setup
            .usernames
            .contains_key(&clacks_model::directory::values::DirectoryProvider::Qrz)
    );
    assert_eq!(
        setup
            .usernames
            .get(&clacks_model::directory::values::DirectoryProvider::HamQth)
            .map(String::as_str),
        Some("ham-user")
    );
    for (username, password, message) in [
        ("unsaved-user", "", "Enter a password"),
        ("", "unsaved-password", "Enter a username"),
    ] {
        let refused = client
            .post(format!(
                "{base}{}",
                clacks_web::actions::directory::SaveDirectory::PATH
            ))
            .header("Origin", &base)
            .form(&[
                ("expected_revision", "6"),
                ("selected", ""),
                ("operation", "save"),
                ("provider", "qrz"),
                ("username", username),
                ("password", password),
            ])
            .send()
            .await?
            .text()
            .await?;
        assert!(refused.contains(message), "{refused}");
        assert!(!refused.contains("unsaved-password"));
    }
    let unchanged: clacks_web::actions::directory::DirectorySetup = client
        .post(format!(
            "{base}{}",
            clacks_web::actions::directory::LoadDirectory::PATH
        ))
        .header("Origin", &base)
        .body("")
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(unchanged.configuration, setup.configuration);
    assert_eq!(unchanged.usernames, setup.usernames);
    let foreign_origin = client
        .post(format!(
            "{base}{}",
            clacks_web::actions::directory::SaveDirectory::PATH
        ))
        .header("Origin", "https://unrelated.example")
        .form(&[
            ("expected_revision", "1"),
            ("selected", ""),
            ("operation", "keep"),
            ("provider", "qrz"),
            ("username", ""),
            ("password", ""),
        ])
        .send()
        .await?;
    assert_eq!(foreign_origin.status(), reqwest::StatusCode::FORBIDDEN);
    server.abort();
    Ok(())
}
