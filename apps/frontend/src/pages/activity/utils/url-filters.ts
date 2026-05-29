import type { AccountScope } from "@/lib/types";
import type { ActivityStatusFilter } from "../hooks/use-activity-search";

interface ActivityUrlFilters {
  accountScope?: AccountScope;
  statusFilter?: ActivityStatusFilter;
}

export function resolveActivityUrlFilters(searchParams: URLSearchParams): ActivityUrlFilters {
  const accountId = searchParams.get("account")?.trim();
  const needsReview = searchParams.get("needsReview") === "true";

  return {
    ...(accountId ? { accountScope: { type: "account" as const, accountId } } : {}),
    ...(needsReview ? { statusFilter: "pending" as const } : {}),
  };
}
