import { useMutation } from "@tanstack/react-query";
import { calculateRebalancePlan } from "@/adapters";
import type { AccountScope } from "@/lib/types";

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
