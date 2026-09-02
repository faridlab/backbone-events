-- Down: drop event.mail_registrations table
DROP TABLE IF EXISTS event.mail_registrations CASCADE;
DROP FUNCTION IF EXISTS event.mail_registrations_audit_timestamp() CASCADE;
