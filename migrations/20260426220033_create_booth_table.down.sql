-- Down: drop event.booths table
DROP TABLE IF EXISTS event.booths CASCADE;
DROP FUNCTION IF EXISTS event.booths_audit_timestamp() CASCADE;
