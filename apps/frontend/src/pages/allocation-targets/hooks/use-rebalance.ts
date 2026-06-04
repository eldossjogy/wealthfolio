import { useMutation } from "@tanstack/react-query";
import { calculateRebalancePlan } from "@/adapters";
import type { AccountScope, ScenarioMode } from "@/lib/types";

export function useCalculateRebalancePlan() {
  return useMutation({
    mutationFn: ({
      targetId,
      availableCash,
      filter,
      scenarioMode,
    }: {
      targetId: string;
      availableCash: number;
      filter: AccountScope;
      scenarioMode?: ScenarioMode;
    }) => calculateRebalancePlan(targetId, availableCash, filter, scenarioMode),
  });
}
