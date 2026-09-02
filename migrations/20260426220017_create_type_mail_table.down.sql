-- Down: drop event.type_mails table
DROP TABLE IF EXISTS event.type_mails CASCADE;
DROP FUNCTION IF EXISTS event.type_mails_audit_timestamp() CASCADE;
