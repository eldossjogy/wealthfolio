import { useQuery } from "@tanstack/react-query";
import { listTargetProfiles } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import type { TargetProfile } from "@/lib/types";

export function useTargetProfiles() {
  const {
    data: profiles = [],
    isLoading,
    isError,
  } = useQuery<TargetProfile[], Error>({
    queryKey: [QueryKeys.TARGET_PROFILES],
    queryFn: listTargetProfiles,
  });

  const activeProfile = profiles.find((p) => p.status === "active") ?? profiles[0] ?? null;

  return { profiles, activeProfile, isLoading, isError };
}
