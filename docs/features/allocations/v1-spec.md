# Allocation Targets V1 Specification

Status: Draft  
Date: 2026-05-07  
Audience: Product, frontend, backend, desktop, web

## 1. Purpose

This document scopes a practical v1 for allocation targets and rebalancing in
Wealthfolio. It is intentionally smaller than the SOTA north-star model in
`docs/features/allocations/sota-target-model-spec.md`.

V1 should let users:

- Define a target allocation for a portfolio or account.
- See current vs target allocation and drift.
- Know when a target is out of tolerance.
- Enter available cash and generate suggested manual trades.
- Use cash-only planning first, with an optional simple sell-to-rebalance mode.

V1 should not implement tax-lot logic, wash-sale checks, lot disposal strategy,
tax reporting, household/account-group optimization, or broker execution. Those
belong to separate planned features and should integrate later through explicit
extension points.

## 2. Product Positioning

Use conservative product language.

Recommended user-facing names:

- **Allocation Targets**
- **Target & Drift**
- **Rebalance Planner**
- **Suggested manual trades**

Avoid user-facing names that imply regulated advice:

- Allocation Advisor
- Trade recommendations
- Tax optimization
- Advisor strategy

The feature should help users model their own target and produce transparent
manual suggestions. It should not present itself as personalized financial or
tax advice.

## 3. V1 Scope

### In Scope

- One active target profile per scope.
- Scope types:
  - whole portfolio.
  - single account.
- Primary taxonomy target, defaulting to `asset_classes`.
- Target categories can include categories with zero current holdings.
- Targets stored in basis points.
- Drift calculation.
- Tolerance band trigger.
- Manual rebalance planning.
- Cash-flow-only planning.
- Optional simple sell-to-rebalance planning if enabled.
- Minimum trade amount.
- Whole-share vs fractional planning.
- Save draft only when the user explicitly clicks save.
- Export suggested trades as CSV.
- Desktop and web adapter parity.

### Out of Scope

- Tax lots.
- Lot disposal method such as FIFO, LIFO, HIFO, loss-first, or long-term-first.
- Wash-sale detection.
- Realized gain caps.
- Tax-loss harvesting.
- Asset location optimization.
- Account groups or household rebalancing.
- Holding-level target percentages.
- Guardrail persistence.
- Value averaging in core.
- Model marketplace.
- Broker order execution.
- Persisting immutable runs for every calculation.

## 4. Conceptual Model

The full conceptual model remains:

```text
Target
  -> Rebalance trigger
  -> Funding input
  -> Execution constraints
  -> Suggested manual trades
```

V1 does not need separate tables for every concept. Keep the concepts distinct
in service code and UI copy, but persist them compactly.

| Concept | V1 Persistence |
| --- | --- |
| Target profile | `target_profiles` |
| Allocation sleeves | `target_allocation_nodes` |
| Rebalance trigger | Inline fields on `target_profiles` |
| Funding input | Request-time input, not persisted |
| Execution constraints | Inline simple fields on `target_profiles` |
| Suggested trades | Computed on demand; save to `rebalance_drafts` only by user action |
| Tax lots | Not in this feature |

## 5. Data Model

V1 uses two required tables and one optional save-draft table.

### 5.1 target_profiles

Stores target metadata, trigger settings, and simple execution defaults.

```sql
CREATE TABLE target_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    scope_type TEXT NOT NULL,
    scope_id TEXT,
    taxonomy_id TEXT NOT NULL DEFAULT 'asset_classes',
    base_currency TEXT NOT NULL,

    trigger_type TEXT NOT NULL DEFAULT 'threshold',
    drift_band_bps INTEGER NOT NULL DEFAULT 500,
    rebalance_to TEXT NOT NULL DEFAULT 'nearest_band',

    allow_sells INTEGER NOT NULL DEFAULT 0,
    min_trade_amount TEXT NOT NULL DEFAULT '0',
    whole_shares_only INTEGER NOT NULL DEFAULT 0,

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Field rules:

- `status`: `draft`, `active`, or `archived`.
- `scope_type`: `portfolio` or `account`.
- `scope_id`: null for portfolio, account id for account scope.
- `trigger_type`: v1 supports `manual` and `threshold`.
- `drift_band_bps`: absolute tolerance band. `500` means +/-5 percentage
  points.
- `rebalance_to`: `nearest_band` or `exact_target`.
- `allow_sells`: false by default.
- `min_trade_amount`: decimal string for Rust `Decimal`.
- Only one active profile may exist for the same `(scope_type, scope_id)`.

Indexes:

```sql
CREATE INDEX idx_target_profiles_scope
ON target_profiles(scope_type, scope_id, status);
```

### 5.2 target_allocation_nodes

Stores target percentages for categories in the profile taxonomy.

```sql
CREATE TABLE target_allocation_nodes (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    target_bps INTEGER NOT NULL,
    is_locked INTEGER NOT NULL DEFAULT 0,
    is_required INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (profile_id) REFERENCES target_profiles(id) ON DELETE CASCADE
);
```

Rules:

- All nodes under a profile reference categories from
  `target_profiles.taxonomy_id`.
- Top-level target nodes must sum to exactly `10000`.
- `target_bps` must be between `0` and `10000`.
- Zero-current categories are valid and should be selectable from the taxonomy.
- `is_required` controls whether a zero-current target appears in drift and
  breach calculations.
- V1 does not support child nodes or holding-level targets.

Indexes:

```sql
CREATE INDEX idx_target_allocation_nodes_profile
ON target_allocation_nodes(profile_id);
```

### 5.3 rebalance_drafts

Optional. Only create a row when the user explicitly saves a generated plan.
Do not persist every calculation.

```sql
CREATE TABLE rebalance_drafts (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    profile_snapshot_json TEXT NOT NULL,
    input_json TEXT NOT NULL,
    result_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (profile_id) REFERENCES target_profiles(id)
);
```

Notes:

- JSON is acceptable for v1 drafts because drafts are user artifacts, not core
  query surfaces.
- The snapshot makes saved drafts understandable even if the profile changes.
- Tax-lot details must not be stored here until the tax-lot feature defines its
  own data model.

## 6. Backend Design

### 6.1 Module Shape

Recommended core module:

```text
crates/core/src/portfolio/allocation_targets/
  mod.rs
  model.rs
  traits.rs
  target_service.rs
  drift_service.rs
  rebalance_service.rs
  validation.rs
```

Recommended storage module:

```text
crates/storage-sqlite/src/portfolio/allocation_targets/
  mod.rs
  model.rs
  repository.rs
```

Application layers:

```text
apps/tauri/src/commands/allocation_targets.rs
apps/server/src/api/allocation_targets.rs
apps/frontend/src/adapters/shared/allocation-targets.ts
```

### 6.2 Services

TargetService:

- Create profile.
- Update profile.
- List profiles by scope.
- Get profile detail.
- Activate profile.
- Archive profile.
- Save allocation nodes.
- Validate target sum.

DriftService:

- Load active profile.
- Load current allocations from existing allocation service.
- Merge target categories with current allocation categories.
- Include zero-current target categories.
- Compute drift, target value, and status.

RebalanceService:

- Calculate a plan on demand.
- Support cash-flow-only mode.
- Support simple sell-to-rebalance mode only when `allow_sells` is true.
- Apply min trade amount.
- Apply whole-share rounding if enabled.
- Return suggested manual trades and before/after allocation.
- Save draft only on explicit request.

### 6.3 Drift Math

Use one sign convention everywhere:

```text
target_value = total_value * target_bps / 10000
current_bps = current_value / total_value * 10000
drift_bps = current_bps - target_bps
value_delta = current_value - target_value
```

Interpretation:

- `drift_bps > 0`: overweight.
- `drift_bps < 0`: underweight.
- `value_delta > 0`: dollars above target.
- `value_delta < 0`: dollars needed.

Out-of-band check:

```text
abs(drift_bps) > drift_band_bps
```

### 6.4 Rebalance Planning

Input:

```typescript
interface CalculateRebalancePlanInput {
  profileId: string;
  availableCash: string;
  mode: "cash_flow_only" | "sell_to_rebalance";
}
```

Rules:

- Empty `availableCash` is parsed as zero.
- `cash_flow_only` never sells.
- `sell_to_rebalance` is rejected unless profile `allow_sells` is true.
- If `rebalance_to = nearest_band`, generate the minimum trade amount needed to
  bring breached sleeves inside band.
- If `rebalance_to = exact_target`, aim for target percentages.
- Do not generate trades below `min_trade_amount`.
- If `whole_shares_only` is true and quote is missing or non-positive, skip that
  asset with a warning.

Cash-flow-only algorithm:

1. Find underweight target nodes.
2. Compute needed dollars per underweight node.
3. Scale needs to available cash.
4. Pick a buy candidate:
   - existing holding in the sleeve if available.
   - otherwise user-selected default buy asset if future UI provides one.
   - otherwise return a sleeve-level suggestion without ticker.
5. Apply min trade and whole-share rules.

Sell-to-rebalance algorithm:

1. Compute overweight nodes.
2. Sell from existing holdings in overweight sleeves.
3. For v1, sell proportionally by market value within the overweight sleeve.
4. Do not choose tax lots.
5. Do not estimate gains.
6. Add sell proceeds to available cash.
7. Buy underweight sleeves using cash-flow algorithm.

Trade output should be honest when no concrete asset can be selected:

- Sleeve-level suggestion: "Buy $2,500 of Bonds".
- Asset-level suggestion: "Buy 12.3 shares of BND".

Do not force fake ticker suggestions.

### 6.5 Tax-Lot Extension Point

Tax lots and disposal rules are planned in another feature. V1 should leave a
clean integration point without schema pollution.

Future extension should be able to provide:

```rust
trait TaxLotPlanningService {
    fn estimate_trade_tax_impact(...);
    fn rank_sell_candidates(...);
    fn detect_wash_sale_risk(...);
}
```

V1 RebalanceService should depend only on an optional trait boundary or no-op
placeholder, not on persisted tax-lot columns.

## 7. API and Adapter Surface

Every command must work in desktop and web builds.

Suggested frontend adapter functions:

```typescript
export async function listTargetProfiles(
  scope?: TargetScope,
): Promise<TargetProfileSummary[]>;

export async function getTargetProfile(
  profileId: string,
): Promise<TargetProfileDetail>;

export async function saveTargetProfile(
  input: SaveTargetProfileInput,
): Promise<TargetProfileDetail>;

export async function activateTargetProfile(
  profileId: string,
): Promise<TargetProfileDetail>;

export async function archiveTargetProfile(
  profileId: string,
): Promise<void>;

export async function getTargetDrift(
  input: TargetDriftInput,
): Promise<TargetDriftReport>;

export async function calculateRebalancePlan(
  input: CalculateRebalancePlanInput,
): Promise<RebalancePlan>;

export async function saveRebalanceDraft(
  input: SaveRebalanceDraftInput,
): Promise<RebalanceDraft>;

export async function exportRebalancePlanCsv(
  input: ExportRebalancePlanInput,
): Promise<ExportResult>;
```

Keep handlers thin:

- Frontend validates form shape.
- Tauri/Axum validates request DTO shape.
- Core validates financial rules.
- Repository persists only validated models.

## 8. Frontend UX

V1 should use one main feature area, not three top-level pages.

Recommended navigation label:

- **Allocation Targets**

Recommended layout:

```text
Allocation Targets
  Header
    Scope selector
    Active target profile
    Target status
    Max drift
    Set targets
    Plan rebalance

  Main panel
    Current vs target chart
    Drift table
    Holdings drilldown

  Set targets panel
    Profile name
    Scope
    Taxonomy
    Target rows
    Drift band
    Rebalance-to setting
    Allow sells toggle
    Minimum trade amount
    Whole shares toggle

  Plan rebalance drawer
    Available cash
    Scenario selector
    Before/after allocation
    Suggested manual trades
    Warnings
    Save draft
    Export CSV
```

### 8.1 Main View

Show:

- Current allocation chart.
- Current vs target rows.
- Drift bps and dollar delta.
- Out-of-band badge.
- Click row to inspect holdings.

Rows must include:

| Column | Description |
| --- | --- |
| Sleeve | Category |
| Current | Current percent and value |
| Target | Target percent |
| Drift | Current - target |
| Status | In band, underweight, overweight |

### 8.2 Set Targets Panel

Requirements:

- Load categories from taxonomy, not current holdings.
- Allow categories with zero current value.
- Show total target sum.
- Disable save when total is not 100%.
- Store percentages as basis points.
- Use inputs for exact values, not only sliders.
- Keep drift-band configuration simple: one absolute band for the whole profile.

### 8.3 Plan Rebalance Drawer

Inputs:

- Available cash.
- Scenario:
  - Cash-flow only.
  - Sell to rebalance, shown only when profile allows sells.

Outputs:

- Cash used.
- Cash remaining.
- Estimated max drift after plan.
- Suggested manual trades.
- Warnings:
  - missing quote.
  - no buy candidate.
  - whole-share rounding left cash.
  - sells disabled.
  - tax impact not estimated.

Copy requirements:

- Use "suggested manual trades".
- Use "tax impact not estimated" when sells are shown.
- Do not use "tax-aware" or "optimized" in v1.

## 9. Type Shapes

### 9.1 TargetProfile

```typescript
interface TargetProfile {
  id: string;
  name: string;
  status: "draft" | "active" | "archived";
  scopeType: "portfolio" | "account";
  scopeId: string | null;
  taxonomyId: string;
  baseCurrency: string;
  triggerType: "manual" | "threshold";
  driftBandBps: number;
  rebalanceTo: "nearest_band" | "exact_target";
  allowSells: boolean;
  minTradeAmount: string;
  wholeSharesOnly: boolean;
  createdAt: string;
  updatedAt: string;
}
```

### 9.2 TargetAllocationNode

```typescript
interface TargetAllocationNode {
  id: string;
  profileId: string;
  categoryId: string;
  categoryName: string;
  color: string;
  targetBps: number;
  isLocked: boolean;
  isRequired: boolean;
}
```

### 9.3 TargetDriftRow

```typescript
interface TargetDriftRow {
  categoryId: string;
  categoryName: string;
  color: string;
  currentBps: number;
  targetBps: number;
  driftBps: number;
  currentValue: string;
  targetValue: string;
  valueDelta: string;
  status: "in_band" | "underweight" | "overweight" | "not_targeted";
  isRequired: boolean;
  isZeroCurrent: boolean;
}
```

### 9.4 RebalancePlan

```typescript
interface RebalancePlan {
  profileId: string;
  mode: "cash_flow_only" | "sell_to_rebalance";
  availableCash: string;
  cashUsed: string;
  cashRemaining: string;
  maxDriftBpsBefore: number;
  maxDriftBpsAfter: number;
  trades: SuggestedManualTrade[];
  warnings: RebalanceWarning[];
}
```

### 9.5 SuggestedManualTrade

```typescript
interface SuggestedManualTrade {
  action: "buy" | "sell";
  accountId: string | null;
  categoryId: string;
  categoryName: string;
  assetId: string | null;
  symbol: string | null;
  name: string | null;
  quantity: string | null;
  estimatedPrice: string | null;
  estimatedAmount: string;
  reason: string;
}
```

## 10. Validation

Profile:

- Name is required.
- Scope type is required.
- Account scope requires `scope_id`.
- Portfolio scope requires null `scope_id`.
- Taxonomy id is required.
- Drift band must be between 0 and 10000.
- Minimum trade amount must be >= 0.

Targets:

- Sum must equal 10000 bps.
- No target below 0.
- No target above 10000.
- Category ids must exist in the selected taxonomy.
- Duplicate category ids are rejected.

Planning:

- Empty cash input becomes zero.
- Negative cash input is rejected.
- Sell mode is rejected when `allow_sells` is false.
- Whole-share mode requires quote price for asset-level trades.
- Missing quote should produce a warning, not a crash.

## 11. Rollout

### Milestone 1: Targets and Drift

- Tables and repository.
- TargetService.
- DriftService.
- Tauri and web APIs.
- Allocation Targets page.
- Set targets panel.
- Current vs target drift view.

### Milestone 2: Cash-Flow Planner

- RebalanceService cash-flow-only mode.
- Available cash input.
- Suggested manual buys.
- Minimum trade amount.
- Whole/fractional behavior.
- CSV export.

### Milestone 3: Simple Sell Mode

- `allow_sells` profile setting.
- Sell-to-rebalance scenario.
- Proportional sell suggestions.
- Clear "tax impact not estimated" warnings.
- Save draft.

### Milestone 4: Integration Hooks

- Add optional trait boundary for future tax-lot planner.
- Add no-op tax warning provider.
- Keep tax-lot schema out of this feature.

## 12. Acceptance Criteria

- User can create a portfolio-level target profile.
- User can create an account-level target profile.
- User can activate exactly one target profile per scope.
- User can target a category they do not currently own.
- Target rows must sum to 100% before activation.
- Drift uses `current - target` consistently.
- Underweight rows show negative drift.
- Overweight rows show positive drift.
- Cash-flow-only mode never generates sells.
- Sell mode cannot run unless enabled on the profile.
- Empty available cash is handled as zero.
- Missing quotes create warnings.
- Suggested trades have plain-language reasons.
- CSV export includes only the currently displayed suggested trades.
- Desktop and web builds expose equivalent commands.

## 13. Test Plan

Rust unit tests:

- Target sum validation.
- One active profile per scope.
- Zero-current category appears in drift.
- Drift sign convention.
- Threshold breach detection.
- Cash-flow-only plan with limited cash.
- Cash-flow-only plan with zero cash.
- Minimum trade filtering.
- Whole-share rounding.
- Sell mode disabled rejection.
- Sell mode proportional sells.
- Missing quote warning.

Repository tests:

- Create/update/list profile.
- Activate profile archives or deactivates prior active profile for same scope.
- Save allocation nodes.
- Cascade delete draft profile nodes.
- Save and load rebalance draft.

Frontend tests:

- Set targets panel requires 100%.
- Taxonomy categories appear even with no holdings.
- Empty cash input submits zero.
- Sell scenario hidden unless enabled.
- Drift labels match sign.
- Export button disabled when no suggested trades exist.

Manual QA:

- Desktop target creation.
- Web target creation.
- Portfolio with only equity targeting bonds.
- Account with no holdings.
- Missing classification.
- Missing quote.
- Whole-share-only planning.
- Fractional planning.

## 14. Future Integration With Tax-Lot Feature

When tax reporting and lot disposal ship, extend V1 rather than redesigning it.

Expected integration points:

- Replace proportional sell candidate ranking with tax-lot-aware ranking.
- Add tax impact estimates to plan output.
- Add wash-sale warnings.
- Add lot ids to saved drafts only after lot persistence exists.
- Add execution policy fields in a new table or profile extension only when the
  tax feature defines the required data contracts.

Do not pre-add tax columns in V1 tables.

## 15. Open Questions

- Should sell-to-rebalance ship in v1, or should v1 be cash-flow-only?
- Should the default profile scope be portfolio or first account?
- Should `rebalance_to` default to `nearest_band` for all users?
- Which CSV export format should be supported first?
- Should saved drafts appear in the UI in v1, or is export enough?
