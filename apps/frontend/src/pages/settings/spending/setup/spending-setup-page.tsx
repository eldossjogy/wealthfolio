import { Navigate } from "react-router-dom";

import { BudgetEditor } from "@/features/spending/components/budget-editor";
import { useSpendingSettings } from "@/features/spending/hooks/use-spending-settings";

import { SettingsHeader } from "../../settings-header";
import { SpendingBackLink } from "../components/spending-back-link";

export default function SpendingSetupPage() {
  const { isEnabled, isLoading: settingsLoading } = useSpendingSettings();

  if (!settingsLoading && !isEnabled) {
    return <Navigate to="/settings/spending" replace />;
  }

  return (
    <div className="space-y-5">
      <SpendingBackLink />

      <SettingsHeader
        heading="Budget setup"
        text="Define groups, default monthly targets, and rollover behavior. Used as the baseline for every month."
        backTo="/settings/spending"
      />

      {settingsLoading ? null : <BudgetEditor mode="setup" periodKey="default" />}
    </div>
  );
}
