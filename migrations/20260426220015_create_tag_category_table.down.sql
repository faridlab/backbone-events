-- Down: drop event.tag_categories table
DROP TABLE IF EXISTS event.tag_categories CASCADE;
DROP FUNCTION IF EXISTS event.tag_categories_audit_timestamp() CASCADE;
