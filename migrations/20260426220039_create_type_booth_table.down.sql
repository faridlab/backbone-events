-- Down: drop event.type_booths table
DROP TABLE IF EXISTS event.type_booths CASCADE;
DROP FUNCTION IF EXISTS event.type_booths_audit_timestamp() CASCADE;
