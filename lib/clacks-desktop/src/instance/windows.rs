//! Windows file identity and current-user-only named-pipe permissions.

#[cfg(test)]
mod tests;

use super::{ActivationAddress, suffix};
use interprocess::{
    local_socket::{GenericNamespaced, ListenerOptions, ToNsName},
    os::windows::{local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor},
};
use std::{
    fs::File,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
};
use widestring::{U16CStr, U16CString};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    },
    Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

/// Obtain the current process token user's security identifier for local access control.
fn current_user() -> anyhow::Result<String> {
    let mut token = ptr::null_mut();
    // SAFETY: output points to writable handle storage; the pseudo process handle
    // is borrowed, and the returned token is immediately owned for proper cleanup.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: OpenProcessToken succeeded and returned an owned valid handle.
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut required = 0;
    // SAFETY: this first call requests the required buffer length only.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            ptr::null_mut(),
            0,
            &mut required,
        );
    }
    anyhow::ensure!(
        required > 0,
        "could not determine current user identity size"
    );
    let mut storage =
        vec![0_usize; usize::try_from(required)?.div_ceil(std::mem::size_of::<usize>())];
    // SAFETY: storage has the requested byte capacity and pointer alignment, and
    // the token remains owned for the entire query.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            storage.as_mut_ptr().cast(),
            required,
            &mut required,
        )
    } == 0
    {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: successful TokenUser query initializes TOKEN_USER within the aligned
    // buffer, whose allocation stays alive while the embedded identifier is read.
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let mut text = ptr::null_mut();
    // SAFETY: the identifier points into the live token buffer; text receives an
    // OS-allocated null-terminated string released below with LocalFree.
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: successful conversion returns a valid null-terminated UTF-16 string.
    let decoded = unsafe { U16CStr::from_ptr_str(text) }.to_string();
    // SAFETY: this pointer was allocated by ConvertSidToStringSidW and is not used again.
    unsafe {
        LocalFree(text.cast());
    }
    Ok(decoded?)
}

/// Derive a stable endpoint from OS file identity and the current token user.
pub(super) fn address(file: &File) -> anyhow::Result<ActivationAddress> {
    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: the file handle is borrowed/live and the output has appropriate size/alignment.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: the successful call fully initializes BY_HANDLE_FILE_INFORMATION.
    let information = unsafe { information.assume_init() };
    let index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    let identity = format!(
        "{}:{index}:{}",
        information.dwVolumeSerialNumber,
        current_user()?
    );
    let name =
        format!("clacks-log-{}", suffix(identity.as_bytes())).to_ns_name::<GenericNamespaced>()?;
    Ok(ActivationAddress { name })
}

/// Bind with a protected access-control list granting only the current token user.
pub(super) fn owned_options(
    address: &ActivationAddress,
) -> anyhow::Result<ListenerOptions<'static>> {
    let descriptor = U16CString::from_str(format!("D:P(A;;GA;;;{})", current_user()?))?;
    let descriptor = SecurityDescriptor::deserialize(&descriptor)?;
    Ok(ListenerOptions::new()
        .name(address.name.clone())
        .security_descriptor(descriptor))
}

/// Bind the user-qualified pipe with its explicit current-user descriptor.
pub(super) fn bind(
    address: &ActivationAddress,
) -> anyhow::Result<interprocess::local_socket::tokio::Listener> {
    Ok(owned_options(address)?.create_tokio()?)
}
