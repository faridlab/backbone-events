-- Down: drop event.registrations table
DROP TABLE IF EXISTS event.registrations CASCADE;
DROP FUNCTION IF EXISTS event.registrations_audit_timestamp() CASCADE;
