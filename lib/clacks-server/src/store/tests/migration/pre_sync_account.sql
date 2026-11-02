-- Input from the account-only schema: an existing account and linked identity,
-- before synchronization metadata and accepted definitions were introduced.
WITH owner AS (
    INSERT INTO clacks.accounts (account_id) VALUES ($1) RETURNING account_id
)
INSERT INTO clacks.login_identities (identity_id, account_id, issuer, subject, display_name)
SELECT $2, account_id, 'https://migration-fixture.example', $3, 'Existing operator' FROM owner
