-- Down: drop the event URL shape guarantee.

ALTER TABLE event.events DROP CONSTRAINT IF EXISTS events_url_scheme;
