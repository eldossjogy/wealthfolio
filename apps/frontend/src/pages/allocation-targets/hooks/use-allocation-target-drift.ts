import { useQuery } from "@tanstack/react-query";
import { getAllocationTargetDrift } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import type { AccountScope, DriftReport } from "@/lib/types";

interface UseAllocationTargetDriftOptions {
  includeHoldings?: boolean;
}

export function useAllocationTargetDrift(
  targetId: string | null,
  scope: AccountScope,
  options?: UseAllocationTargetDriftOptions,
) {
  const includeHoldings = options?.includeHoldings ?? false;
  const {
    data: driftReport,
    dataUpdatedAt,
    isLoading,
    isError,
  } = useQuery<DriftReport | null, Error>({
    queryKey: QueryKeys.allocationTargetDrift(targetId ?? "", scope, includeHoldings),
    queryFn: () => getAllocationTargetDrift(targetId!, scope, { includeHoldings }),
    enabled: !!targetId,
  });

  return { driftReport: driftReport ?? null, dataUpdatedAt, isLoading, isError };
}
