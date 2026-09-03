-- Down: drop event.booth_categories table
DROP TABLE IF EXISTS event.booth_categories CASCADE;
DROP FUNCTION IF EXISTS event.booth_categories_audit_timestamp() CASCADE;
