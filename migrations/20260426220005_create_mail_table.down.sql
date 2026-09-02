-- Down: drop event.mails table
DROP TABLE IF EXISTS event.mails CASCADE;
DROP FUNCTION IF EXISTS event.mails_audit_timestamp() CASCADE;
