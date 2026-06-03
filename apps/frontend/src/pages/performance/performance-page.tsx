import { BenchmarkSymbolSelector } from "@/components/benchmark-symbol-selector";
import {
  ANNUALIZED_RETURN_INFO as annualizedReturnInfo,
  IRR_RETURN_INFO,
  MAX_DRAWDOWN_INFO as maxDrawdownInfo,
  MetricLabelWithInfo,
  PRICE_RETURN_INFO,
  SIMPLE_RETURN_INFO,
  TIME_WEIGHTED_RETURN_INFO,
  VALUE_RETURN_INFO,
  VOLATILITY_INFO as volatilityInfo,
} from "@/components/metric-display";
import { PerformanceChart } from "@/components/performance-chart";
import { PerformanceChartMobile } from "@/components/performance-chart-mobile";

import { PERFORMANCE_CHART_COLORS } from "@/components/performance-chart-colors";
import { EmptyPlaceholder } from "@wealthfolio/ui/components/ui/empty-placeholder";
import { useAccounts } from "@/hooks/use-accounts";
import { usePersistentState } from "@/hooks/use-persistent-state";
import { useIsMobileViewport } from "@/hooks/use-platform";
import { AccountPurpose, PORTFOLIO_SCOPE_ID } from "@/lib/constants";
import { performancePeriodPnl } from "@/lib/performance";
import { DateRange, PerformanceResult, TrackedItem } from "@/lib/types";
import { cn } from "@/lib/utils";
import {
  AlertFeedback,
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Carousel,
  CarouselContent,
  CarouselItem,
  DateRangeSelector,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  formatPercent,
  GainPercent,
  Icons,
  PrivacyAmount,
  Separator,
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@wealthfolio/ui";
import { isSameDay, subDays, subMonths } from "date-fns";
import { useEffect, useMemo, useState } from "react";
import { AccountSelector } from "../../components/account-selector";
import { AccountSelectorMobile } from "../../components/account-selector-mobile";
import { BenchmarkSymbolSelectorMobile } from "../../components/benchmark-symbol-selector-mobile";
import { useCalculatePerformanceHistory } from "./hooks/use-performance-data";
import {
  comparablePerformanceChartData,
  type ComparableChartDataItem as ChartDataItem,
  type PerformanceMetric,
} from "./performance-chart-series";
import {
  ALL_PORTFOLIO_ITEM,
  migratePerformanceSelectedItemId,
  migratePerformanceSelectedItems,
} from "./performance-selection";

function chartMetricForResult(result: PerformanceResult): PerformanceMetric {
  return result.mode === "valueReturn" ? "valueReturn" : "twr";
}

function trackingModeBadge(result: PerformanceResult): {
  label: string;
  variant: "outline" | "warning";
} | null {
  if (result.isMixedTrackingMode) {
    return { label: "Mixed mode", variant: "warning" };
  }
  if (result.isHoldingsMode || result.mode === "valueReturn") {
    return { label: "Holdings mode", variant: "outline" };
  }
  return null;
}

function chartExclusionReason(
  result: PerformanceResult | undefined,
  metric: PerformanceMetric,
): string {
  if (!result) return "Performance data is not available for this selection.";
  if (!result.series.length) return "No chart series is available for this period.";

  if (metric === "twr" && result.mode === "valueReturn") {
    return "Holdings-mode accounts use Value Return and are not plotted with TWR.";
  }
  if (metric === "valueReturn" && result.mode === "timeWeighted") {
    return "Transaction-mode accounts use TWR and are not plotted with Value Return.";
  }

  return "No overlapping chart dates with the selected item.";
}

function comparisonNoticeMessage(reasons: string[]): string {
  const hasDifferentReturnMethod = reasons.some(
    (reason) =>
      reason.includes("Value Return") || reason.includes("TWR") || reason.includes("price return"),
  );
  if (hasDifferentReturnMethod) {
    return "Different return methods cannot share the same chart. Select a muted chip to switch the chart mode.";
  }
  return "These selected items do not have enough overlapping chart dates with the active chart.";
}

function isVolatilityMethodologyNote(warning: string): boolean {
  return warning.toLowerCase().startsWith("volatility is annualized");
}

function metricPresentation(metric: PerformanceMetric): {
  label: string;
  mobileLabel: string;
  infoText: string;
} {
  switch (metric) {
    case "twr":
      return {
        label: "Time-Weighted Return",
        mobileLabel: "TWR",
        infoText: TIME_WEIGHTED_RETURN_INFO,
      };
    case "irr":
      return {
        label: "IRR",
        mobileLabel: "IRR",
        infoText: IRR_RETURN_INFO,
      };
    case "valueReturn":
      return {
        label: "Value Return",
        mobileLabel: "Value",
        infoText: VALUE_RETURN_INFO,
      };
    case "volatility":
      return {
        label: "Volatility",
        mobileLabel: "Vol",
        infoText: volatilityInfo,
      };
    case "drawdown":
      return {
        label: "Max Drawdown",
        mobileLabel: "Drawdown",
        infoText: maxDrawdownInfo,
      };
  }
}

function metricValue(result: PerformanceResult, metric: PerformanceMetric): number | null {
  switch (metric) {
    case "twr":
      return result.returns.twr == null ? null : Number(result.returns.twr);
    case "irr":
      return result.returns.irr == null ? null : Number(result.returns.irr);
    case "valueReturn":
      return result.returns.valueReturn == null ? null : Number(result.returns.valueReturn);
    case "volatility":
      return result.risk.volatility == null ? null : Number(result.risk.volatility);
    case "drawdown":
      return result.risk.maxDrawdown == null ? null : Number(result.risk.maxDrawdown);
  }
}

function annualizedMetricValue(
  result: PerformanceResult,
  metric: PerformanceMetric,
): number | null {
  switch (metric) {
    case "twr":
      return result.returns.annualizedTwr == null ? null : Number(result.returns.annualizedTwr);
    case "irr":
      return result.returns.annualizedIrr == null ? null : Number(result.returns.annualizedIrr);
    case "valueReturn":
      return result.returns.annualizedValueReturn == null
        ? null
        : Number(result.returns.annualizedValueReturn);
    case "volatility":
    case "drawdown":
      return null;
  }
}

function displayMetricValue(result: PerformanceResult, metric: PerformanceMetric): number | null {
  if (result.mode === "symbolPriceBased" && (metric === "twr" || metric === "valueReturn")) {
    return metricValue(result, "valueReturn");
  }
  return metricValue(result, metric);
}

function annualizedDisplayMetricValue(
  result: PerformanceResult,
  metric: PerformanceMetric,
): number | null {
  if (result.mode === "symbolPriceBased" && (metric === "twr" || metric === "valueReturn")) {
    return annualizedMetricValue(result, "valueReturn");
  }
  return annualizedMetricValue(result, metric);
}

function metricNotApplicableReason(
  result: PerformanceResult,
  metric: PerformanceMetric,
): string | undefined {
  const reasons = result.dataQuality.notApplicableReasons ?? [];
  const needle =
    metric === "twr"
      ? "TWR"
      : metric === "irr"
        ? "IRR"
        : metric === "valueReturn"
          ? "value return"
          : metric === "volatility"
            ? "Volatility"
            : "drawdown";
  return reasons.find((reason) => reason.toLowerCase().includes(needle.toLowerCase()));
}

function MetricValue({
  value,
  className,
  tone = "gain",
}: {
  value: number | null;
  className?: string;
  tone?: "gain" | "neutral";
}) {
  if (value == null) {
    return <span className={cn("text-muted-foreground font-medium", className)}>N/A</span>;
  }

  if (tone === "neutral") {
    return (
      <span className={cn("text-foreground font-medium", className)}>{formatPercent(value)}</span>
    );
  }

  return <GainPercent value={value} animated={true} className={className} />;
}

function HeaderMetric({
  label,
  infoText,
  warningText,
  value,
  tone = "gain",
  align = "center",
  valueClassName,
  reason,
}: {
  label: string;
  infoText: string;
  warningText?: string | string[];
  value: number | null;
  tone?: "gain" | "neutral";
  align?: "left" | "center" | "right";
  valueClassName?: string;
  reason?: string;
}) {
  return (
    <div
      className={cn(
        "flex min-w-0 flex-col gap-1",
        align === "left" && "items-start text-left",
        align === "center" && "items-center text-center",
        align === "right" && "items-end text-right",
      )}
    >
      <MetricLabelWithInfo
        label={label}
        infoText={infoText}
        warningText={warningText}
        className={cn(
          align === "left" && "justify-start",
          align === "center" && "justify-center",
          align === "right" && "justify-end",
        )}
      />
      <MetricValue
        value={value}
        tone={tone}
        className={cn("text-base font-semibold", valueClassName)}
      />
      {reason && (
        <span className="text-muted-foreground line-clamp-1 max-w-[12rem] text-[10px]">
          {reason}
        </span>
      )}
    </div>
  );
}

interface AttributionRow {
  label: string;
  value: number;
  description: string;
}

interface SelectedItemPlotState {
  isPlotted: boolean;
  reason?: string;
}

const PLOTTED_ITEM_STATE: SelectedItemPlotState = { isPlotted: true };
const PERFORMANCE_HIDDEN_DATE_RANGES = ["1D"] as const;

function isAllTimeDateRange(range: DateRange | undefined): boolean {
  return !!range?.from && isSameDay(range.from, new Date(1970, 0, 1));
}

function amountTone(value: number): string {
  if (value < 0) return "text-destructive";
  if (value > 0) return "text-success";
  return "text-muted-foreground";
}

function AttributionAmount({
  value,
  currency,
  className,
  tone = "semantic",
}: {
  value: number;
  currency: string;
  className?: string;
  tone?: "semantic" | "neutral";
}) {
  return (
    <PrivacyAmount
      value={value}
      currency={currency}
      className={cn(
        "whitespace-nowrap tabular-nums",
        tone === "semantic"
          ? amountTone(value)
          : value === 0
            ? "text-muted-foreground"
            : "text-foreground",
        className,
      )}
    />
  );
}

function AttributionRows({
  rows,
  currency,
  amountTone = "semantic",
}: {
  rows: AttributionRow[];
  currency: string;
  amountTone?: "semantic" | "neutral";
}) {
  return (
    <div className="border-border/60 divide-border/60 divide-y divide-dashed border-y">
      {rows.map((row) => (
        <div key={row.label} className="flex items-start justify-between gap-4 py-3.5">
          <div className="min-w-0">
            <div className="text-sm font-medium">{row.label}</div>
            <div className="text-muted-foreground mt-1 text-xs">{row.description}</div>
          </div>
          <AttributionAmount
            value={row.value}
            currency={currency}
            tone={amountTone}
            className="text-sm font-medium"
          />
        </div>
      ))}
    </div>
  );
}

function AttributionDetailMetric({
  result,
  itemName,
  dateRangeLabel,
  isMobile,
  className,
  valueClassName,
  align = "center",
}: {
  result: PerformanceResult | null;
  itemName?: string;
  dateRangeLabel: string;
  isMobile: boolean;
  className?: string;
  valueClassName?: string;
  align?: "left" | "center" | "right";
}) {
  const [isOpen, setIsOpen] = useState(false);
  if (!result || result.mode === "symbolPriceBased") return null;
  const periodPnl = performancePeriodPnl(result);
  if (periodPnl == null) return null;

  const currency = result.scope.currency;
  const driverRows: AttributionRow[] = [
    {
      label: "Unrealized P&L",
      value: Number(result.attribution.unrealizedPnlChange),
      description: "Profit and loss from open positions",
    },
    {
      label: "Realized P&L",
      value: Number(result.attribution.realizedPnl),
      description: "Profit and loss from closed positions",
    },
    {
      label: "Income",
      value: Number(result.attribution.income),
      description: "Dividends and interest received",
    },
    {
      label: "FX effect",
      value: Number(result.attribution.fxEffect),
      description: "Gain or loss from currency conversion",
    },
    {
      label: "Fees",
      value: -Number(result.attribution.fees),
      description: "Trading commissions and account fees",
    },
    {
      label: "Taxes",
      value: -Number(result.attribution.taxes),
      description: "Tax withholdings on income",
    },
    {
      label: "Residual",
      value: Number(result.attribution.residual),
      description: "Unallocated rounding and adjustments",
    },
  ];
  const flowRows: AttributionRow[] = [
    {
      label: "Contributions",
      value: Number(result.attribution.contributions),
      description: "Cash you added to the account",
    },
    {
      label: "Distributions",
      value: -Number(result.attribution.distributions),
      description: "Cash withdrawn from the account",
    },
  ];

  return (
    <Sheet open={isOpen} onOpenChange={setIsOpen}>
      <Button
        type="button"
        variant="ghost"
        className={cn(
          "hover:bg-muted/50 group h-auto w-full min-w-0 rounded-md px-2 py-1",
          align === "left" && "text-left",
          align === "center" && "text-center",
          align === "right" && "text-right",
          className,
        )}
        onClick={() => setIsOpen(true)}
      >
        <div className="w-full min-w-0 space-y-1">
          <div
            className={cn(
              "text-muted-foreground flex items-center gap-1 text-xs font-light",
              align === "left" && "justify-start",
              align === "center" && "justify-center",
              align === "right" && "justify-end",
            )}
          >
            <span>Period Gain/Loss</span>
            <Icons.Info className="h-3 w-3" />
          </div>
          <div
            className={cn(
              "flex items-center gap-1",
              align === "left" && "justify-start",
              align === "center" && "justify-center",
              align === "right" && "justify-end",
            )}
          >
            <AttributionAmount
              value={periodPnl}
              currency={currency}
              className={cn("text-base font-semibold", valueClassName)}
            />
            <Icons.ChevronRight className="text-muted-foreground/60 group-hover:text-muted-foreground h-3.5 w-3.5 shrink-0 transition-all group-hover:translate-x-0.5" />
          </div>
        </div>
      </Button>

      <SheetContent
        side={isMobile ? "bottom" : "right"}
        className={cn(
          "flex w-full flex-col p-0",
          isMobile ? "h-[85vh] rounded-t-3xl" : "h-full sm:max-w-md",
        )}
        showCloseButton={false}
      >
        <SheetHeader className="border-border border-b px-6 py-5">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0 space-y-2">
              <SheetTitle className="text-xl">Attribution</SheetTitle>
              <SheetDescription className="truncate">
                {[itemName, dateRangeLabel].filter(Boolean).join(" · ")}
              </SheetDescription>
            </div>
            <SheetClose asChild>
              <Button
                variant="ghost"
                size="icon"
                className="text-muted-foreground h-8 w-8 shrink-0"
              >
                <Icons.X className="h-4 w-4" />
                <span className="sr-only">Close attribution</span>
              </Button>
            </SheetClose>
          </div>
          <div className="pt-3">
            <div className="text-muted-foreground text-xs font-medium uppercase tracking-wide">
              Period gain/loss
            </div>
            <AttributionAmount
              value={periodPnl}
              currency={currency}
              className="mt-2 block text-xl font-semibold leading-tight"
            />
          </div>
        </SheetHeader>
        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-2">
          <div className="py-3.5">
            <div className="text-muted-foreground mb-3 text-xs font-medium uppercase tracking-wide">
              Performance drivers
            </div>
            <AttributionRows rows={driverRows} currency={currency} />
          </div>

          <div className="py-3.5">
            <div className="text-muted-foreground mb-3 text-xs font-medium uppercase tracking-wide">
              Cash flows
            </div>
            <AttributionRows rows={flowRows} currency={currency} amountTone="neutral" />
          </div>
        </div>
        <div className="border-border bg-background flex items-center justify-between gap-4 border-t px-6 py-4">
          <div className="text-xs font-medium uppercase tracking-wide">Total gain/loss</div>
          <AttributionAmount
            value={periodPnl}
            currency={currency}
            className="text-base font-semibold"
          />
        </div>
      </SheetContent>
    </Sheet>
  );
}

function PerformanceContent({
  chartData,
  isLoading,
  hasErrors,
  errorMessages,
  isMobile,
}: {
  chartData: ChartDataItem[] | undefined;
  isLoading: boolean;
  hasErrors: boolean;
  errorMessages: string[];
  isMobile: boolean;
}) {
  return (
    <div className="relative flex h-full w-full flex-col">
      {chartData && chartData.length > 0 && (
        <div className="min-h-0 w-full flex-1">
          {isMobile ? (
            <PerformanceChartMobile data={chartData} />
          ) : (
            <PerformanceChart data={chartData} />
          )}
        </div>
      )}

      {!chartData?.length && !isLoading && !hasErrors && (
        <EmptyPlaceholder
          className="mx-auto flex max-w-[420px] items-center justify-center"
          icon={<Icons.BarChart className="h-10 w-10" />}
          title="No performance data"
          description="Select accounts to compare their performance over time."
        />
      )}

      {/* Modern horizontal loader with improved UX */}
      {isLoading && (
        <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
          <div className="animate-subtle-pulse absolute inset-0 border-2 border-transparent">
            <div className="animate-progress-border bg-primary absolute left-0 top-0 h-[2px]"></div>
          </div>
          <div className="absolute bottom-4 right-4">
            <div className="bg-background/80 rounded-md border px-3 py-1.5 shadow-sm backdrop-blur-sm">
              <p className="text-muted-foreground flex items-center text-xs font-medium">
                <span className="bg-primary mr-2 inline-block h-2 w-2 animate-pulse rounded-full"></span>
                Calculating...
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Error display using AlertFeedback component */}
      {hasErrors && (
        <div className="w-full">
          <AlertFeedback title="Error calculating performance data" variant="error">
            <div>
              {errorMessages.map((error, index) => (
                <p key={index} className="text-sm">
                  {error}
                </p>
              ))}
            </div>
            <div className="mt-4 flex justify-end">
              <Button size="sm" onClick={() => window.location.reload()} variant="default">
                Retry
              </Button>
            </div>
          </AlertFeedback>
        </div>
      )}
    </div>
  );
}

const SelectedItemBadge = ({
  item,
  isSelected,
  isPlotted,
  plotReason,
  contextLabel,
  onSelect,
  onDelete,
  color,
}: {
  item: TrackedItem;
  isSelected: boolean;
  isPlotted: boolean;
  plotReason?: string;
  contextLabel?: string;
  onSelect: () => void;
  onDelete: (e: React.MouseEvent) => void;
  color?: string;
}) => {
  return (
    <Badge
      className={cn(
        "text-foreground group relative cursor-pointer rounded-md border px-2.5 py-1.5 shadow-sm transition-all sm:px-3",
        "hover:bg-accent/80 hover:shadow-md",
        "focus-visible:ring-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2",
        !isPlotted &&
          "text-muted-foreground bg-muted/30 hover:bg-muted/50 border-dashed shadow-none hover:shadow-sm",
        isSelected && "bg-warning/20 hover:bg-warning/30",
      )}
      onClick={onSelect}
      role="button"
      variant="secondary"
      tabIndex={0}
      title={plotReason}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect();
        }
      }}
      aria-pressed={isSelected}
    >
      <div className="flex items-center space-x-2 sm:space-x-3">
        <div
          className={cn(
            "h-3 w-1 rounded-full sm:h-4",
            !isPlotted
              ? "bg-muted-foreground/35"
              : color
                ? "transition-opacity group-hover:opacity-80"
                : item.type === "account"
                  ? "bg-muted-foreground group-hover:bg-foreground transition-colors"
                  : "bg-orange-500 transition-colors group-hover:bg-orange-600 dark:bg-orange-400",
          )}
          style={isPlotted && color ? { backgroundColor: color } : undefined}
        />
        <span className="group-hover:text-foreground max-w-40 truncate text-xs font-medium transition-colors sm:text-sm">
          {item.name}
        </span>
        {!isPlotted && (
          <span className="bg-background/70 text-muted-foreground rounded px-1.5 py-0.5 text-[10px] font-medium">
            Not plotted
          </span>
        )}
        {isPlotted && contextLabel && (
          <span className="bg-background/70 text-muted-foreground rounded px-1.5 py-0.5 text-[10px] font-medium">
            {contextLabel}
          </span>
        )}
      </div>
      <Button
        variant="ghost"
        size="icon-xs"
        className={cn(
          "ml-2 size-5 transition-all duration-150",
          "hover:bg-destructive/10 hover:text-destructive hover:scale-110",
          "focus-visible:ring-destructive/50 focus-visible:ring-2",
        )}
        onClick={onDelete}
        aria-label={`Remove ${item.name}`}
      >
        <Icons.Close className="h-3 w-3 sm:h-3.5 sm:w-3.5" />
      </Button>
    </Badge>
  );
};

export default function PerformancePage() {
  const isMobile = useIsMobileViewport();
  const [storedSelectedItems, setSelectedItems] = usePersistentState<TrackedItem[]>(
    "performance:selectedItems",
    [ALL_PORTFOLIO_ITEM],
  );
  const [storedSelectedItemId, setSelectedItemId] = usePersistentState<string | null>(
    "performance:selectedItemId",
    null,
  );
  const [dateRange, setDateRange] = usePersistentState<DateRange | undefined>(
    "performance:dateRange",
    {
      from: subMonths(new Date(), 12),
      to: new Date(),
    },
  );

  useEffect(() => {
    if (!dateRange?.from || !dateRange?.to) return;
    const today = new Date();
    if (isSameDay(dateRange.from, subDays(today, 1)) && isSameDay(dateRange.to, today)) {
      setDateRange({ from: subDays(today, 7), to: today });
    }
  }, [dateRange, setDateRange]);
  const { accounts, isLoading: isAccountsLoading } = useAccounts({
    accountPurpose: AccountPurpose.PERFORMANCE,
  });

  // State for mobile dropdown menu
  const [accountSheetOpen, setAccountSheetOpen] = useState(false);
  const [benchmarkSheetOpen, setBenchmarkSheetOpen] = useState(false);
  const selectedItems = useMemo(
    () => migratePerformanceSelectedItems(storedSelectedItems),
    [storedSelectedItems],
  );
  const selectedItemId = migratePerformanceSelectedItemId(storedSelectedItemId);

  useEffect(() => {
    if (selectedItems !== storedSelectedItems) {
      setSelectedItems(selectedItems);
    }
  }, [selectedItems, setSelectedItems, storedSelectedItems]);

  useEffect(() => {
    if (selectedItemId !== storedSelectedItemId) {
      setSelectedItemId(selectedItemId);
    }
  }, [selectedItemId, setSelectedItemId, storedSelectedItemId]);

  useEffect(() => {
    if (isAccountsLoading) {
      return;
    }
    const reportAccountIds = new Set(accounts.map((account) => account.id));
    // User-created portfolios resolve to account ids at calc time, so we keep
    // them regardless of `reportAccountIds`; the backend filter handles it.
    const isPortfolioItem = (item: TrackedItem) => item.accountScope?.type === "portfolio";
    setSelectedItems((current) => {
      const next = current.filter(
        (item) =>
          item.type !== "account" ||
          item.id === PORTFOLIO_SCOPE_ID ||
          isPortfolioItem(item) ||
          reportAccountIds.has(item.id),
      );
      if (next.length === current.length) {
        return current;
      }
      return next.length > 0 ? next : [ALL_PORTFOLIO_ITEM];
    });
    const selectedItemStillPresent =
      !selectedItemId ||
      selectedItems.some(
        (item) =>
          item.id === selectedItemId &&
          (item.type !== "account" ||
            item.id === PORTFOLIO_SCOPE_ID ||
            isPortfolioItem(item) ||
            reportAccountIds.has(item.id)),
      );
    if (!selectedItemStillPresent) {
      setSelectedItemId(null);
    }
  }, [
    accounts,
    isAccountsLoading,
    selectedItemId,
    selectedItems,
    setSelectedItemId,
    setSelectedItems,
  ]);

  // Helper function to sort comparison items (accounts first, then symbols)
  const sortComparisonItems = (items: TrackedItem[]): TrackedItem[] => {
    return [...items].sort((a, b) => {
      // Sort by type first (accounts before symbols)
      if (a.type !== b.type) {
        return a.type === "account" ? -1 : 1;
      }
      // If same type, maintain original order
      return 0;
    });
  };

  // Use the custom hook for parallel data fetching with effective date calculation
  const {
    data: performanceData,
    isLoading: isLoadingPerformance,
    hasErrors,
    errorMessages,
    displayDateRange,
  } = useCalculatePerformanceHistory({
    selectedItems,
    dateRange: isAllTimeDateRange(dateRange) ? undefined : dateRange,
  });

  const selectedPerformanceData = useMemo(() => {
    if (!performanceData?.length || !selectedItems) return null;
    const targetId = selectedItemId ?? performanceData.find((item) => item !== null)?.id; // Find first non-null item ID if none selected
    if (!targetId) return null;
    const found = performanceData.find((item) => item?.id === targetId);
    if (!found) return null;
    const name = selectedItems.find((item) => item.id === found.id)?.name ?? "Unknown";
    return {
      result: found,
      name,
      chartMetric: chartMetricForResult(found),
    };
  }, [selectedItemId, performanceData, selectedItems]);

  const selectedChartMetric = selectedPerformanceData?.chartMetric ?? "twr";
  const activeChartAnchorId = selectedPerformanceData?.result.id ?? selectedItemId;

  // Calculate derived chart data
  const chartData = useMemo(() => {
    return comparablePerformanceChartData(
      performanceData,
      selectedChartMetric,
      activeChartAnchorId ?? null,
    );
  }, [activeChartAnchorId, performanceData, selectedChartMetric]);

  const chartColorMap = useMemo(() => {
    const map = new Map<string, string>();
    chartData.forEach((series, index) => {
      map.set(series.id, PERFORMANCE_CHART_COLORS[index % PERFORMANCE_CHART_COLORS.length]);
    });
    return map;
  }, [chartData]);

  const itemPlotStateById = useMemo<Map<string, SelectedItemPlotState>>(() => {
    const chartedIds = new Set(chartData.map((series) => series.id));
    const resultById = new Map<string, PerformanceResult>();
    for (const item of performanceData ?? []) {
      if (item) {
        resultById.set(item.id, item);
      }
    }

    return new Map(
      selectedItems.map((item) => {
        const isAnchor = item.id === activeChartAnchorId;
        const isPlotted = isLoadingPerformance || hasErrors || isAnchor || chartedIds.has(item.id);
        const plotState: SelectedItemPlotState = {
          isPlotted,
          reason: isPlotted
            ? undefined
            : chartExclusionReason(resultById.get(item.id), selectedChartMetric),
        };
        return [item.id, plotState];
      }),
    );
  }, [
    activeChartAnchorId,
    chartData,
    hasErrors,
    isLoadingPerformance,
    performanceData,
    selectedChartMetric,
    selectedItems,
  ]);

  const notPlottedItems = useMemo(
    () => selectedItems.filter((item) => itemPlotStateById.get(item.id)?.reason),
    [itemPlotStateById, selectedItems],
  );
  const comparisonNotice = useMemo(() => {
    if (!notPlottedItems.length) return null;
    const reasons = notPlottedItems
      .map((item) => itemPlotStateById.get(item.id)?.reason)
      .filter((reason): reason is string => Boolean(reason));
    return {
      count: notPlottedItems.length,
      message: comparisonNoticeMessage(reasons),
    };
  }, [itemPlotStateById, notPlottedItems]);

  // Calculate selected item data
  const selectedItemData = useMemo(() => {
    if (!selectedPerformanceData) return null;
    const found = selectedPerformanceData.result;
    const selectedMetric = selectedPerformanceData.chartMetric;
    const selectedMetricPresentation =
      found.mode === "symbolPriceBased" &&
      (selectedMetric === "twr" || selectedMetric === "valueReturn")
        ? {
            label: "Price Return",
            mobileLabel: "Price",
            infoText: PRICE_RETURN_INFO,
          }
        : metricPresentation(selectedMetric);
    const selectedMetricValue = displayMetricValue(found, selectedMetric);
    const warnings = found.dataQuality.warnings ?? [];
    const visibleWarnings = warnings.filter((warning) => !isVolatilityMethodologyNote(warning));
    return {
      id: found.id,
      name: selectedPerformanceData.name,
      result: found,
      chartMetric: selectedMetric,
      selectedMetricValue,
      selectedMetricReason:
        selectedMetricValue == null ? metricNotApplicableReason(found, selectedMetric) : undefined,
      annualizedReturn: annualizedDisplayMetricValue(found, selectedMetric),
      volatility: metricValue(found, "volatility"),
      maxDrawdown: metricValue(found, "drawdown"),
      periodPnl: found.mode === "symbolPriceBased" ? null : performancePeriodPnl(found),
      ...selectedMetricPresentation,
      trackingModeBadge: trackingModeBadge(found),
      returnWarnings: visibleWarnings,
      volatilityWarnings: [],
      warnings: visibleWarnings,
      notApplicableReasons: found.dataQuality.notApplicableReasons ?? [],
    };
  }, [selectedPerformanceData]);

  const preserveCurrentChartAnchor = (fallbackId: string) => {
    setSelectedItemId(
      selectedItemId ?? selectedPerformanceData?.result.id ?? selectedItems[0]?.id ?? fallbackId,
    );
  };

  const handleAccountSelect = (account: { id: string; name: string }) => {
    const accountId = String(account.id);
    const exists = selectedItems.some((item) => item.id === accountId);

    if (exists) {
      const nextItems = sortComparisonItems(selectedItems.filter((item) => item.id !== accountId));
      setSelectedItems(nextItems);
      if (selectedItemId === accountId) {
        setSelectedItemId(null);
      }
      return;
    }

    const newItem: TrackedItem = {
      id: accountId,
      type: "account",
      name: account.name,
      accountScope:
        accountId === PORTFOLIO_SCOPE_ID ? { type: "all" } : { type: "account", accountId },
    };

    setSelectedItems(sortComparisonItems([...selectedItems, newItem]));
    preserveCurrentChartAnchor(accountId);
  };

  const handlePortfolioSelect = (portfolio: { id: string; name: string }) => {
    const portfolioId = String(portfolio.id);
    const exists = selectedItems.some((item) => item.id === portfolioId);

    if (exists) {
      const nextItems = sortComparisonItems(
        selectedItems.filter((item) => item.id !== portfolioId),
      );
      setSelectedItems(nextItems);
      if (selectedItemId === portfolioId) {
        setSelectedItemId(null);
      }
      return;
    }

    // Tracked as an "account" so it lands in the chart's account-series path,
    // but the scope filter expands it to the portfolio's member account ids
    // at calc time (handled by the Rust `calculate_performance_history` cmd).
    const newItem: TrackedItem = {
      id: portfolioId,
      type: "account",
      name: portfolio.name,
      accountScope: { type: "portfolio", portfolioId },
    };

    setSelectedItems(sortComparisonItems([...selectedItems, newItem]));
    preserveCurrentChartAnchor(portfolioId);
  };

  const handleSymbolSelect = (symbol: { id: string; name: string }) => {
    const symbolId = String(symbol.id);
    const exists = selectedItems.some((item) => item.id === symbolId);
    if (exists) return;

    const newSymbol: TrackedItem = {
      id: symbolId,
      type: "symbol",
      name: symbol.name,
    };

    setSelectedItems(sortComparisonItems([...selectedItems, newSymbol]));
    preserveCurrentChartAnchor(symbolId);
  };

  const handleBadgeSelect = (item: TrackedItem) => {
    setSelectedItemId(item.id);
  };

  const handleBadgeDelete = (e: React.MouseEvent, item: TrackedItem) => {
    e.stopPropagation();
    if (item.type === "account") {
      handleAccountSelect({ id: item.id, name: item.name });
    } else {
      setSelectedItems((prev) => sortComparisonItems(prev.filter((i) => i.id !== item.id)));
    }
    if (selectedItemId === item.id) {
      setSelectedItemId(null);
    }
  };

  return (
    <>
      {/* Date range selector - fixed position in header area */}
      <div className="pointer-events-auto fixed right-2 top-4 z-20 hidden md:block lg:right-4">
        <DateRangeSelector
          value={dateRange}
          onChange={setDateRange}
          hiddenRanges={PERFORMANCE_HIDDEN_DATE_RANGES}
        />
      </div>

      <div className="flex h-full flex-col space-y-4">
        <div className="flex justify-end md:hidden">
          <DateRangeSelector
            value={dateRange}
            onChange={setDateRange}
            hiddenRanges={PERFORMANCE_HIDDEN_DATE_RANGES}
          />
        </div>

        {/* Mobile: Carousel + Plus button in same row */}
        <div className="flex items-center gap-2 md:hidden">
          {/* Selected items badges carousel */}
          {selectedItems.length > 0 && (
            <Carousel
              opts={{
                align: "start",
                loop: false,
              }}
              className="flex-1"
            >
              <CarouselContent className="-ml-2">
                {selectedItems.map((item) => {
                  const plotState = itemPlotStateById.get(item.id) ?? PLOTTED_ITEM_STATE;
                  return (
                    <CarouselItem key={item.id} className="basis-auto pl-2">
                      <SelectedItemBadge
                        item={item}
                        isSelected={activeChartAnchorId === item.id}
                        isPlotted={plotState.isPlotted}
                        plotReason={plotState.reason}
                        contextLabel={
                          selectedChartMetric === "valueReturn" && item.type === "symbol"
                            ? "Reference"
                            : undefined
                        }
                        onSelect={() => handleBadgeSelect(item)}
                        onDelete={(e) => handleBadgeDelete(e, item)}
                        color={plotState.isPlotted ? chartColorMap.get(item.id) : undefined}
                      />
                    </CarouselItem>
                  );
                })}
              </CarouselContent>
            </Carousel>
          )}

          {/* Mobile: Plus button with dropdown */}
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="outline"
                size="icon"
                className="bg-secondary/30 hover:bg-muted/80 size-9 flex-shrink-0 rounded-md border-[1.5px] border-none"
                aria-label="Add item"
              >
                <Icons.Plus className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-48">
              <DropdownMenuItem onSelect={() => setAccountSheetOpen(true)} className="py-4 md:py-2">
                <Icons.Briefcase className="mr-2 h-4 w-4" />
                Add Account
              </DropdownMenuItem>
              <DropdownMenuItem
                onSelect={() => setBenchmarkSheetOpen(true)}
                className="py-4 md:py-2"
              >
                <Icons.TrendingUp className="mr-2 h-4 w-4" />
                Add Benchmark
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        {/* Desktop: Full layout with separator */}
        <div className="hidden md:flex md:flex-row md:items-center">
          {/* Selected items badges - horizontal scroll carousel */}
          {selectedItems.length > 0 && (
            <div className="flex items-center gap-3">
              <Carousel
                opts={{
                  align: "start",
                  loop: false,
                }}
                className="w-full max-w-[calc(100vw-24rem)] md:max-w-[calc(100vw-28rem)]"
              >
                <CarouselContent className="-ml-2">
                  {selectedItems.map((item) => {
                    const plotState = itemPlotStateById.get(item.id) ?? PLOTTED_ITEM_STATE;
                    return (
                      <CarouselItem key={item.id} className="basis-auto pl-2">
                        <SelectedItemBadge
                          item={item}
                          isSelected={activeChartAnchorId === item.id}
                          isPlotted={plotState.isPlotted}
                          plotReason={plotState.reason}
                          contextLabel={
                            selectedChartMetric === "valueReturn" && item.type === "symbol"
                              ? "Reference"
                              : undefined
                          }
                          onSelect={() => handleBadgeSelect(item)}
                          onDelete={(e) => handleBadgeDelete(e, item)}
                          color={plotState.isPlotted ? chartColorMap.get(item.id) : undefined}
                        />
                      </CarouselItem>
                    );
                  })}
                </CarouselContent>
              </Carousel>

              {/* Separator */}
              <Separator orientation="vertical" className="h-6 flex-shrink-0" />
            </div>
          )}

          {/* Desktop: Full text buttons */}
          <div className="flex flex-shrink-0 items-center gap-2">
            <AccountSelector
              setSelectedAccount={handleAccountSelect}
              variant="button"
              buttonText="Add account"
              includePortfolio={true}
              accountPurpose={AccountPurpose.PERFORMANCE}
              onPortfolioSelect={handlePortfolioSelect}
            />
            <BenchmarkSymbolSelector onSelect={handleSymbolSelect} />
          </div>
        </div>

        {/* Mobile sheets controlled by dropdown - rendered but hidden by Sheet component */}
        <AccountSelectorMobile
          setSelectedAccount={(account) => {
            handleAccountSelect(account);
            setAccountSheetOpen(false);
          }}
          includePortfolio={true}
          accountPurpose={AccountPurpose.PERFORMANCE}
          open={accountSheetOpen}
          onOpenChange={setAccountSheetOpen}
          className="hidden"
          onPortfolioSelect={(portfolio) => {
            handlePortfolioSelect(portfolio);
            setAccountSheetOpen(false);
          }}
        />
        <BenchmarkSymbolSelectorMobile
          onSelect={(symbol) => {
            handleSymbolSelect(symbol);
            setBenchmarkSheetOpen(false);
          }}
          open={benchmarkSheetOpen}
          onOpenChange={setBenchmarkSheetOpen}
          className="hidden"
        />

        <div className="flex h-[calc(100vh-19rem)] flex-col md:h-[calc(100vh-12rem)]">
          <Card className="flex min-h-0 flex-1 flex-col">
            <CardHeader className={cn("pb-2", isMobile ? "px-3 py-3" : "px-6 pb-2 pt-5")}>
              <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
                <div className="min-w-0">
                  <CardTitle className={cn("text-lg sm:text-xl", isMobile && "text-sm")}>
                    Performance
                  </CardTitle>
                  <CardDescription className={cn("text-xs sm:text-sm", isMobile && "text-[10px]")}>
                    {displayDateRange}
                  </CardDescription>
                  {selectedItemData?.trackingModeBadge && (
                    <Badge
                      variant={selectedItemData.trackingModeBadge.variant}
                      className="mt-2 h-5 rounded-md px-1.5 text-[10px] font-medium"
                    >
                      {selectedItemData.trackingModeBadge.label}
                    </Badge>
                  )}
                </div>

                {performanceData && performanceData.length > 0 && (
                  <>
                    {isMobile ? (
                      <Carousel
                        opts={{
                          align: "start",
                          loop: false,
                        }}
                        className="w-full"
                      >
                        <CarouselContent className="-ml-2">
                          <CarouselItem className="basis-[42%] pl-2">
                            <div className="bg-muted/30 rounded-lg px-3 py-2">
                              <HeaderMetric
                                label={selectedItemData?.mobileLabel ?? "Return"}
                                infoText={selectedItemData?.infoText ?? SIMPLE_RETURN_INFO}
                                warningText={selectedItemData?.returnWarnings}
                                value={selectedItemData?.selectedMetricValue ?? null}
                                reason={selectedItemData?.selectedMetricReason}
                                align="left"
                                valueClassName="text-base"
                              />
                            </div>
                          </CarouselItem>
                          <CarouselItem className="basis-[42%] pl-2">
                            <div className="bg-muted/30 rounded-lg px-3 py-2">
                              <HeaderMetric
                                label="Annualized"
                                infoText={annualizedReturnInfo}
                                value={selectedItemData?.annualizedReturn ?? null}
                                align="left"
                                valueClassName="text-base"
                              />
                            </div>
                          </CarouselItem>
                          <CarouselItem className="basis-[42%] pl-2">
                            <div className="bg-muted/30 rounded-lg px-3 py-2">
                              <HeaderMetric
                                label="Volatility"
                                infoText={volatilityInfo}
                                warningText={selectedItemData?.volatilityWarnings}
                                value={selectedItemData?.volatility ?? null}
                                tone="neutral"
                                align="left"
                                valueClassName="text-base"
                              />
                            </div>
                          </CarouselItem>
                          <CarouselItem className="basis-[42%] pl-2">
                            <div className="bg-muted/30 rounded-lg px-3 py-2">
                              <HeaderMetric
                                label="Max Drawdown"
                                infoText={maxDrawdownInfo}
                                value={selectedItemData?.maxDrawdown ?? null}
                                align="left"
                                valueClassName="text-base"
                              />
                            </div>
                          </CarouselItem>
                          {selectedItemData?.periodPnl != null && (
                            <CarouselItem className="basis-[52%] pl-2">
                              <AttributionDetailMetric
                                result={selectedItemData.result}
                                itemName={selectedItemData.name}
                                dateRangeLabel={displayDateRange}
                                isMobile={isMobile}
                                align="left"
                                className="bg-muted/30 hover:bg-muted/50 h-auto rounded-lg px-3 py-2"
                                valueClassName="text-base"
                              />
                            </CarouselItem>
                          )}
                        </CarouselContent>
                      </Carousel>
                    ) : (
                      <div
                        className={cn(
                          "grid min-w-0 gap-4 rounded-lg p-2 backdrop-blur-sm sm:gap-5",
                          selectedItemData?.periodPnl != null
                            ? "grid-cols-5 xl:min-w-[58rem]"
                            : "grid-cols-4 xl:min-w-[46rem]",
                        )}
                      >
                        <HeaderMetric
                          label={selectedItemData?.label ?? "Return"}
                          infoText={selectedItemData?.infoText ?? SIMPLE_RETURN_INFO}
                          warningText={selectedItemData?.returnWarnings}
                          value={selectedItemData?.selectedMetricValue ?? null}
                          reason={selectedItemData?.selectedMetricReason}
                          valueClassName="text-base"
                        />
                        <HeaderMetric
                          label="Annualized Return"
                          infoText={annualizedReturnInfo}
                          value={selectedItemData?.annualizedReturn ?? null}
                          valueClassName="text-base"
                        />
                        <HeaderMetric
                          label="Volatility"
                          infoText={volatilityInfo}
                          warningText={selectedItemData?.volatilityWarnings}
                          value={selectedItemData?.volatility ?? null}
                          tone="neutral"
                          valueClassName="text-base"
                        />
                        <HeaderMetric
                          label="Max Drawdown"
                          infoText={maxDrawdownInfo}
                          value={selectedItemData?.maxDrawdown ?? null}
                          valueClassName="text-base"
                        />
                        {selectedItemData?.periodPnl != null && (
                          <AttributionDetailMetric
                            result={selectedItemData.result}
                            itemName={selectedItemData.name}
                            dateRangeLabel={displayDateRange}
                            isMobile={isMobile}
                            valueClassName="text-base"
                          />
                        )}
                      </div>
                    )}
                  </>
                )}
              </div>
            </CardHeader>
            <CardContent className={cn("min-h-0 flex-1", isMobile ? "p-2" : "p-3 sm:p-6")}>
              <div className="flex h-full min-h-0 flex-col gap-2">
                {comparisonNotice && (
                  <div className="border-border/70 bg-muted/20 flex items-start gap-2 rounded-md border border-dashed px-3 py-2 text-xs">
                    <div className="border-border bg-background mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full">
                      <Icons.Info className="text-muted-foreground h-3 w-3" />
                    </div>
                    <div className="min-w-0">
                      <div className="text-foreground font-medium">
                        {comparisonNotice.count} selected{" "}
                        {comparisonNotice.count === 1 ? "item is" : "items are"} not plotted
                      </div>
                      <div className="text-muted-foreground mt-0.5">{comparisonNotice.message}</div>
                    </div>
                  </div>
                )}
                <div className="min-h-0 flex-1">
                  <PerformanceContent
                    chartData={chartData}
                    isLoading={isLoadingPerformance}
                    hasErrors={hasErrors}
                    errorMessages={errorMessages}
                    isMobile={isMobile}
                  />
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </>
  );
}
