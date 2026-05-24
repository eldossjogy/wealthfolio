import { useCallback, useEffect, useMemo, useState } from "react";
import { Navigate, useNavigate, useSearchParams } from "react-router-dom";

import { useAccounts } from "@/hooks/use-accounts";
import { useTaxonomy } from "@/hooks/use-taxonomies";
import { useSettingsContext } from "@/lib/settings-provider";

import { Page, PageContent, PageHeader, usePersistentState } from "@wealthfolio/ui";

import { CategoryTransactionsSheet } from "../components/reports/category-transactions-sheet";
import { HeatmapCellSheet } from "../components/reports/heatmap-cell-sheet";
import { SpendingPeriodToggle } from "../components/spending-period-toggle";
import { StageNav, type InsightsStage } from "../components/reports/insights/stage-nav";
import { WhatChangedStage } from "../components/reports/insights/what-changed-stage";
import { WhenWhereStage } from "../components/reports/insights/when-where-stage";
import { WhereIAmStage } from "../components/reports/insights/where-i-am-stage";
import { useCashActivities } from "../hooks/use-cash-activities";
import { useMonthlyHistory } from "../hooks/use-monthly-history";
import { useEventSpendingSummaries } from "../hooks/use-spending-events";
import { useSpendingInsight } from "../hooks/use-spending-insight";
import { useSpendingReport } from "../hooks/use-spending-report";
import { useSpendingSettings } from "../hooks/use-spending-settings";
import { insightToLegacy, UNCATEGORIZED_CATEGORY_ID } from "../lib/insight-projection";
import {
  DEFAULT_REPORTS_PERIOD,
  REPORTS_PERIODS,
  periodToReportsRange,
  rangeToReportRequest,
  type ReportsPeriod,
  type ReportsRange,
} from "../lib/reports-period";

const SPENDING_TAXONOMY = "spending_categories";
const PERIOD_STORAGE_KEY = "spending-insights-period";
const STAGE_STORAGE_KEY = "spending-insights-stage";
const EMPTY_TAXONOMY: never[] = [];
/** Heatmap window — last 12 weeks regardless of selected period. */
const HEATMAP_WEEKS = 12;
const DAILY_GRANULARITY_THRESHOLD_DAYS = 35;
const HEATMAP_DAY_NAMES = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as const;

/**
 * Spending insights — narrative-first, three-stage page.
 *
 *   Where I am   — pace card + spent + cashflow + breakdown table
 *   What changed — period-vs-period headline + sparklines + delta table
 *   When & where — weekday-hour heatmap + events headline + per-event cards
 *
 * Owns period + comparison + stage state at the top; each stage receives the
 * data it needs. Data hooks run unconditionally so switching stages is instant.
 */
const VALID_STAGES: InsightsStage[] = ["where", "changed", "when"];

export default function SpendingInsightsPage() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";
  const { isEnabled, isLoading: settingsLoading } = useSpendingSettings();

  const [period, setPeriod] = usePersistentState<ReportsPeriod>(
    PERIOD_STORAGE_KEY,
    DEFAULT_REPORTS_PERIOD,
  );
  const [stage, setStage] = usePersistentState<InsightsStage>(STAGE_STORAGE_KEY, "where");

  // ─── URL ↔ state sync ─────────────────────────────────────────────────────
  // ?stage=where|changed|when and ?period=1M|3M|6M|YTD|1Y drive the page when
  // present, otherwise fall back to the persisted localStorage values.
  // Linking from the dashboard (`/spending/insights?stage=where`) and reload-
  // ability of the current view both rely on this.
  useEffect(() => {
    const urlStage = searchParams.get("stage");
    if (urlStage && VALID_STAGES.includes(urlStage as InsightsStage) && urlStage !== stage) {
      setStage(urlStage as InsightsStage);
    }
    const urlPeriod = searchParams.get("period") as ReportsPeriod | null;
    if (urlPeriod && REPORTS_PERIODS.includes(urlPeriod) && urlPeriod !== period) {
      setPeriod(urlPeriod);
    }
    // Eslint disable: we intentionally run only on URL change, not on every
    // state change — the inverse direction (state → URL) is handled by the
    // wrapped setters below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchParams]);

  const setStageAndUrl = useCallback(
    (next: InsightsStage) => {
      setStage(next);
      setSearchParams(
        (prev) => {
          const p = new URLSearchParams(prev);
          p.set("stage", next);
          return p;
        },
        { replace: true },
      );
    },
    [setStage, setSearchParams],
  );

  const setPeriodAndUrl = useCallback(
    (next: ReportsPeriod) => {
      setPeriod(next);
      setSearchParams(
        (prev) => {
          const p = new URLSearchParams(prev);
          p.set("period", next);
          return p;
        },
        { replace: true },
      );
    },
    [setPeriod, setSearchParams],
  );

  // Events-timeline pagination — independent from `period`. Stored alongside
  // the period it was set against so changing periods resets the offset to 0
  // without needing a useEffect.
  const [offsetBinding, setOffsetBinding] = useState<{
    period: ReportsPeriod;
    offset: number;
  }>({ period, offset: 0 });
  const eventsWindowOffset = offsetBinding.period === period ? offsetBinding.offset : 0;
  const setEventsWindowOffset = useCallback(
    (next: number) => setOffsetBinding({ period, offset: Math.max(0, next) }),
    [period],
  );

  const range = useMemo(() => periodToReportsRange(period), [period]);
  const eventsRange = useMemo(
    () => shiftRangeBack(range, period, eventsWindowOffset),
    [range, period, eventsWindowOffset],
  );
  const taxonomy = useTaxonomy(SPENDING_TAXONOMY);
  const { accounts = [] } = useAccounts({ filterActive: false });

  // ─── Single reconciled source of truth for the "Where I am" stage ─────────
  // One server call returns budgets + actuals + uncategorized + prior, all
  // computed against the same window — the math is reconciled by construction.
  const insightRequest = useMemo(
    () => ({
      startDate: range.start.toISOString(),
      endDate: range.end.toISOString(),
      compare: "prior" as const,
    }),
    [range],
  );
  const {
    data: insight,
    isLoading: isInsightLoading,
    isError: insightErrored,
    refetch: refetchInsight,
  } = useSpendingInsight(insightRequest);

  // Legacy hooks remain for the "What changed" + sparkline stages (they
  // produce richer per-day / per-leaf-category shapes that the insight
  // payload doesn't carry yet). Disabled when their stage isn't visible.
  const currentRequest = useMemo(() => rangeToReportRequest(range), [range]);
  const {
    data: currentReport,
    isLoading: isCurrentLoading,
    isError: currentErrored,
    refetch: refetchCurrent,
  } = useSpendingReport(currentRequest, stage === "changed");
  const {
    months,
    isLoading: isHistoryLoading,
    isError: historyErrored,
    refetch: refetchHistory,
  } = useMonthlyHistory(range, stage === "changed");

  // Project insight back into the legacy MonthlyReport + BudgetSnapshot
  // shapes that WhereIAmStage's child cards/table consume. Same numbers,
  // same field names — but every number now flows from one server query
  // so header / breakdown / Δ-vs-prior agree by construction.
  const legacyForWhereIAm = useMemo(() => (insight ? insightToLegacy(insight) : null), [insight]);
  const taxonomyCategoriesForWhereIAm = useMemo(() => {
    const base = taxonomy.data?.categories ?? [];
    if (!insight || insight.uncategorized.txnCount === 0) return base;
    // Synthetic top-level row so the breakdown table renders an
    // "Uncategorized" line. Matches the colors/shape of a regular category.
    const now = new Date().toISOString();
    return [
      ...base,
      {
        id: UNCATEGORIZED_CATEGORY_ID,
        taxonomyId: SPENDING_TAXONOMY,
        parentId: null,
        name: "Uncategorized",
        key: UNCATEGORIZED_CATEGORY_ID,
        color: "#9CA3AF",
        icon: null,
        description: null,
        sortOrder: 9999,
        createdAt: now,
        updatedAt: now,
      },
    ];
  }, [insight, taxonomy.data?.categories]);

  // 12-week activity window for the weekday × hour heatmap.
  const heatmapRequest = useMemo(() => {
    const end = new Date();
    end.setHours(23, 59, 59, 999);
    const start = new Date(end);
    start.setDate(end.getDate() - HEATMAP_WEEKS * 7);
    start.setHours(0, 0, 0, 0);
    return { startDate: start.toISOString(), endDate: end.toISOString() };
  }, []);
  const { data: heatmapActivities = [] } = useCashActivities(heatmapRequest);
  const { data: heatmapReport } = useSpendingReport(heatmapRequest, stage === "when");
  const heatmapDailySpendByDate = useMemo(
    () =>
      heatmapReport
        ? new Map(heatmapReport.byDay.map((day) => [day.date, day.outflow] as const))
        : undefined,
    [heatmapReport?.byDay],
  );

  const eventsRequest = useMemo(
    () => ({
      startDate: eventsRange.start.toISOString(),
      endDate: eventsRange.end.toISOString(),
    }),
    [eventsRange],
  );
  const {
    data: events = [],
    isError: eventsErrored,
    refetch: refetchEvents,
  } = useEventSpendingSummaries(eventsRequest);

  const onJumpToBreakdown = useCallback(() => {
    const el = document.getElementById("breakdown");
    if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
  }, []);

  // Click-through sheet for category transactions. The synthetic Uncategorized
  // row has no real category to filter activities by — clicks on it are
  // silently ignored for now (follow-up: route to a dedicated "uncategorized
  // transactions" filter so users can categorize them in-place).
  const [activeCategoryId, setActiveCategoryId] = useState<string | null>(null);
  const handleCategoryClick = useCallback((categoryId: string) => {
    if (categoryId === UNCATEGORIZED_CATEGORY_ID) return;
    setActiveCategoryId(categoryId);
  }, []);
  const activeCategory = useMemo(
    () =>
      activeCategoryId
        ? (taxonomy.data?.categories.find((c) => c.id === activeCategoryId) ?? null)
        : null,
    [activeCategoryId, taxonomy.data?.categories],
  );

  // Click-through sheet for heatmap cells (weekday × hour)
  const [heatmapCell, setHeatmapCell] = useState<{ weekday: number; hour: number } | null>(null);
  const handleHeatmapCellClick = useCallback((weekday: number, hour: number) => {
    setHeatmapCell({ weekday, hour });
  }, []);
  const heatmapCellActivities = useMemo(() => {
    if (!heatmapCell) return [];
    return heatmapActivities.filter((a) => {
      const d = new Date(a.activityDate);
      if (isNaN(d.getTime())) return false;
      const weekday = (d.getDay() + 6) % 7;
      return weekday === heatmapCell.weekday && d.getHours() === heatmapCell.hour;
    });
  }, [heatmapActivities, heatmapCell]);

  const useDailyForHistory = range.days <= DAILY_GRANULARITY_THRESHOLD_DAYS;
  const taxonomyCategories = taxonomy.data?.categories ?? EMPTY_TAXONOMY;
  const accountTypeById = useMemo(
    () => new Map(accounts.map((account) => [account.id, account.accountType])),
    [accounts],
  );

  // Gate the entire page when tracking is disabled — mirrors spending-budget-page.
  // Without this, disabling tracking from ModuleCard leaves Insights live and
  // continues rendering numbers the user explicitly opted out of seeing.
  if (!settingsLoading && !isEnabled) {
    return <Navigate to="/dashboard?tab=spending" replace />;
  }

  const periodToggle = <SpendingPeriodToggle value={period} onValueChange={setPeriodAndUrl} />;

  return (
    <Page>
      <PageHeader
        heading="Spending Insight"
        onBack={() => {
          if (window.history.length > 1) navigate(-1);
          else navigate("/dashboard?tab=spending");
        }}
        actions={periodToggle}
      />
      <PageContent className="space-y-5">
        <StageNav stage={stage} onStageChange={setStageAndUrl} />

        {insight?.foreignCurrencies && insight.foreignCurrencies.length > 0 && (
          <ForeignCurrencyBanner
            currency={insight.currency}
            foreign={insight.foreignCurrencies}
            nativeTotals={insight.nativeOutflowByCurrency ?? {}}
            asOf={insight.period.end}
          />
        )}

        {(insightErrored || (stage === "changed" && (currentErrored || historyErrored))) && (
          <div className="flex items-center justify-between gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-1.5 text-xs text-amber-700 dark:text-amber-300">
            <span>
              <span className="font-semibold">Couldn't load insights.</span> Showing zeros below.
            </span>
            <button
              type="button"
              onClick={() => {
                if (insightErrored) void refetchInsight();
                if (currentErrored) void refetchCurrent();
                if (historyErrored) refetchHistory();
              }}
              className="text-foreground hover:underline"
            >
              Retry
            </button>
          </div>
        )}

        {stage === "where" && (
          <WhereIAmStage
            range={range}
            currentReport={legacyForWhereIAm?.currentReport}
            priorReport={legacyForWhereIAm?.priorReport}
            months={legacyForWhereIAm?.months ?? []}
            taxonomyCategories={taxonomyCategoriesForWhereIAm}
            budget={legacyForWhereIAm?.budget}
            currency={insight?.currency ?? baseCurrency}
            isLoading={isInsightLoading}
            reconciledPace={insight?.headline.pace}
            onJumpToBreakdown={onJumpToBreakdown}
            onCategoryClick={handleCategoryClick}
          />
        )}

        {stage === "changed" && (
          <WhatChangedStage
            range={range}
            currentReport={legacyForWhereIAm?.currentReport}
            priorReport={legacyForWhereIAm?.priorReport}
            trendReport={currentReport}
            months={months}
            taxonomyCategories={taxonomyCategories}
            currency={insight?.currency ?? baseCurrency}
            isLoading={
              isInsightLoading || (useDailyForHistory ? isCurrentLoading : isHistoryLoading)
            }
            onCategoryClick={handleCategoryClick}
          />
        )}

        {stage === "when" && (
          <WhenWhereStage
            heatmapActivities={heatmapActivities}
            accountTypeById={accountTypeById}
            dailySpendByDate={heatmapDailySpendByDate}
            events={events}
            eventsErrored={eventsErrored}
            onRetryEvents={() => refetchEvents()}
            taxonomyCategories={taxonomyCategories}
            currency={baseCurrency}
            rangeStart={eventsRange.start}
            rangeEnd={eventsRange.end}
            windowOffset={eventsWindowOffset}
            onPrevWindow={() => setEventsWindowOffset(eventsWindowOffset + 1)}
            onNextWindow={() => setEventsWindowOffset(eventsWindowOffset - 1)}
            onHeatmapCellClick={handleHeatmapCellClick}
          />
        )}
      </PageContent>

      <CategoryTransactionsSheet
        open={!!activeCategory}
        onOpenChange={(open) => {
          if (!open) setActiveCategoryId(null);
        }}
        category={activeCategory}
        taxonomyCategories={taxonomyCategories}
        rangeStart={range.start}
        rangeEnd={range.end}
        currency={baseCurrency}
      />

      <HeatmapCellSheet
        open={!!heatmapCell}
        onOpenChange={(open) => {
          if (!open) setHeatmapCell(null);
        }}
        activities={heatmapCellActivities}
        dayLabel={heatmapCell ? HEATMAP_DAY_NAMES[heatmapCell.weekday] : null}
        hour={heatmapCell?.hour ?? null}
        currency={baseCurrency}
      />
    </Page>
  );
}

/** Shift the events window back by `offset` periods. Calendar-aligned for the
 *  month-based periods; YTD pages back by a full year so each click lands on
 *  the prior year's window. */
const MONTHS_PER_PERIOD: Record<ReportsPeriod, number> = {
  "1M": 1,
  "3M": 3,
  "6M": 6,
  YTD: 12,
  "1Y": 12,
};

function shiftRangeBack(range: ReportsRange, period: ReportsPeriod, offset: number): ReportsRange {
  if (offset === 0) return range;
  const months = MONTHS_PER_PERIOD[period] * offset;
  const start = new Date(range.start);
  start.setMonth(start.getMonth() - months);
  const end = new Date(range.end);
  end.setMonth(end.getMonth() - months);
  return { ...range, start, end };
}

/** Small notice rendered when activities in non-target currencies contributed.
 *  Single-foreign-currency reports get a "source: €1,200 EUR" hint (shows the
 *  pre-FX native total). Multi-currency reports get a list. Always names the
 *  FX as-of date so the user knows what rate snapshot was used. */
function ForeignCurrencyBanner({
  currency,
  foreign,
  nativeTotals,
  asOf,
}: {
  currency: string;
  foreign: string[];
  nativeTotals: Record<string, number>;
  asOf: string; // RFC3339
}) {
  const fmtNative = (ccy: string) => {
    const v = nativeTotals[ccy];
    if (v == null) return ccy;
    try {
      return new Intl.NumberFormat(undefined, {
        style: "currency",
        currency: ccy,
        maximumFractionDigits: 0,
      }).format(Math.abs(v));
    } catch {
      // Unknown ISO code → fall back to bare magnitude + code.
      return `${Math.abs(v).toFixed(0)} ${ccy}`;
    }
  };
  const asOfDate = new Date(asOf).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
  const detail =
    foreign.length === 1 ? (
      <>
        source: <span className="font-medium">{fmtNative(foreign[0])}</span>
      </>
    ) : (
      <>sources: {foreign.map((c) => fmtNative(c)).join(" + ")}</>
    );
  return (
    <div className="text-muted-foreground border-border/60 bg-muted/30 rounded-md border px-3 py-2 text-[11px]">
      <span className="text-foreground/90 font-medium">Multi-currency:</span> totals shown in{" "}
      {currency}, FX-converted from {foreign.join(", ")} using rates from {asOfDate}. {detail}.
    </div>
  );
}
