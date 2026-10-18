# Server credential protection: deployment options

Research checkpoint: 17 October 2026. D236 selects per-account envelope encryption with a deployment wrapping key. D16 requires server-managed protection for stored LoTW private keys; D63 requires account isolation; D97 distributes directory credentials to enrolled clients for protected local storage. D57 requires documented backup/recovery, D189 selects Linux containers, and D213 selects one Clacks application with PostgreSQL. No application implementation is authorized.

## Scope

This comparison concerns protection of stored publication/directory credentials and LoTW private keys. It does not change client operating-system secret storage or distribute LoTW signing keys to clients. The selected protection separates encrypted database contents from the deployment's decryption key. It addresses disclosure of the database/backup without the key; it does not make secrets inaccessible to a compromised running application authorized to use them.

## Options

| Approach | Fit and tradeoff |
| --- | --- |
| Application-encrypted credential records; deployment supplies a protected key file outside PostgreSQL | Self-hostable with the selected application/database deployment. Operator must protect and back up key material as well as the database. The running application can decrypt credentials it is authorized to use. |
| External secrets/key service, self-hosted or managed | Can centralize key policy, access and rotation operations. Adds an integration, operational dependency and bootstrap authorization. Appropriate for some deployments, but not needed as a mandatory MVP dependency. |
| Database/disk encryption alone | Useful infrastructure protection, but does not by itself establish separate protection for application-level database exports/backups. Its suitability depends on the exposure being addressed. |

Source evidence: [OWASP cryptographic-storage guidance](https://cheatsheetseries.owasp.org/cheatsheets/Cryptographic_Storage_Cheat_Sheet.html) distinguishes encryption layers, recommends separation of keys/data and discusses dedicated key-management tradeoffs. [Docker Compose secrets](https://docs.docker.com/compose/how-tos/use-secrets/) provides file-based secret delivery to services. [Vault's transit service](https://developer.hashicorp.com/vault/docs/secrets/transit) is an example of externally managed cryptographic operations, not a selected Clacks dependency. A mounted file is a delivery mechanism; it does not imply encrypted storage of the underlying host file or backup.

## Selected direction (D236)

For MVP, use independent randomly generated account encryption keys to encrypt each account's sensitive service-credential values in PostgreSQL, using established authenticated-encryption tooling. Protect those account keys by encrypting (wrapping) them with a deployment wrapping key supplied through a protected key file mounted into the Clacks container. Store the wrapped account keys alongside the encrypted credential records in PostgreSQL; provision the deployment key independently of the database and application image. Document key provisioning, access protection, backup and recovery together with database recovery. Do not generate a replacement key silently when existing credentials cannot be decrypted.

This is envelope encryption: one key encrypts the data, and another protects that key. [Google's envelope-encryption guidance](https://docs.cloud.google.com/kms/docs/envelope-encryption) explains this hierarchy and explicitly recommends against using the same data-encryption key across users. Its managed service is an example, not a proposed Clacks dependency. The per-account granularity here is the Clacks selection, not a claim that every envelope-encryption system uses one key per account.

Independent account keys limit the scope of an exposed account key and allow account-key rotation independently. Deployment-key rotation can rewrap account keys without re-encrypting every credential. They do not isolate accounts from compromise of the deployment wrapping key together with the database, or of the fully authorized running server. Authenticated encryption should bind wrapped keys to their account and credential ciphertext to its account and credential identity, so swapping stored values between accounts or records fails authentication.

The database alone is insufficient to recover encrypted credentials without the required key material. Backups containing that key need protection of their own. Historical backups may contain credentials removed from current state; D228 removal is not a promise to erase independent backups. Restore/publication procedures still follow D141.

External secrets/key-service integrations can be post-MVP rather than mandatory deployment components. Exact encryption format/library, encoding of account/credential binding, rotation procedure and key-version retention remain engineering design work. Recovery requires the deployment-key versions corresponding to the restored wrapped account keys. D236 selects the hierarchy and deployment direction; it does not select a custom cryptographic algorithm.
