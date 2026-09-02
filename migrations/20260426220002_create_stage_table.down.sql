-- Down: drop event.stages table
DROP TABLE IF EXISTS event.stages CASCADE;
DROP FUNCTION IF EXISTS event.stages_audit_timestamp() CASCADE;
