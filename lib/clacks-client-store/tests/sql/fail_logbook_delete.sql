CREATE TRIGGER fail_logbook_delete BEFORE INSERT ON payload
WHEN NEW.kind = 'tag' AND NEW.deleted = 1
BEGIN SELECT RAISE(ABORT, 'test logbook deletion failure'); END;
