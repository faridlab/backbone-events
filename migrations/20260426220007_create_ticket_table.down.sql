-- Down: drop event.tickets table
DROP TABLE IF EXISTS event.tickets CASCADE;
DROP FUNCTION IF EXISTS event.tickets_audit_timestamp() CASCADE;
