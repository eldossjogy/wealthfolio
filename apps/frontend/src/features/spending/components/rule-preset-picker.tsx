import { Icons } from "@wealthfolio/ui";
import { cn } from "@/lib/utils";

import { useImportRulePreset, useRulePresets } from "../hooks/use-categorization-rules";

const FLAGS: Record<string, string> = {
  us: "🇺🇸",
  ca: "🇨🇦",
  gb: "🇬🇧",
};

interface RulePresetPickerProps {
  /** Compact rendering for inline placement on overview cards. */
  compact?: boolean;
}

/**
 * Country picker that seeds the categorization-rules table from a bundled
 * preset (US/CA/GB). Idempotent — re-importing skips already-installed rules.
 */
export function RulePresetPicker({ compact = false }: RulePresetPickerProps) {
  const { data: presets = [], isLoading } = useRulePresets();
  const importMutation = useImportRulePreset();

  if (isLoading) {
    return <div className="text-muted-foreground text-xs">Loading presets…</div>;
  }
  if (presets.length === 0) return null;

  return (
    <div className={compact ? "space-y-2" : "space-y-3"}>
      {!compact && (
        <div className="space-y-0.5">
          <div className="text-foreground text-sm font-medium">Start with a preset</div>
          <p className="text-muted-foreground text-xs">
            Pick your country to seed common merchant rules. Skips rules you already have.
          </p>
        </div>
      )}
      <div className="flex flex-wrap items-stretch gap-2">
        {presets.map((preset) => {
          const flag = FLAGS[preset.presetId] ?? "🌐";
          const isImporting =
            importMutation.isPending && importMutation.variables === preset.presetId;
          return (
            <button
              key={preset.presetId}
              type="button"
              onClick={() => importMutation.mutate(preset.presetId)}
              disabled={importMutation.isPending}
              aria-label={
                preset.installed
                  ? `Re-import ${preset.name} preset (${preset.ruleCount} rules)`
                  : `Import ${preset.name} preset (${preset.ruleCount} rules)`
              }
              className={cn(
                "border-input bg-card hover:bg-muted/50 group flex min-w-[160px] items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors",
                "focus-visible:ring-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2",
                "disabled:cursor-not-allowed disabled:opacity-60",
                preset.installed && "border-success/40 bg-success/5",
              )}
            >
              <span className="text-xl leading-none" aria-hidden="true">
                {flag}
              </span>
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="text-foreground text-sm font-medium leading-tight">
                  {preset.name}
                </span>
                <span className="text-muted-foreground text-[11px] leading-tight">
                  {preset.ruleCount} rules
                </span>
              </div>
              {isImporting ? (
                <Icons.Spinner
                  className="text-muted-foreground h-4 w-4 shrink-0 animate-spin"
                  aria-hidden="true"
                />
              ) : preset.installed ? (
                <Icons.Check className="text-success h-4 w-4 shrink-0" aria-hidden="true" />
              ) : (
                <Icons.ArrowRight
                  className="text-muted-foreground h-4 w-4 shrink-0 opacity-0 transition-opacity group-hover:opacity-100"
                  aria-hidden="true"
                />
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}
