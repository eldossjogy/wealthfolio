import { useState } from "react";
import { cn } from "@/lib/utils";
import { Icons } from "@wealthfolio/ui";
import type { TaxonomyCategory } from "@/lib/types";

export interface NodeDraft {
  categoryId: string;
  targetBps: number; // 0–10000
  isLocked: boolean;
}

interface TargetNodeEditorProps {
  categories: TaxonomyCategory[];
  nodes: NodeDraft[];
  currentAllocation?: Record<string, number>; // categoryId → pct 0-100
  onChange: (nodes: NodeDraft[]) => void;
}

function redistribute(nodes: NodeDraft[], changedId: string, newBps: number): NodeDraft[] {
  const bps = Math.max(0, Math.min(10000, newBps));
  const updated = nodes.map((n) => (n.categoryId === changedId ? { ...n, targetBps: bps } : n));

  const fixedTotal = updated.reduce(
    (s, n) => (n.categoryId === changedId || n.isLocked ? s + n.targetBps : s),
    0,
  );
  const remaining = 10000 - fixedTotal;
  const flexible = updated.filter((n) => n.categoryId !== changedId && !n.isLocked);

  if (flexible.length === 0) return updated;
  if (remaining <= 0) {
    return updated.map((n) =>
      n.categoryId !== changedId && !n.isLocked ? { ...n, targetBps: 0 } : n,
    );
  }

  const flexTotal = flexible.reduce((s, n) => s + n.targetBps, 0);

  if (flexTotal === 0) {
    const perCat = Math.floor(remaining / flexible.length);
    let leftover = remaining - perCat * flexible.length;
    return updated.map((n) => {
      if (n.categoryId === changedId || n.isLocked) return n;
      const extra = leftover-- > 0 ? 1 : 0;
      return { ...n, targetBps: perCat + extra };
    });
  }

  // Proportional
  let distributed = 0;
  const result = updated.map((n) => {
    if (n.categoryId === changedId || n.isLocked) return n;
    const share = Math.round((n.targetBps / flexTotal) * remaining);
    distributed += share;
    return { ...n, targetBps: share };
  });
  // Fix rounding error on largest flexible node
  const diff = remaining - distributed;
  if (diff !== 0) {
    let largestIdx = -1;
    let largestBps = -1;
    result.forEach((n, i) => {
      if (n.categoryId !== changedId && !n.isLocked && n.targetBps > largestBps) {
        largestBps = n.targetBps;
        largestIdx = i;
      }
    });
    if (largestIdx >= 0)
      result[largestIdx] = {
        ...result[largestIdx],
        targetBps: result[largestIdx].targetBps + diff,
      };
  }
  return result;
}

export function TargetNodeEditor({
  categories,
  nodes,
  currentAllocation = {},
  onChange,
}: TargetNodeEditorProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");

  const totalBps = nodes.reduce((s, n) => s + n.targetBps, 0);
  const isValid = totalBps === 10000;

  function getNodeBps(categoryId: string): number {
    return nodes.find((n) => n.categoryId === categoryId)?.targetBps ?? 0;
  }
  function getIsLocked(categoryId: string): boolean {
    return nodes.find((n) => n.categoryId === categoryId)?.isLocked ?? false;
  }

  function startEdit(categoryId: string) {
    if (getIsLocked(categoryId)) return;
    setEditingId(categoryId);
    setEditValue((getNodeBps(categoryId) / 100).toFixed(1));
  }

  function commitEdit(categoryId: string) {
    const pct = parseFloat(editValue);
    const bps = isNaN(pct) ? 0 : Math.round(Math.min(100, Math.max(0, pct)) * 100);
    onChange(redistribute(nodes, categoryId, bps));
    setEditingId(null);
  }

  function toggleLock(categoryId: string) {
    onChange(nodes.map((n) => (n.categoryId === categoryId ? { ...n, isLocked: !n.isLocked } : n)));
  }

  return (
    <div className="space-y-1">
      {/* Allocation bar */}
      {totalBps > 0 && (
        <div className="mb-3 flex h-2 w-full overflow-hidden rounded-full">
          {categories.map((cat) => {
            const bps = getNodeBps(cat.id);
            if (bps === 0) return null;
            return (
              <div
                key={cat.id}
                style={{ width: `${bps / 100}%`, background: cat.color }}
                title={`${cat.name} ${(bps / 100).toFixed(0)}%`}
              />
            );
          })}
        </div>
      )}

      {/* Column headers */}
      <div className="mb-1 grid grid-cols-[1fr_56px_80px_28px] gap-2 px-1">
        <span className="text-muted-foreground text-[10px] font-medium uppercase tracking-wider">
          Asset class
        </span>
        <span className="text-muted-foreground text-right text-[10px] font-medium uppercase tracking-wider">
          Current
        </span>
        <span className="text-muted-foreground text-right text-[10px] font-medium uppercase tracking-wider">
          Target
        </span>
        <span />
      </div>

      {/* Category rows */}
      {categories.map((cat) => {
        const bps = getNodeBps(cat.id);
        const isLocked = getIsLocked(cat.id);
        const isEditing = editingId === cat.id;
        const currentPct = currentAllocation[cat.id] ?? 0;

        return (
          <div
            key={cat.id}
            className="hover:bg-muted/30 group grid grid-cols-[1fr_56px_80px_28px] items-center gap-2 rounded px-1 py-1.5"
          >
            {/* Name */}
            <div className="flex items-center gap-2">
              <span className="h-2 w-2 shrink-0 rounded-sm" style={{ background: cat.color }} />
              <span className="text-foreground text-[13px]">{cat.name}</span>
            </div>

            {/* Current % */}
            <span className="text-muted-foreground text-right text-[12px] tabular-nums">
              {currentPct > 0 ? `${currentPct.toFixed(1)}%` : "—"}
            </span>

            {/* Target input */}
            <div className="flex items-center justify-end gap-0.5">
              {isEditing ? (
                <input
                  type="number"
                  min={0}
                  max={100}
                  step={1}
                  value={editValue}
                  autoFocus
                  onChange={(e) => setEditValue(e.target.value)}
                  onBlur={() => commitEdit(cat.id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commitEdit(cat.id);
                    if (e.key === "Escape") setEditingId(null);
                  }}
                  className="border-primary bg-background focus:ring-primary w-12 rounded border px-1.5 py-1 text-right text-[13px] tabular-nums [appearance:textfield] focus:outline-none focus:ring-1 [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
                />
              ) : (
                <button
                  type="button"
                  onClick={() => startEdit(cat.id)}
                  disabled={isLocked}
                  className={cn(
                    "w-12 rounded px-1.5 py-1 text-right text-[13px] tabular-nums transition-colors",
                    isLocked
                      ? "text-muted-foreground cursor-not-allowed opacity-60"
                      : "hover:bg-muted cursor-pointer",
                    bps > 0 ? "text-foreground font-medium" : "text-muted-foreground",
                  )}
                >
                  {(bps / 100).toFixed(1)}
                </button>
              )}
              <span className="text-muted-foreground text-[12px]">%</span>
            </div>

            {/* Lock */}
            <button
              type="button"
              onClick={() => toggleLock(cat.id)}
              className={cn(
                "flex h-6 w-6 items-center justify-center rounded transition-colors",
                isLocked
                  ? "text-foreground"
                  : "text-muted-foreground/40 hover:text-foreground opacity-0 group-hover:opacity-100",
              )}
              title={isLocked ? "Unlock" : "Lock"}
            >
              {isLocked ? (
                <Icons.Pin className="h-3.5 w-3.5" />
              ) : (
                <Icons.PinOff className="h-3.5 w-3.5" />
              )}
            </button>
          </div>
        );
      })}

      {/* Total row */}
      <div className="border-t pt-2">
        <div className="grid grid-cols-[1fr_56px_80px_28px] gap-2 px-1">
          <span className="text-[13px] font-medium">Total</span>
          <span />
          <span
            className={cn(
              "text-right text-[14px] font-semibold tabular-nums",
              isValid ? "text-green-700 dark:text-green-400" : "text-destructive",
            )}
          >
            {(totalBps / 100).toFixed(1)}%{isValid && " ✓"}
          </span>
          <span />
        </div>
        {!isValid && (
          <p className="text-destructive mt-0.5 px-1 text-[11px]">
            {totalBps < 10000
              ? `${((10000 - totalBps) / 100).toFixed(1)}% unallocated`
              : `${((totalBps - 10000) / 100).toFixed(1)}% over 100%`}
          </p>
        )}
      </div>
    </div>
  );
}
