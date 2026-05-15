//! Wealthfolio Spending Module
//!
//! Optional, additive spending tracking for cash and credit-card accounts. Sibling crate to
//! `wealthfolio-core`; depends on it for shared types (Account, Activity, Taxonomy).
//! Mirrors the `wealthfolio-ai` / `wealthfolio-device-sync` leaf-crate pattern.
//!
//! # Isolation contract
//!
//! - This crate **does not edit** anything in `wealthfolio-core::portfolio`
//!   (snapshot / valuation / income / holdings / allocations).
//! - Spending accounts participate in net worth via the existing snapshot pipeline.
//!   Spending categorization, rules, events, and budget live entirely here.
//! - Spending is gated by a runtime toggle in `app_settings`. When the toggle is OFF,
//!   command handlers should early-return / no-op so the investment experience is
//!   byte-identical to a pre-spending build.
//!
//! # Module map
//!
//! - `settings` — enable toggle + spending-account opt-in list (stored in app_settings).
//! - `cash_activities` — query/CRUD for spending-account activities.
//! - `categories_seed` — boot-time seeder for the two scope=`activity` system taxonomies.
//! - `categorization_rules` — pattern-based auto-categorization (Gmail-filters style).
//! - `events` — first-class event entity (trips, holidays) with event_types.
//! - `budget` — monthly budget config and per-category allocations.
//! - `analytics` — aggregations for the Spending overview / reports pages.

pub mod activity_assignments;
mod activity_classification;
pub mod analytics;
pub mod budget;
pub mod cash_activities;
pub mod categories_seed;
pub mod categorization_rules;
pub mod events;
pub mod settings;
