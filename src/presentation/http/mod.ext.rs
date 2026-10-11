// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// Hand-written route surfaces (user-owned; see metaphor.codegen.yaml):
// the bare-of-auth public capability tree (the token + throttle are
// the fence; the host nests it under the schema name) and the officer
// tree (the host mounts it behind company_auth + the module write
// gate keyed "event").
pub mod public_routes;
pub mod admin_routes;
pub use public_routes::{
    caller_ip, event_intake_routes, event_public_routes, trusted_proxy_from_env,
    EventPublicState, ICS_THROTTLE,
};
pub use admin_routes::{event_admin_routes, EventActor, EventAdminState};
