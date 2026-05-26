import { useQuery } from "@tanstack/react-query";
import { getTargetDriftForProfile } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import type { AccountScope, DriftReport } from "@/lib/types";

export function useTargetDrift(profileId: string | null, scope: AccountScope) {
  const {
    data: driftReport,
    isLoading,
    isError,
  } = useQuery<DriftReport | null, Error>({
    queryKey: QueryKeys.targetDrift(profileId ?? "", scope),
    queryFn: () => getTargetDriftForProfile(profileId!, scope),
    enabled: !!profileId,
  });

  return { driftReport: driftReport ?? null, isLoading, isError };
}
