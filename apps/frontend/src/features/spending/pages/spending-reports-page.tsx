import { useMemo } from "react";
import { useSearchParams } from "react-router-dom";

import { Tabs, TabsContent, TabsList, TabsTrigger, usePersistentState } from "@wealthfolio/ui";

import { useSettingsContext } from "@/lib/settings-provider";

import { ReportsHeader } from "../components/reports/reports-header";
import { CategoriesTab } from "../components/reports/tabs/categories-tab";
import { EventsTab } from "../components/reports/tabs/events-tab";
import { OverviewTab } from "../components/reports/tabs/overview-tab";
import {
  DEFAULT_COMPARISON,
  DEFAULT_REPORTS_PERIOD,
  periodToReportsRange,
  type ComparisonMode,
  type ReportsPeriod,
} from "../lib/reports-period";

const TABS = ["overview", "categories", "events"] as const;
type TabId = (typeof TABS)[number];

/**
 * Map legacy ?tab= values to the new tab set so dashboard deep-links
 * generated under the previous structure don't 404 the user back to "overview".
 */
const LEGACY_TAB_REDIRECTS: Record<string, TabId> = {
  trends: "overview",
  patterns: "overview",
};

const PERIOD_STORAGE_KEY = "spending-reports-period";
const COMPARISON_STORAGE_KEY = "spending-reports-comparison";

/**
 * Reports page — tabbed analytics.
 *
 * Period + comparison mode are owned by the page and lifted into every tab,
 * so switching tabs preserves "what window am I looking at".
 *
 * Active tab is reflected in the URL (?tab=…) so deep links from the
 * dashboard can land directly on a specific view.
 */
export default function SpendingReportsPage() {
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const [period, setPeriod] = usePersistentState<ReportsPeriod>(
    PERIOD_STORAGE_KEY,
    DEFAULT_REPORTS_PERIOD,
  );
  const [comparison, setComparison] = usePersistentState<ComparisonMode>(
    COMPARISON_STORAGE_KEY,
    DEFAULT_COMPARISON,
  );

  const range = useMemo(() => periodToReportsRange(period), [period]);

  const [searchParams, setSearchParams] = useSearchParams();
  const tabFromUrl = searchParams.get("tab");
  const activeTab: TabId = TABS.includes(tabFromUrl as TabId)
    ? (tabFromUrl as TabId)
    : tabFromUrl && tabFromUrl in LEGACY_TAB_REDIRECTS
      ? LEGACY_TAB_REDIRECTS[tabFromUrl]
      : "overview";

  const setActiveTab = (next: string) => {
    const params = new URLSearchParams(searchParams);
    params.set("tab", next);
    setSearchParams(params, { replace: true });
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 px-4 pb-6 pt-4 md:px-6 lg:px-8 lg:pb-8">
      <ReportsHeader
        period={period}
        onPeriodChange={setPeriod}
        comparison={comparison}
        onComparisonChange={setComparison}
      />

      <Tabs value={activeTab} onValueChange={setActiveTab} className="flex flex-1 flex-col">
        <TabsList className="self-start">
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="categories">Categories</TabsTrigger>
          <TabsTrigger value="events">Events</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="mt-4 flex-1">
          <OverviewTab range={range} comparison={comparison} currency={baseCurrency} />
        </TabsContent>
        <TabsContent value="categories" className="mt-4 flex-1">
          <CategoriesTab range={range} comparison={comparison} currency={baseCurrency} />
        </TabsContent>
        <TabsContent value="events" className="mt-4 flex-1">
          <EventsTab range={range} currency={baseCurrency} />
        </TabsContent>
      </Tabs>
    </div>
  );
}
