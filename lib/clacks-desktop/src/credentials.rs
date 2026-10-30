//! Native credential entry construction shared by desktop secret owners.

/// Resolve exactly one entry through the selected OS backend without changing global stores.
pub(crate) fn entry(service: &str, reference: &str) -> anyhow::Result<keyring_core::Entry> {
    #[cfg(target_os = "macos")]
    {
        use keyring_core::api::CredentialStoreApi;
        Ok(apple_native_keyring_store::keychain::Store::new()?.build(service, reference, None)?)
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
