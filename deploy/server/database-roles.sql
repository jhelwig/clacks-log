-- Run once as the deployment's PostgreSQL administrator, before release setup.
-- Supply passwords through deployment secret management, not this file.
CREATE ROLE clacks_migrator LOGIN;
CREATE ROLE clacks_runtime LOGIN;
CREATE ROLE clacks_maintenance LOGIN;
