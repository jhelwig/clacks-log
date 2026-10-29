-- Local-only admission holds survive ordinary record-role updates.
ALTER TABLE client_record ADD COLUMN authorization_hold TEXT;
