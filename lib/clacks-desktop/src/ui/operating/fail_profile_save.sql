CREATE TRIGGER fail_profile_save BEFORE INSERT ON profile_payload BEGIN
    SELECT RAISE(ABORT, 'test profile write failure');
END;
