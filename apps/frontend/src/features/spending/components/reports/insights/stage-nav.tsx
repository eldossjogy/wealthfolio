import { cn } from "@/lib/utils";

export type InsightsStage = "where" | "changed" | "when";

interface StageNavProps {
  stage: InsightsStage;
  onStageChange: (s: InsightsStage) => void;
  /** Right-aligned context — usually "Mon D – Mon D, YYYY · N tx". */
  contextLabel?: string;
}

const STAGES: { id: InsightsStage; label: string }[] = [
  { id: "where", label: "Where I am" },
  { id: "changed", label: "What changed" },
  { id: "when", label: "When & where" },
];

export function StageNav({ stage, onStageChange, contextLabel }: StageNavProps) {
  return (
    <div className="border-border/60 bg-card/40 flex flex-wrap items-center gap-2 rounded-2xl border px-3 py-2 backdrop-blur-xl">
      <div className="flex flex-wrap items-center gap-1">
        {STAGES.map((s) => {
          const active = stage === s.id;
          return (
            <button
              key={s.id}
              type="button"
              onClick={() => onStageChange(s.id)}
              className={cn(
                "group inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-xs transition-colors",
                active
                  ? "bg-foreground text-background"
                  : "text-muted-foreground hover:text-foreground hover:bg-muted/50",
              )}
              aria-current={active ? "step" : undefined}
            >
              <span className="font-medium">{s.label}</span>
            </button>
          );
        })}
      </div>
      {contextLabel && (
        <div className="text-muted-foreground/80 ml-auto text-xs tabular-nums">{contextLabel}</div>
      )}
    </div>
  );
}
