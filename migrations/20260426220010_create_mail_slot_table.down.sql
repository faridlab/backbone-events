-- Down: drop event.mail_slots table
DROP TABLE IF EXISTS event.mail_slots CASCADE;
DROP FUNCTION IF EXISTS event.mail_slots_audit_timestamp() CASCADE;
