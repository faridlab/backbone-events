-- Down: drop event.types table
DROP TABLE IF EXISTS event.types CASCADE;
DROP FUNCTION IF EXISTS event.types_audit_timestamp() CASCADE;
