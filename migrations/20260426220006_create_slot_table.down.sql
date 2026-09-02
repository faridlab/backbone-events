-- Down: drop event.slots table
DROP TABLE IF EXISTS event.slots CASCADE;
DROP FUNCTION IF EXISTS event.slots_audit_timestamp() CASCADE;
