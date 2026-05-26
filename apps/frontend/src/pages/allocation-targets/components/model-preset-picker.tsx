import { cn } from "@/lib/utils";
import type { CategoryAllocation } from "@/lib/types";

export interface ModelPreset {
  id: string;
  name: string;
  description: string;
  risk: string;
  // weights keyed by asset_classes category key (e.g. EQUITY, FIXED_INCOME, CASH...)
  weights: Record<string, number>; // 0-100
}

export const BUILT_IN_PRESETS: ModelPreset[] = [
  {
    id: "three_fund",
    name: "Three-Fund",
    description: "Bogleheads classic",
    risk: "Moderate",
    weights: { EQUITY: 60, FIXED_INCOME: 30, CASH: 10 },
  },
  {
    id: "sixty_forty",
    name: "60 / 40",
    description: "The benchmark",
    risk: "Moderate",
    weights: { EQUITY: 60, FIXED_INCOME: 40 },
  },
  {
    id: "all_weather",
    name: "All Weather",
    description: "Ray Dalio",
    risk: "Conservative",
    weights: { EQUITY: 30, FIXED_INCOME: 55, COMMODITIES: 7, CASH: 8 },
  },
];

interface PresetBarProps {
  weights: Record<string, number>;
  colorMap: Record<string, string>;
}

function PresetBar({ weights, colorMap }: PresetBarProps) {
  const nonZero = Object.entries(weights).filter(([, pct]) => pct > 0);
  return (
    <div className="space-y-1.5">
      <div className="flex h-2 w-full overflow-hidden rounded-full">
        {nonZero.map(([key, pct]) => (
          <div key={key} style={{ width: `${pct}%`, background: colorMap[key] ?? "#878580" }} />
        ))}
      </div>
      <div className="flex flex-wrap gap-x-2 gap-y-0.5">
        {nonZero.map(([key, pct]) => (
          <span key={key} className="text-muted-foreground flex items-center gap-0.5 text-[10px]">
            <span
              className="inline-block h-1.5 w-1.5 rounded-full"
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
}

export function ModelPresetPicker({
  selected,
  onSelect,
  currentCategories,
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
    weights: currentWeights,
  };

  const allPresets = [...BUILT_IN_PRESETS, currentPreset];

  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
      {allPresets.map((preset) => (
        <button
          key={preset.id}
          type="button"
          onClick={() => onSelect(preset.id)}
          className={cn(
            "rounded-lg border p-3 text-left transition-colors",
            selected === preset.id
              ? "border-foreground bg-muted/40"
              : "hover:border-muted-foreground/40 border-border",
          )}
        >
          <div className="flex items-start justify-between gap-1">
            <span className="text-foreground text-[13px] font-semibold leading-tight">
              {preset.name}
            </span>
            <span className="text-muted-foreground shrink-0 text-[10px]">{preset.risk}</span>
          </div>
          <p className="text-muted-foreground mt-0.5 text-[11px]">{preset.description}</p>
          <div className="mt-2">
            <PresetBar weights={preset.weights} colorMap={colorMap} />
          </div>
        </button>
      ))}
    </div>
  );
}
