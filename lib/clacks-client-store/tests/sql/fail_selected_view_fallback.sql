CREATE TRIGGER fail_selected_view_fallback BEFORE UPDATE ON local_state
WHEN NEW.key = 'logbooks.selected_view'
BEGIN SELECT RAISE(ABORT, 'test selected-view fallback failure'); END;
