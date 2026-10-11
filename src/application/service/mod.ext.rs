// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// Hand-written verb/contract services (user-owned; see
// metaphor.codegen.yaml). event_service and registration_service
// fill the files the generator declares above but skips emitting for
// read-only models (the CRUD alias keeps lib.rs's wiring compiling);
// the rest are the module's own surfaces.
pub mod event_error;
pub mod capability;
pub mod template_port;
pub mod seat_service;
pub mod ics_service;
pub mod my_tickets_service;
pub mod intake_service;
pub mod scheduler_service;
pub mod event_surface;
pub mod sale_seam_service;
pub mod booth_command_service;
pub mod lead_sink;
pub mod lead_command_service;
pub mod lead_generation_service;
pub mod sms_port;
pub mod desk_service;
