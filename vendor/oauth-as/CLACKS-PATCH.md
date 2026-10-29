# Clacks dependency correction

Base: the published oauth-as 1.0.0 crate, MIT OR Apache-2.0.

The sole source correction permits the native-app loopback redirect port to vary
while retaining exact loopback IP, raw path and query matching. The actual
requested URI is retained in the authorization code and must match at redemption.
The manifest adds url 2 for parsing. No token, proof, refresh or replay policy is
changed. Upstream issue: https://github.com/MattJackson/oauth-as/issues/26

Preserve upstream notices and license texts. Replace the correction with an
upstream release only after verifying equivalent native callback behavior.
