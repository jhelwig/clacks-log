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
        ACCESS_ALLOWED_ACE,
        Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT},
        DACL_SECURITY_INFORMATION, GetAce, SE_DACL_PROTECTED,
    },
    System::SystemServices::ACCESS_ALLOWED_ACE_TYPE,
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
        let owned = descriptor.to_owned_sd();
        // SAFETY: raw was allocated by GetSecurityInfo; its borrowed view is no longer used.
        unsafe {
            LocalFree(raw.cast());
        }
        let descriptor = owned?;
        assert_ne!(descriptor.control_and_revision()?.0 & SE_DACL_PROTECTED, 0);

        let (acl, _) = descriptor.dacl()?.expect("pipe DACL is present");
        assert!(!acl.is_null());
        // SAFETY: the descriptor owns the initialized ACL throughout these assertions.
        assert_eq!(unsafe { (*acl).AceCount }, 1);
        let mut entry = ptr::null_mut();
        // SAFETY: index zero exists in this live, single-entry ACL.
        anyhow::ensure!(
            unsafe { GetAce(acl, 0, &mut entry) } != 0,
            "could not read pipe access entry"
        );
        // SAFETY: GetAce returned an initialized entry; its header identifies its layout.
        let header = unsafe { &*entry.cast::<windows_sys::Win32::Security::ACE_HEADER>() };
        assert_eq!(u32::from(header.AceType), ACCESS_ALLOWED_ACE_TYPE);
        // SAFETY: this is an ACCESS_ALLOWED_ACE, with the variable-length SID at SidStart.
        let sid = unsafe { ptr::addr_of!((*entry.cast::<ACCESS_ALLOWED_ACE>()).SidStart) };
        let mut text = ptr::null_mut();
        // SAFETY: the SID belongs to the live descriptor; text receives an owned OS string.
        anyhow::ensure!(
            unsafe { ConvertSidToStringSidW(sid.cast_mut().cast(), &mut text) } != 0,
            "could not decode pipe trustee"
        );
        // SAFETY: the successful conversion returned a null-terminated UTF-16 string.
        let trustee = unsafe { U16CStr::from_ptr_str(text) }.to_string();
        // SAFETY: the conversion allocated text with LocalAlloc, and it is no longer borrowed.
        unsafe {
            LocalFree(text.cast());
        }
        assert_eq!(trustee?, current_user()?);
        Ok::<_, anyhow::Error>(())
    })?;
    runtime.block_on(listener.shutdown())?;
    Ok(())
}
