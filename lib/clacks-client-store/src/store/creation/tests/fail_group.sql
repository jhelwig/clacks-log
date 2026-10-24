CREATE TRIGGER fail_group BEFORE INSERT ON record_identity
WHEN NEW.kind = 'group'
BEGIN
    SELECT RAISE(ABORT, 'injected seed failure');
END;
