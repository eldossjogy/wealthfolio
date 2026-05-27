import { cn } from "@/lib/utils";
import type { CategoryAllocation } from "@/lib/types";
import type { PortfolioStats } from "../hooks/use-portfolio-stats";

export interface ModelPreset {
  id: string;
  name: string;
  description: string;
  risk: string;
  expectedReturn?: number; // % annualized, historical estimate
  volatility?: number; // % annualized std dev, historical estimate
  // weights keyed by asset_classes category key (e.g. EQUITY, FIXED_INCOME, CASH...)
  weights: Record<string, number>; // 0-100
}

export const BUILT_IN_PRESETS: ModelPreset[] = [
  {
    id: "three_fund",
    name: "Three-Fund",
    description: "Bogleheads classic — US stocks, international & bonds",
    risk: "Moderate",
    expectedReturn: 7.0,
    volatility: 11.0,
    weights: { EQUITY: 60, FIXED_INCOME: 30, CASH: 10 },
  },
  {
    id: "sixty_forty",
    name: "60 / 40",
    description: "The benchmark — balanced stocks & bonds",
    risk: "Moderate",
    expectedReturn: 6.4,
    volatility: 10.1,
    weights: { EQUITY: 60, FIXED_INCOME: 40 },
  },
  {
    id: "all_weather",
    name: "All Weather",
    description: "Ray Dalio — diversified across all market regimes",
    risk: "Conservative",
    expectedReturn: 5.6,
    volatility: 7.8,
    weights: { EQUITY: 30, FIXED_INCOME: 55, COMMODITIES: 7, CASH: 8 },
  },
];

interface PresetBarProps {
  weights: Record<string, number>;
  colorMap: Record<string, string>;
}

const RISK_BADGE: Record<string, string> = {
  Conservative: "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
  Moderate: "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400",
  Aggressive: "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400",
  "From holdings": "bg-muted text-muted-foreground",
};

function PresetBar({ weights, colorMap }: PresetBarProps) {
  const nonZero = Object.entries(weights).filter(([, pct]) => pct > 0);
  return (
    <div className="space-y-3">
      <div className="flex h-3.5 w-full overflow-hidden rounded-sm">
        {nonZero.map(([key, pct]) => (
          <div key={key} style={{ width: `${pct}%`, background: colorMap[key] ?? "#878580" }} />
        ))}
      </div>
      <div className="flex flex-wrap gap-x-2 gap-y-0.5">
        {nonZero.map(([key, pct]) => (
          <span key={key} className="text-muted-foreground flex items-center gap-0.5 text-[10px]">
            <span
              className="inline-block h-2 w-1.5 rounded-full"
              style={{ background: colorMap[key] ?? "#878580" }}
            />
            {pct}%
          </span>
        ))}
      </div>
    </div>
  );
}

interface ModelPresetPickerProps {
  selected: string | null;
  onSelect: (presetId: string) => void;
  currentCategories: CategoryAllocation[];
  portfolioStats?: PortfolioStats | null;
}

export function ModelPresetPicker({
  selected,
  onSelect,
  currentCategories,
  portfolioStats,
}: ModelPresetPickerProps) {
  const colorMap = Object.fromEntries(currentCategories.map((c) => [c.categoryId, c.color]));

  const currentWeights = Object.fromEntries(
    currentCategories.map((c) => [c.categoryId, c.percentage]),
  );

  const currentPreset: ModelPreset = {
    id: "current",
    name: "Current allocation",
    description: "Start from what you hold today",
    risk: "From holdings",
    expectedReturn: portfolioStats?.annualizedReturn ?? undefined,
    volatility: portfolioStats?.volatility ?? undefined,
    weights: currentWeights,
  };

  const allPresets = [...BUILT_IN_PRESETS, currentPreset];

  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
      {allPresets.map((preset) => (
        <button
          key={preset.id}
          type="button"
          onClick={() => onSelect(preset.id)}
          className={cn(
            "flex min-h-[200px] flex-col rounded-lg border px-4 py-5 text-left transition-colors",
            selected === preset.id
              ? "border-foreground bg-muted/40"
              : "hover:border-muted-foreground/40 border-border",
          )}
        >
          <div className="flex items-start justify-between gap-1">
            <span className="text-foreground text-[15px] font-semibold leading-tight">
              {preset.name}
            </span>
            <span
              className={cn(
                "shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium",
                RISK_BADGE[preset.risk] ?? "bg-muted text-muted-foreground",
              )}
            >
              {preset.risk}
            </span>
          </div>
          <p className="text-muted-foreground mt-1 text-[11px] leading-relaxed">
            {preset.description}
          </p>
          <div className="mt-auto space-y-3 pt-8">
            <PresetBar weights={preset.weights} colorMap={colorMap} />
            <div className="border-border/60 border-t pt-2.5" />
            <div className="flex">
              <div className="flex-1">
                <div className="text-muted-foreground text-[10px] uppercase tracking-wider">
                  Exp. return
                </div>
                <div className="text-foreground text-[12px] font-semibold tabular-nums">
                  {preset.expectedReturn != null ? `${preset.expectedReturn.toFixed(1)}%` : "—"}
                </div>
              </div>
              <div className="flex-1">
                <div className="text-muted-foreground text-[10px] uppercase tracking-wider">
                  Volatility
                </div>
                <div className="text-foreground text-[12px] font-semibold tabular-nums">
                  {preset.volatility != null ? `${preset.volatility.toFixed(1)}%` : "—"}
                </div>
              </div>
            </div>
          </div>
        </button>
      ))}
    </div>
  );
}
