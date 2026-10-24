//! Inspect permissions on an actual Windows activation pipe, not just its template.

use super::*;
use interprocess::{
    local_socket::{tokio::Stream, traits::tokio::Stream as _},
    os::windows::security_descriptor::{AsSecurityDescriptorExt, BorrowedSecurityDescriptor},
};
use std::os::windows::io::AsHandle;
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT},
        DACL_SECURITY_INFORMATION, SE_DACL_PROTECTED,
    },
};

#[test]
fn pipe_descriptor_grants_only_the_current_user() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let paths = crate::instance::tests::paths(directory.path());
    let crate::instance::Election::Owner(owner) = crate::instance::elect(&paths)? else {
        panic!("owner");
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let (sender, _receiver) = tokio::sync::watch::channel(());
    let listener = owner.listen(runtime.handle(), sender)?;
    runtime.block_on(async {
        let stream = Stream::connect(owner.address.name.clone()).await?;
        let Stream::NamedPipe(stream) = stream;
        let mut raw = ptr::null_mut();
        // SAFETY: the pipe handle remains live; the function owns the returned
        // descriptor allocation, which is released after extracting its data.
        let status = unsafe {
            GetSecurityInfo(
                stream.inner().as_handle().as_raw_handle(),
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut raw,
            )
        };
        anyhow::ensure!(status == 0, "could not inspect actual pipe permissions");
        // SAFETY: successful GetSecurityInfo returned the initialized descriptor.
        let descriptor = unsafe { BorrowedSecurityDescriptor::from_ptr(raw.cast()) };
        let control = descriptor.control_and_revision();
        let encoded =
            descriptor.serialize(DACL_SECURITY_INFORMATION, |value| value.to_string_lossy());
        // SAFETY: raw was allocated by GetSecurityInfo and is no longer borrowed.
        unsafe {
            LocalFree(raw.cast());
        }
        assert_ne!(control?.0 & SE_DACL_PROTECTED, 0);
        let encoded = encoded?;
        assert_eq!(encoded.matches("(A;").count(), 1);
        assert!(encoded.contains(&current_user()?));
        assert!(!encoded.contains(";;;WD)"));
        Ok::<_, anyhow::Error>(())
    })?;
    runtime.block_on(listener.shutdown())?;
    Ok(())
}
