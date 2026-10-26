CREATE TRIGGER fail_logbook_window_progress BEFORE UPDATE ON window_progress
BEGIN SELECT RAISE(ABORT, 'test window progress failure'); END;
