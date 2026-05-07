//! Budget — monthly target + per-category allocations.
//! `budget_config` is a singleton (id="default") with a base monthly_spending_target +
//! monthly_income_target. `budget_allocations` stores per-category caps.

pub mod model;
pub mod service;
pub mod traits;

pub use model::{
    BudgetAllocation, BudgetConfig, BudgetSnapshot, NewBudgetAllocation, NewBudgetConfig,
    UpdateBudgetConfig,
};
pub use service::BudgetService;
pub use traits::BudgetRepositoryTrait;
