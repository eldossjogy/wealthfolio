import type { ActivityType } from "@/lib/constants";
import type { AccountScope } from "@/lib/types";
import type { ActivityStatusFilter } from "../hooks/use-activity-search";

interface ActivityUrlFilters {
  accountScope?: AccountScope;
  statusFilter?: ActivityStatusFilter;
  activityTypes?: ActivityType[];
  dateFrom?: string;
  dateTo?: string;
}

export function resolveActivityUrlFilters(searchParams: URLSearchParams): ActivityUrlFilters {
  const accountId = searchParams.get("account")?.trim();
  const needsReview = searchParams.get("needsReview") === "true";
  const typesRaw = searchParams.get("types")?.trim();
  const dateFrom = searchParams.get("from")?.trim();
  const dateTo = searchParams.get("to")?.trim();

  const activityTypes = typesRaw
    ? (typesRaw
        .split(",")
        .map((type) => type.trim())
        .filter(Boolean) as ActivityType[])
    : undefined;

  return {
    ...(accountId ? { accountScope: { type: "account" as const, accountId } } : {}),
    ...(needsReview ? { statusFilter: "pending" as const } : {}),
    ...(activityTypes && activityTypes.length > 0 ? { activityTypes } : {}),
    ...(dateFrom ? { dateFrom } : {}),
    ...(dateTo ? { dateTo } : {}),
  };
}

export function clearActivityUrlFilters(searchParams: URLSearchParams): URLSearchParams {
  const next = new URLSearchParams(searchParams);
  next.delete("account");
  next.delete("needsReview");
  next.delete("types");
  next.delete("from");
  next.delete("to");
  return next;
}
