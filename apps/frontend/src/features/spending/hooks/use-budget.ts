import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { QueryKeys } from "@/lib/query-keys";

import {
  deleteBudgetAllocation,
  getBudget,
  updateBudgetConfig,
  upsertBudgetAllocation,
} from "../adapters/budget";
import type { BudgetSnapshot, UpdateBudgetConfig } from "../types/budget";

export function useBudget() {
  return useQuery<BudgetSnapshot, Error>({
    queryKey: [QueryKeys.SPENDING_BUDGET],
    queryFn: getBudget,
  });
}

export function useBudgetMutations() {
  const qc = useQueryClient();
  const invalidate = () => qc.invalidateQueries({ queryKey: [QueryKeys.SPENDING_BUDGET] });

  const updateConfig = useMutation({
    mutationFn: (patch: UpdateBudgetConfig) => updateBudgetConfig(patch),
    onSuccess: () => {
      invalidate();
      toast.success("Budget updated.");
    },
    onError: () => toast.error("Failed to update budget."),
  });

  const upsertAllocation = useMutation({
    mutationFn: ({
      taxonomyId,
      categoryId,
      amount,
    }: {
      taxonomyId: string;
      categoryId: string;
      amount: string;
    }) => upsertBudgetAllocation(taxonomyId, categoryId, amount),
    onSuccess: () => {
      invalidate();
      toast.success("Allocation saved.");
    },
    onError: () => toast.error("Failed to save allocation."),
  });

  const removeAllocation = useMutation({
    mutationFn: (id: string) => deleteBudgetAllocation(id),
    onSuccess: () => {
      invalidate();
      toast.success("Allocation removed.");
    },
    onError: () => toast.error("Failed to delete allocation."),
  });

  return { updateConfig, upsertAllocation, removeAllocation };
}
