CREATE TRIGGER fail_equipment_delete BEFORE INSERT ON payload
WHEN NEW.kind = 'equipment' AND NEW.deleted = 1
BEGIN SELECT RAISE(ABORT, 'test equipment deletion failure'); END
