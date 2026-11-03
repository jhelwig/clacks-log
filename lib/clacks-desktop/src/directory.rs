//! Protected configuration and direct lookup owned by the actual desktop session.

pub(crate) mod credentials;
pub(crate) mod providers;
pub(crate) mod provisioning;

pub(crate) mod lookup;
#[cfg(test)]
pub(crate) mod test_support;
