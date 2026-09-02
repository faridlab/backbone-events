-- Down: drop event.events table
DROP TABLE IF EXISTS event.events CASCADE;
DROP FUNCTION IF EXISTS event.events_audit_timestamp() CASCADE;
