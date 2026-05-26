import { useNetWorth, useNetWorthHistory } from "@/hooks/use-alternative-assets";
import { useSettingsContext } from "@/lib/settings-provider";
import type { DateRange } from "@/lib/types";
import { formatDateISO } from "@/lib/utils";
import Balance from "@/pages/dashboard/balance";
import {
  GainAmount,
  GainPercent,
  IntervalSelector,
  getInitialIntervalData,
  usePersistentState,
  type TimePeriod,
} from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@wealthfolio/ui/components/ui/tooltip";
import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { BreakdownTable } from "./components/breakdown-table";
import { CompositionCard } from "./components/composition-card";
import { MomentumCard } from "./components/momentum-card";
import {
  CARD_GLASS,
  THEME_COLOR,
  THEME_COLOR_LIGHT,
  averageMonthlyChange,
  computeMomentum,
  computeVelocity,
  parseHistory,
  type ParsedNetWorth,
} from "./components/utils";
import { VelocityCard } from "./components/velocity-card";
import { NetWorthChart } from "./net-worth-chart";

const DEFAULT_INTERVAL: TimePeriod = "ALL";
const INTERVAL_STORAGE_KEY = "networth-interval";
const MS_PER_DAY = 86_400_000;

export function NetWorthContent() {
  const { settings } = useSettingsContext();
  const { data: netWorthData, isLoading, isError, error } = useNetWorth();

  const [intervalCode] = usePersistentState<TimePeriod>(INTERVAL_STORAGE_KEY, DEFAULT_INTERVAL);

  const [dateRange, setDateRange] = useState<DateRange | undefined>(
    () => getInitialIntervalData(intervalCode).range,
  );
  const [selectedIntervalDescription, setSelectedIntervalDescription] = useState<string>(
    () => getInitialIntervalData(intervalCode).description,
  );
  const [periodCode, setPeriodCode] = useState<TimePeriod>(intervalCode);

  // ISO date strings for the selected-range history query.
  const historyDates = useMemo(() => {
    if (!dateRange?.from) return null;
    const endDate = dateRange.to ?? new Date();
    return { startDate: formatDateISO(dateRange.from), endDate: formatDateISO(endDate) };
  }, [dateRange]);

  // Extended range covering an equal prior window (for Momentum) and the trailing
  // year (for the Velocity multiple), so both come from one extra query.
  const longHistoryDates = useMemo(() => {
    if (!dateRange?.from) return null;
    const end = dateRange.to ?? new Date();
    const rangeMs = end.getTime() - dateRange.from.getTime();
    const priorStart = new Date(dateRange.from.getTime() - rangeMs);
    const yearStart = new Date(end.getTime() - 366 * MS_PER_DAY);
    const start = priorStart < yearStart ? priorStart : yearStart;
    return { startDate: formatDateISO(start), endDate: formatDateISO(end) };
  }, [dateRange]);

  const { data: historyData, isLoading: isHistoryLoading } = useNetWorthHistory({
    startDate: historyDates?.startDate ?? "",
    endDate: historyDates?.endDate ?? "",
    enabled: !!historyDates,
  });

  const { data: longHistoryData } = useNetWorthHistory({
    startDate: longHistoryDates?.startDate ?? "",
    endDate: longHistoryDates?.endDate ?? "",
    enabled: !!longHistoryDates,
  });

  const handleIntervalSelect = (
    code: TimePeriod,
    description: string,
    range: DateRange | undefined,
  ) => {
    setSelectedIntervalDescription(description);
    setPeriodCode(code);
    setDateRange(range);
  };

  const parsedData = useMemo((): ParsedNetWorth | null => {
    if (!netWorthData) return null;
    return {
      netWorth: parseFloat(netWorthData.netWorth) || 0,
      assets: {
        total: parseFloat(netWorthData.assets.total) || 0,
        breakdown: (netWorthData.assets.breakdown || []).map((item) => ({
          category: item.category,
          name: item.name,
          value: parseFloat(item.value) || 0,
          assetId: item.assetId,
        })),
      },
      liabilities: {
        total: parseFloat(netWorthData.liabilities.total) || 0,
        breakdown: (netWorthData.liabilities.breakdown || []).map((item) => ({
          category: item.category,
          name: item.name,
          value: parseFloat(item.value) || 0,
          assetId: item.assetId,
        })),
      },
    };
  }, [netWorthData]);

  const parsedHistory = useMemo(() => parseHistory(historyData), [historyData]);
  const longHistory = useMemo(() => parseHistory(longHistoryData), [longHistoryData]);

  const velocity = useMemo(() => computeVelocity(parsedHistory), [parsedHistory]);
  const trailingYearMonthly = useMemo(() => {
    const cutoff = formatDateISO(new Date(Date.now() - 366 * MS_PER_DAY));
    return averageMonthlyChange(longHistory.filter((point) => point.date >= cutoff));
  }, [longHistory]);
  const momentum = useMemo(() => {
    if (!historyDates) return null;
    return computeMomentum(longHistory, historyDates.startDate, historyDates.endDate);
  }, [longHistory, historyDates]);

  // Net worth change over the selected range (simple delta).
  const { gainLossAmount, gainLossPercent } = useMemo(() => {
    if (parsedHistory.length < 2) return { gainLossAmount: 0, gainLossPercent: 0 };
    const first = parsedHistory[0].netWorth;
    const last = parsedHistory[parsedHistory.length - 1].netWorth;
    const change = last - first;
    const base = first !== 0 ? Math.abs(first) : 1;
    return { gainLossAmount: change, gainLossPercent: change / base };
  }, [parsedHistory]);

  const currency = netWorthData?.currency || settings?.baseCurrency || "USD";
  const hasStaleValuations = netWorthData && netWorthData.staleAssets.length > 0;
  const periodLabel = periodCode;

  if (isError && error) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center p-8">
        <div className="text-center">
          <div className="bg-destructive/10 mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full">
            <Icons.AlertTriangle className="text-destructive h-6 w-6" />
          </div>
          <p className="text-destructive text-lg font-medium">Failed to load net worth</p>
          <p className="text-muted-foreground mt-2 text-sm">{error?.message}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-full flex-col">
      {/* Top section: Net Worth value */}
      <div className="px-4 pb-1 pt-2 md:px-6 md:pb-2 lg:px-8">
        <div className="flex items-start gap-2">
          <div>
            <div className="flex items-center gap-3">
              <Balance
                isLoading={isLoading}
                targetValue={parsedData?.netWorth ?? 0}
                currency={currency}
                displayCurrency={true}
              />
              {hasStaleValuations && (
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div className="bg-warning/10 flex h-8 w-8 items-center justify-center rounded-full">
                        <Icons.AlertCircle className="text-warning h-4 w-4" />
                      </div>
                    </TooltipTrigger>
                    <TooltipContent side="bottom" className="max-w-[280px]">
                      <p className="mb-2 text-xs font-medium">Stale valuations (90+ days):</p>
                      <ul className="space-y-1 text-xs">
                        {netWorthData?.staleAssets.map((asset) => (
                          <li
                            key={asset.assetId}
                            className="flex items-center justify-between gap-2"
                          >
                            <span className="truncate">{asset.name ?? asset.assetId}</span>
                            <span className="text-muted-foreground shrink-0">
                              {asset.daysStale}d ago
                            </span>
                          </li>
                        ))}
                      </ul>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              )}
            </div>
            <div className="text-md flex space-x-3">
              {isHistoryLoading ? (
                <div className="flex items-center gap-3 pt-1">
                  <Skeleton className="h-4 w-24" />
                  <div className="border-secondary my-1 border-r pr-2" />
                  <Skeleton className="h-4 w-16" />
                </div>
              ) : (
                <>
                  <GainAmount
                    className="lg:text-md text-sm font-light"
                    value={gainLossAmount}
                    currency={currency}
                    displayCurrency={false}
                  />
                  <div className="border-secondary my-1 border-r pr-2" />
                  <GainPercent
                    className="lg:text-md text-sm font-light"
                    value={gainLossPercent}
                    animated={true}
                  />
                </>
              )}
              {selectedIntervalDescription && (
                <span className="lg:text-md text-muted-foreground ml-1 text-sm font-light">
                  {selectedIntervalDescription}
                </span>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Wrapper: chart + content with continuous gradient */}
      <div
        className="flex grow flex-col"
        style={{
          backgroundImage:
            (parsedData?.netWorth ?? 0) < 0
              ? `linear-gradient(to top, color-mix(in srgb, var(--destructive) 30%, transparent), color-mix(in srgb, var(--destructive) 15%, transparent) 50%, transparent 100%)`
              : `linear-gradient(to top, ${THEME_COLOR.replace(")", " / 0.30)")}, ${THEME_COLOR.replace(")", " / 0.15)")} 50%, transparent 100%)`,
        }}
      >
        {/* Chart section */}
        <div className="h-[280px]">
          {isHistoryLoading ? (
            <div className="flex h-full items-center justify-center">
              <Skeleton className="h-full w-full" />
            </div>
          ) : historyData && historyData.length > 0 ? (
            <NetWorthChart data={historyData} isLoading={isHistoryLoading} />
          ) : (
            <div className="flex h-full flex-col items-center justify-center">
              <Icons.TrendingUp className="text-muted-foreground/30 mb-3 h-12 w-12" />
              <p className="text-muted-foreground text-sm">No history data available</p>
            </div>
          )}
          {historyData && historyData.length > 0 && (
            <div className="flex w-full justify-center">
              <IntervalSelector
                className="pointer-events-auto relative z-20 w-full max-w-screen-sm sm:max-w-screen-md md:max-w-2xl lg:max-w-3xl"
                onIntervalSelect={handleIntervalSelect}
                isLoading={isHistoryLoading}
                storageKey={INTERVAL_STORAGE_KEY}
                defaultValue={DEFAULT_INTERVAL}
              />
            </div>
          )}
        </div>

        {/* Content section */}
        <div className="grow px-4 pb-[calc(var(--mobile-nav-ui-height)+max(var(--mobile-nav-gap),env(safe-area-inset-bottom)))] pt-20 md:px-6 md:pb-6 md:pt-20 lg:px-10 lg:pb-8 lg:pt-24">
          <div className="grid grid-cols-1 gap-8 lg:grid-cols-3 lg:gap-12">
            {/* Left column: Breakdown */}
            <div className="lg:col-span-2">
              {isLoading || isHistoryLoading ? (
                <div className={CARD_GLASS}>
                  <div className="space-y-4">
                    {Array.from({ length: 5 }).map((_, i) => (
                      <div key={i} className="flex items-center justify-between">
                        <Skeleton className="h-4 w-32" />
                        <Skeleton className="h-4 w-24" />
                      </div>
                    ))}
                  </div>
                </div>
              ) : parsedData ? (
                <BreakdownTable
                  data={parsedData}
                  history={parsedHistory}
                  currency={currency}
                  periodLabel={periodLabel}
                />
              ) : (
                <div
                  className="rounded-xl border border-orange-200/50 p-6 text-center md:p-8 dark:border-orange-800/50"
                  style={{ backgroundColor: THEME_COLOR_LIGHT }}
                >
                  <p className="text-sm">No assets found.</p>
                  <Link
                    to="/holdings"
                    className="text-muted-foreground hover:text-foreground mt-2 inline-flex items-center gap-1 text-xs underline-offset-4 hover:underline"
                  >
                    Add your first asset
                    <Icons.ChevronRight className="h-3 w-3" />
                  </Link>
                </div>
              )}
            </div>

            {/* Right column: insight cards */}
            <div className="space-y-6 lg:col-span-1">
              {parsedData && <CompositionCard data={parsedData} />}

              {velocity && (
                <VelocityCard
                  velocity={velocity}
                  trailingYearMonthly={trailingYearMonthly}
                  currency={currency}
                  periodLabel={periodLabel}
                />
              )}

              {momentum && (
                <MomentumCard momentum={momentum} currency={currency} periodLabel={periodLabel} />
              )}

              {/* Stale valuations warning */}
              {hasStaleValuations && (
                <div className="border-warning/30 bg-warning/5 rounded-xl border p-4 md:p-5">
                  <div className="flex items-start gap-3">
                    <div className="bg-warning/10 flex h-8 w-8 shrink-0 items-center justify-center rounded-full">
                      <Icons.AlertCircle className="text-warning h-4 w-4" />
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="text-sm font-medium">Update your valuations</p>
                      <p className="text-muted-foreground mt-1 text-xs">
                        {netWorthData?.staleAssets.length} asset
                        {netWorthData?.staleAssets.length !== 1 ? "s have" : " has"} not been
                        updated in over 90 days.
                      </p>
                      <div className="mt-3 space-y-1.5">
                        {netWorthData?.staleAssets.map((asset) => (
                          <Link
                            key={asset.assetId}
                            to={`/holdings/${encodeURIComponent(asset.assetId)}?tab=history`}
                            className="hover:bg-warning/10 flex items-center justify-between rounded-md px-2 py-1.5 transition-colors"
                          >
                            <span className="truncate text-xs font-medium">
                              {asset.name ?? asset.assetId}
                            </span>
                            <span className="text-muted-foreground ml-2 shrink-0 text-xs">
                              {asset.daysStale}d ago
                            </span>
                          </Link>
                        ))}
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default NetWorthContent;
