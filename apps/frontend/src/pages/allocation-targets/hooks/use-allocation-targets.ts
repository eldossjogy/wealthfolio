import { useQuery } from "@tanstack/react-query";
import { listAllocationTargets } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import type { AllocationTarget } from "@/lib/types";

export function useAllocationTargets() {
  const {
    data: targets = [],
    isLoading,
    isError,
  } = useQuery<AllocationTarget[], Error>({
    queryKey: [QueryKeys.ALLOCATION_TARGETS],
    queryFn: listAllocationTargets,
  });

  return { targets, isLoading, isError };
}
