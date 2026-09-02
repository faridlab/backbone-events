-- Down: drop event.tags table
DROP TABLE IF EXISTS event.tags CASCADE;
DROP FUNCTION IF EXISTS event.tags_audit_timestamp() CASCADE;
