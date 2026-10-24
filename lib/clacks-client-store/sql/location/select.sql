SELECT position_kind, locator, latitude, longitude, grid_length, dxcc, primary_subdivision, secondary_subdivision, cq_zone, itu_zone FROM recorded_location WHERE station_id = ? AND account = ?
