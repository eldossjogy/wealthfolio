import type { NetWorthHistoryPoint } from "@/lib/types";

// Goldish orange net-worth theme (matches the history chart).
export const THEME_COLOR = "hsl(38 75% 50%)";
export const THEME_COLOR_LIGHT = "hsl(38 75% 50% / 0.12)";

// Card styles mirrored from the spending dashboard (glass + solid variants).
export const CARD_GLASS =
  "border-border/60 bg-card/80 rounded-xl border p-4 backdrop-blur-xl md:p-5";
export const CARD_LABEL =
  "text-muted-foreground/70 text-[10px] font-semibold uppercase tracking-wide";

// Tailwind dot classes per asset category.
export const CATEGORY_COLORS: Record<string, string> = {
  cash: "bg-chart-9",
  investments: "bg-chart-1",
  properties: "bg-chart-2",
  vehicles: "bg-chart-3",
  collectibles: "bg-chart-4",
  preciousMetals: "bg-chart-5",
  otherAssets: "bg-muted-foreground",
  liabilities: "bg-destructive",
};

// CSS-variable colors per asset category (for inline styles / composition bar).
export const CATEGORY_CSS_COLORS: Record<string, string> = {
  cash: "var(--chart-9)",
  investments: "var(--chart-1)",
  properties: "var(--chart-2)",
  vehicles: "var(--chart-3)",
  collectibles: "var(--chart-4)",
  preciousMetals: "var(--chart-5)",
  otherAssets: "var(--muted-foreground)",
  liabilities: "var(--destructive)",
};

export interface BreakdownEntry {
  category: string;
  name: string;
  value: number;
  assetId?: string;
}

export interface ParsedNetWorth {
  netWorth: number;
  assets: { total: number; breakdown: BreakdownEntry[] };
  liabilities: { total: number; breakdown: BreakdownEntry[] };
}

export interface ParsedHistoryPoint {
  date: string;
  netWorth: number;
  totalAssets: number;
  totalLiabilities: number;
  portfolioValue: number;
  alternativeAssetsValue: number;
  netContribution: number;
  /** Per-category / per-liability value at this date (keys match the breakdown rows). */
  breakdown: Record<string, number>;
}

const num = (value: string | number | undefined | null): number => {
  if (value == null) return 0;
  const parsed = typeof value === "number" ? value : parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
};

/** Parse the raw history response (decimal strings, or numbers in web mode) into numbers. */
export function parseHistory(history: NetWorthHistoryPoint[] | undefined): ParsedHistoryPoint[] {
  if (!history) return [];
  return history.map((point) => {
    const breakdown: Record<string, number> = {};
    for (const [key, value] of Object.entries(point.breakdown ?? {})) {
      breakdown[key] = num(value);
    }
    return {
      date: point.date,
      netWorth: num(point.netWorth),
      totalAssets: num(point.totalAssets),
      totalLiabilities: num(point.totalLiabilities),
      portfolioValue: num(point.portfolioValue),
      alternativeAssetsValue: num(point.alternativeAssetsValue),
      netContribution: num(point.netContribution),
      breakdown,
    };
  });
}

/** Series of values for a breakdown key across the history (forward-filled by the backend). */
export function seriesFor(history: ParsedHistoryPoint[], key: string): number[] {
  return history.map((point) => point.breakdown[key] ?? 0);
}

const MS_PER_DAY = 86_400_000;
const DAYS_PER_MONTH = 365.25 / 12;

function daysBetween(fromISO: string, toISO: string): number {
  return Math.max(0, (new Date(toISO).getTime() - new Date(fromISO).getTime()) / MS_PER_DAY);
}

/** Net worth as-of a date: last point on/before it, else the earliest point. */
function netWorthAsOf(history: ParsedHistoryPoint[], iso: string): number {
  let result: number | null = null;
  for (const point of history) {
    if (point.date <= iso) result = point.netWorth;
    else break;
  }
  return result ?? history[0]?.netWorth ?? 0;
}

export interface Velocity {
  netChange: number;
  marketGains: number;
  contributions: number;
  equityBuilt: number;
  perMonth: number;
}

/**
 * Decompose net worth change over the range into market gains (portfolio price +
 * alternative-asset appreciation), contributions, and equity built (liability
 * reduction). These three sum to the net worth change.
 */
export function computeVelocity(history: ParsedHistoryPoint[]): Velocity | null {
  if (history.length < 2) return null;
  const first = history[0];
  const last = history[history.length - 1];

  const portfolioGain =
    last.portfolioValue - last.netContribution - (first.portfolioValue - first.netContribution);
  const altGain = last.alternativeAssetsValue - first.alternativeAssetsValue;
  const contributions = last.netContribution - first.netContribution;
  const equityBuilt = first.totalLiabilities - last.totalLiabilities;
  const netChange = last.netWorth - first.netWorth;

  const months = daysBetween(first.date, last.date) / DAYS_PER_MONTH;
  const perMonth = months > 0 ? netChange / months : netChange;

  return { netChange, marketGains: portfolioGain + altGain, contributions, equityBuilt, perMonth };
}

/** Average monthly net worth change across a history span (for the trailing-year baseline). */
export function averageMonthlyChange(history: ParsedHistoryPoint[]): number {
  return computeVelocity(history)?.perMonth ?? 0;
}

export interface Momentum {
  currentChange: number;
  priorChange: number | null;
  beatBy: number | null;
  bars: { month: string; value: number; current: boolean }[];
}

/**
 * Compare the current range's net worth change to the equal-length prior window.
 * `longHistory` must extend back at least one range-length before `rangeStartISO`.
 */
export function computeMomentum(
  longHistory: ParsedHistoryPoint[],
  rangeStartISO: string,
  rangeEndISO: string,
): Momentum | null {
  if (longHistory.length < 2) return null;

  const rangeDays = daysBetween(rangeStartISO, rangeEndISO) || 1;
  const priorStartISO = new Date(new Date(rangeStartISO).getTime() - rangeDays * MS_PER_DAY)
    .toISOString()
    .slice(0, 10);

  const nwEnd = netWorthAsOf(longHistory, rangeEndISO);
  const nwStart = netWorthAsOf(longHistory, rangeStartISO);
  const currentChange = nwEnd - nwStart;

  const earliest = longHistory[0].date;
  const hasPrior = earliest <= priorStartISO;
  const nwPriorStart = netWorthAsOf(longHistory, priorStartISO);
  const priorChange = hasPrior ? nwStart - nwPriorStart : null;
  const beatBy = priorChange != null ? currentChange - priorChange : null;

  // Monthly net worth change bars across the prior + current windows.
  const monthEnd = new Map<string, number>();
  for (const point of longHistory) {
    if (point.date < priorStartISO) continue;
    monthEnd.set(point.date.slice(0, 7), point.netWorth);
  }
  const months = [...monthEnd.keys()].sort();
  const currentMonth = rangeStartISO.slice(0, 7);
  let prev: number | null = null;
  const bars = months.map((month) => {
    const end = monthEnd.get(month)!;
    const value = prev == null ? 0 : end - prev;
    prev = end;
    return { month, value, current: month >= currentMonth };
  });

  return { currentChange, priorChange, beatBy, bars };
}
