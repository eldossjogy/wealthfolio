import { invoke, logger } from "#platform";
import type { BudgetAllocation, BudgetSnapshot, UpdateBudgetConfig } from "../types/budget";

export const getBudget = async (): Promise<BudgetSnapshot> => {
  try {
    return await invoke<BudgetSnapshot>("get_budget");
  } catch (e) {
    logger.error("Error fetching budget.");
    throw e;
  }
};

export const updateBudgetConfig = async (patch: UpdateBudgetConfig): Promise<BudgetSnapshot> => {
  try {
    return await invoke<BudgetSnapshot>("update_budget_config", { patch });
  } catch (e) {
    logger.error("Error updating budget config.");
    throw e;
  }
};

export const upsertBudgetAllocation = async (
  taxonomyId: string,
  categoryId: string,
  amount: string,
): Promise<BudgetAllocation> => {
  try {
    return await invoke<BudgetAllocation>("upsert_budget_allocation", {
      taxonomyId,
      categoryId,
      amount,
    });
  } catch (e) {
    logger.error("Error saving budget allocation.");
    throw e;
  }
};

export const deleteBudgetAllocation = async (id: string): Promise<void> => {
  try {
    await invoke<void>("delete_budget_allocation", { id });
  } catch (e) {
    logger.error("Error deleting budget allocation.");
    throw e;
  }
};
