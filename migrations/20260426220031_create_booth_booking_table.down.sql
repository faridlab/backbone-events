-- Down: drop event.booth_bookings table
DROP TABLE IF EXISTS event.booth_bookings CASCADE;
DROP FUNCTION IF EXISTS event.booth_bookings_audit_timestamp() CASCADE;
