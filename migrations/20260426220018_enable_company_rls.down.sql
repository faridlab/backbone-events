-- Down: remove the company RLS fence for event module

-- Reverse the company RLS fence for event.events
DROP POLICY IF EXISTS events_company_isolation ON event.events;
ALTER TABLE event.events NO FORCE ROW LEVEL SECURITY;
ALTER TABLE event.events DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for event.registrations
DROP POLICY IF EXISTS registrations_company_isolation ON event.registrations;
ALTER TABLE event.registrations NO FORCE ROW LEVEL SECURITY;
ALTER TABLE event.registrations DISABLE ROW LEVEL SECURITY;

