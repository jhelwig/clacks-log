//! Native credential entry construction shared by desktop secret owners.

#[cfg(all(test, target_os = "macos"))]
mod tests {
    /// Both production secret owners must use app-isolated, local protected storage.
    #[test]
    fn macos_secrets_use_data_protection_without_user_presence_or_cloud_sync() -> anyhow::Result<()>
    {
        for service in [
            "com.w7hex.clacks-log.authorization",
            "com.w7hex.clacks-log.directory",
        ] {
            let entry = super::entry(service, "backend-selection-fixture")?;
            let protected = entry
                .as_any()
                .downcast_ref::<apple_native_keyring_store::protected::Cred>()
                .ok_or_else(|| {
                    anyhow::anyhow!("Clacks selected a non-protected Keychain backend")
                })?;
            assert_eq!(protected.service, service);
            assert_eq!(protected.account, "backend-selection-fixture");
            assert_eq!(
                protected.access_policy,
                apple_native_keyring_store::protected::AccessPolicy::WhenUnlocked
            );
            assert!(!protected.cloud_synchronize);
            assert!(protected.access_group.is_none());
        }
        Ok(())
    }
}

/// Resolve an app-owned native entry without changing global stores.
///
/// macOS uses the local Data Protection Keychain and the signed app's default
/// access group. Access requires a correctly provisioned app; no legacy fallback,
/// user-presence requirement or cloud synchronization is selected.
pub(crate) fn entry(service: &str, reference: &str) -> anyhow::Result<keyring_core::Entry> {
    #[cfg(target_os = "macos")]
    {
        use keyring_core::api::CredentialStoreApi;
        Ok(apple_native_keyring_store::protected::Store::new()?.build(service, reference, None)?)
    }
    #[cfg(windows)]
    {
        use keyring_core::api::CredentialStoreApi;
        let modifiers = [("persistence", "local")].into();
        Ok(windows_native_keyring_store::Store::new()?.build(
            service,
            reference,
            Some(&modifiers),
        )?)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (service, reference);
        anyhow::bail!("Protected desktop credential storage is not supported on this platform yet.")
    }
}
