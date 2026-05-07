import { SettingsHeader } from "../settings-header";
import { useSpendingSettings } from "@/features/spending/hooks/use-spending-settings";

import { AccountsCard } from "./components/accounts-card";
import { BudgetOverviewCard } from "./components/budget-overview-card";
import { CategoriesOverviewCard } from "./components/categories-overview-card";
import { EventTypesOverviewCard } from "./components/event-types-overview-card";
import { ModuleCard } from "./components/module-card";
import { RulesOverviewCard } from "./components/rules-overview-card";

export default function SpendingSettingsPage() {
  const { isEnabled } = useSpendingSettings();

  return (
    <div className="space-y-6">
      <SettingsHeader
        heading="Spending Tracker"
        text="Track expenses on your cash accounts: categories, events, categorization rules, budgets, and reports."
      />

      <div className="space-y-4">
        <ModuleCard />

        {isEnabled && (
          <>
            <AccountsCard />
            <CategoriesOverviewCard variant="expense" />
            <CategoriesOverviewCard variant="income" />
            <EventTypesOverviewCard />
            <RulesOverviewCard />
            <BudgetOverviewCard />
          </>
        )}
      </div>
    </div>
  );
}
