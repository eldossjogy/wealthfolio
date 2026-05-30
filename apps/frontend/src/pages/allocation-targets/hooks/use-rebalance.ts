import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { QueryKeys } from "@/lib/query-keys";
import {
  calculateRebalancePlan,
  saveRebalanceDraft,
  listRebalanceDrafts,
  deleteRebalanceDraft,
} from "@/adapters";
import type { AccountScope, RebalancePlan } from "@/lib/types";

export function useCalculateRebalancePlan() {
  return useMutation({
    mutationFn: ({
      targetId,
      availableCash,
      filter,
    }: {
      targetId: string;
      availableCash: number;
      filter: AccountScope;
    }) => calculateRebalancePlan(targetId, availableCash, filter),
  });
}

export function useSaveRebalanceDraft(targetId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      availableCash,
      filter,
      plan,
    }: {
      availableCash: number;
      filter: AccountScope;
      plan: RebalancePlan;
    }) => saveRebalanceDraft(targetId, availableCash, filter, plan),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QueryKeys.rebalanceDrafts(targetId) });
    },
  });
}

export function useRebalanceDrafts(targetId: string | null) {
  return useQuery({
    queryKey: QueryKeys.rebalanceDrafts(targetId ?? ""),
    queryFn: () => listRebalanceDrafts(targetId!),
    enabled: !!targetId,
  });
}

export function useDeleteRebalanceDraft(targetId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteRebalanceDraft(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QueryKeys.rebalanceDrafts(targetId) });
    },
  });
}
