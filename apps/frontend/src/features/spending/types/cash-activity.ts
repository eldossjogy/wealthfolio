import type { Activity } from "@/lib/types";

export interface CashActivityFilter {
  accountIds?: string[];
  startDate?: string;
  endDate?: string;
  activityTypes?: string[];
}

export interface ActivityTaxonomyAssignment {
  id: string;
  activityId: string;
  taxonomyId: string;
  categoryId: string;
  weight: number;
  source: string;
  createdAt: string;
  updatedAt: string;
}

export type CashActivity = Activity;

export type CashActivityStatusFilter = "all" | "needs_review" | "uncategorized" | "categorized";

export type CashActivitySortField = "date" | "amount";
export type CashActivitySortDirection = "asc" | "desc";

/** Search request — mirrors `wealthfolio_spending::cash_activities::CashActivitySearchRequest`. */
export interface CashActivitySearchRequest {
  search?: string;
  accountIds?: string[];
  activityTypes?: string[];
  categoryIds?: string[];
  subcategoryIds?: string[];
  eventIds?: string[];
  status?: CashActivityStatusFilter;
  startDate?: string;
  endDate?: string;
  minAmount?: number;
  maxAmount?: number;
  sortBy?: CashActivitySortField;
  sortDir?: CashActivitySortDirection;
  offset?: number;
  limit?: number;
}

/** Activity row enriched with its single-select assignments (typically 0 or 1). */
export interface CashActivityWithAssignments extends Activity {
  assignments: ActivityTaxonomyAssignment[];
}

export interface CashActivitySearchResponse {
  items: CashActivityWithAssignments[];
  totalCount: number;
}
