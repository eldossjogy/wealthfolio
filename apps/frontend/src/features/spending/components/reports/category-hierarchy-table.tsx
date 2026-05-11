import { useMemo, useState } from "react";

import { Icons, Skeleton } from "@wealthfolio/ui";
import type { TaxonomyCategory } from "@/lib/types";
import { cn, formatAmount } from "@/lib/utils";

import { CategoryIcon } from "../category-chips";
import type { BudgetAllocation } from "../../types/budget";
import type { CategoryBreakdownRow } from "../../types/report";

export type CategorySort = "spent" | "delta" | "name";

interface CategoryHierarchyTableProps {
  /** Spending breakdown for the current period (flat rows from backend). */
  breakdown: CategoryBreakdownRow[];
  /** Prior-period breakdown — drives the Δ column. */
  priorBreakdown: CategoryBreakdownRow[];
  /** Per-category budget allocations (top-level categories only in our model). */
  allocations: BudgetAllocation[];
  /** Taxonomy metadata (used to resolve names + parent ids). */
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  isLoading: boolean;
  /** Sort order for top-level rows. Defaults to "spent" (largest first). */
  sort?: CategorySort;
}

interface NodeRow {
  id: string;
  name: string;
  color: string | null;
  icon: string | null;
  parentId: string | null;
  /** Spent in current period. */
  spent: number;
  /** Spent in prior period. */
  priorSpent: number;
  /** Budgeted (top-level only). */
  budgeted: number;
  /** Children flat list. */
  children: NodeRow[];
}

/**
 * Hierarchical Budgeted / Spent / Balance / Δ table.
 *
 * Rolls flat backend rows into a top-level → subcategory tree using the
 * taxonomy `parentId` graph. Top-level rows can expand to show their leaves;
 * leaves contribute their `spent` upward into the parent total.
 */
export function CategoryHierarchyTable({
  breakdown,
  priorBreakdown,
  allocations,
  taxonomyCategories,
  currency,
  isLoading,
  sort = "spent",
}: CategoryHierarchyTableProps) {
  const tree = useMemo(
    () => buildTree({ breakdown, priorBreakdown, allocations, taxonomyCategories, sort }),
    [breakdown, priorBreakdown, allocations, taxonomyCategories, sort],
  );

  const totals = useMemo(() => {
    const t = { budgeted: 0, spent: 0, priorSpent: 0 };
    for (const node of tree) {
      t.budgeted += node.budgeted;
      t.spent += node.spent;
      t.priorSpent += node.priorSpent;
    }
    return t;
  }, [tree]);

  if (isLoading) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 6 }).map((_, i) => (
          <Skeleton key={i} className="h-9 w-full" />
        ))}
      </div>
    );
  }

  if (tree.length === 0) {
    return (
      <div className="text-muted-foreground py-8 text-center text-sm">
        No categorized spending in this period.
      </div>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="text-foreground w-full text-sm">
        <thead>
          <tr className="border-border/60 text-muted-foreground/80 border-b text-[11px] uppercase tracking-wide">
            <th className="px-3 py-2 text-left font-medium">Category</th>
            <th className="px-3 py-2 text-right font-medium">Spent / Budget</th>
            <th className="px-3 py-2 text-left font-medium">Progress</th>
            <th className="px-3 py-2 text-right font-medium">Δ vs prior</th>
          </tr>
        </thead>
        <tbody>
          {tree.map((node) => (
            <ParentRow key={node.id} node={node} currency={currency} />
          ))}
        </tbody>
        <tfoot>
          <tr className="border-border/60 border-t text-sm font-medium">
            <td className="px-3 py-2.5">Total</td>
            <td className="px-3 py-2.5 text-right text-xs tabular-nums">
              <span className="text-foreground font-medium">
                −{formatAmount(totals.spent, currency)}
              </span>
              {totals.budgeted > 0 && (
                <span className="text-muted-foreground/70 ml-1">
                  / {formatAmount(totals.budgeted, currency)}
                </span>
              )}
            </td>
            <td className="px-3 py-2.5">
              {totals.budgeted > 0 ? (
                <ProgressBar spent={totals.spent} budget={totals.budgeted} />
              ) : (
                <span className="text-muted-foreground/50 text-xs">No budget set</span>
              )}
            </td>
            <td
              className={cn(
                "px-3 py-2.5 text-right text-xs tabular-nums",
                totals.priorSpent === 0 || totals.spent - totals.priorSpent === 0
                  ? "text-muted-foreground/70"
                  : totals.spent - totals.priorSpent > 0
                    ? "text-destructive"
                    : "text-success",
              )}
            >
              {formatDelta(totals.spent - totals.priorSpent, totals.priorSpent)}
            </td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}

function ProgressBar({ spent, budget }: { spent: number; budget: number }) {
  if (budget <= 0) return null;
  const pct = (spent / budget) * 100;
  const isOver = pct > 100;
  const isClose = pct >= 85 && !isOver;
  const fillColor = isOver ? "var(--destructive)" : isClose ? "#C28B47" : "var(--success)";
  return (
    <div className="flex items-center gap-2">
      <div className="bg-foreground/10 relative h-1.5 min-w-[60px] flex-1 overflow-hidden rounded-full">
        <div
          className="h-full rounded-full transition-all"
          style={{ width: `${Math.min(100, pct)}%`, backgroundColor: fillColor, opacity: 0.65 }}
        />
      </div>
      <span
        className={cn(
          "w-10 shrink-0 text-right text-[11px] tabular-nums",
          isOver ? "text-destructive font-medium" : "text-muted-foreground/80",
        )}
      >
        {pct.toFixed(0)}%
      </span>
    </div>
  );
}

function ParentRow({ node, currency }: { node: NodeRow; currency: string }) {
  const [expanded, setExpanded] = useState(false);
  const hasChildren = node.children.length > 0;
  const delta = node.spent - node.priorSpent;
  const accent = node.color ?? "var(--muted-foreground)";
  const tintBg = node.color ? `${node.color}1F` : "var(--muted)";

  return (
    <>
      <tr className="border-border/40 hover:bg-muted/30 group cursor-pointer border-b">
        <td className="px-3 py-2.5">
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            className="text-foreground flex items-center gap-2 text-left text-sm font-medium"
            aria-expanded={expanded}
            disabled={!hasChildren}
          >
            <Icons.ChevronRight
              className={cn(
                "text-muted-foreground/70 h-3.5 w-3.5 transition-transform",
                expanded && "rotate-90",
                !hasChildren && "opacity-0",
              )}
            />
            <span
              className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full"
              style={{ backgroundColor: tintBg, color: accent }}
            >
              <CategoryIcon icon={node.icon} fallback={node.name} className="h-3.5 w-3.5" />
            </span>
            <span>{node.name}</span>
          </button>
        </td>
        <td className="text-foreground/90 px-3 py-2.5 text-right text-xs tabular-nums">
          <span className="text-foreground font-medium">−{formatAmount(node.spent, currency)}</span>
          {node.budgeted > 0 && (
            <span className="text-muted-foreground/70 ml-1">
              / {formatAmount(node.budgeted, currency)}
            </span>
          )}
        </td>
        <td className="px-3 py-2.5">
          {node.budgeted > 0 ? (
            <ProgressBar spent={node.spent} budget={node.budgeted} />
          ) : (
            <span className="text-muted-foreground/50 text-xs">No budget set</span>
          )}
        </td>
        <td
          className={cn(
            "px-3 py-2.5 text-right text-xs tabular-nums",
            delta === 0 || node.priorSpent === 0
              ? "text-muted-foreground/70"
              : delta > 0
                ? "text-destructive"
                : "text-success",
          )}
        >
          {formatDelta(delta, node.priorSpent)}
        </td>
      </tr>
      {expanded &&
        node.children.map((child) => (
          <ChildRow key={child.id} node={child} currency={currency} parentColor={accent} />
        ))}
    </>
  );
}

function ChildRow({
  node,
  currency,
  parentColor,
}: {
  node: NodeRow;
  currency: string;
  parentColor: string;
}) {
  const delta = node.spent - node.priorSpent;
  return (
    <tr className="border-border/30 hover:bg-muted/20 border-b text-[13px]">
      <td className="text-muted-foreground/90 px-3 py-1.5 pl-9">
        <div className="flex items-center gap-2">
          <span
            className="h-1 w-1 shrink-0 rounded-full"
            style={{ backgroundColor: parentColor, opacity: 0.6 }}
          />
          <span>{node.name}</span>
        </div>
      </td>
      <td className="text-muted-foreground/90 px-3 py-1.5 text-right text-xs tabular-nums">
        −{formatAmount(node.spent, currency)}
      </td>
      <td className="px-3 py-1.5"></td>
      <td
        className={cn(
          "px-3 py-1.5 text-right text-xs tabular-nums",
          delta === 0 || node.priorSpent === 0
            ? "text-muted-foreground/60"
            : delta > 0
              ? "text-destructive"
              : "text-success",
        )}
      >
        {formatDelta(delta, node.priorSpent)}
      </td>
    </tr>
  );
}

function formatDelta(delta: number, baseline: number): string {
  if (delta === 0) return "—";
  // No prior period spend — label as "new" instead of restating current amount.
  if (baseline === 0) return delta > 0 ? "new" : "—";
  const pct = (Math.abs(delta) / baseline) * 100;
  const arrow = delta > 0 ? "↑" : "↓";
  return `${arrow} ${pct.toFixed(0)}%`;
}

// ────────────── tree builder ──────────────

function buildTree({
  breakdown,
  priorBreakdown,
  allocations,
  taxonomyCategories,
  sort,
}: {
  breakdown: CategoryBreakdownRow[];
  priorBreakdown: CategoryBreakdownRow[];
  allocations: BudgetAllocation[];
  taxonomyCategories: TaxonomyCategory[];
  sort: CategorySort;
}): NodeRow[] {
  const meta = new Map(taxonomyCategories.map((c) => [c.id, c]));
  const allocationByCat = new Map(
    allocations.map((a) => [a.categoryId, parseFloat(a.amount) || 0]),
  );

  // Build a row per taxonomy category that has *any* signal (spend, prior spend, or budget).
  const nodes = new Map<string, NodeRow>();
  const ensureNode = (id: string): NodeRow => {
    let n = nodes.get(id);
    if (!n) {
      const m = meta.get(id);
      n = {
        id,
        name: m?.name ?? id,
        color: m?.color ?? null,
        icon: m?.icon ?? null,
        parentId: m?.parentId ?? null,
        spent: 0,
        priorSpent: 0,
        budgeted: allocationByCat.get(id) ?? 0,
        children: [],
      };
      nodes.set(id, n);
    }
    return n;
  };

  for (const r of breakdown) ensureNode(r.categoryId).spent += r.amount;
  for (const r of priorBreakdown) ensureNode(r.categoryId).priorSpent += r.amount;
  // Only ensure nodes for allocations that target a spending-taxonomy category;
  // budget allocations pointing at income/other taxonomies must not appear here.
  for (const id of allocationByCat.keys()) {
    if (meta.has(id)) ensureNode(id);
  }

  // Roll subcategory amounts up to top-level parent for the parent row totals.
  // This keeps parent.spent = direct + children spend, matching the visual mental model.
  const rolledUp = new Map<string, NodeRow>();
  const ensureRolled = (id: string): NodeRow => {
    let n = rolledUp.get(id);
    if (!n) {
      const base = nodes.get(id);
      const m = meta.get(id);
      n = {
        id,
        name: base?.name ?? m?.name ?? id,
        color: base?.color ?? m?.color ?? null,
        icon: base?.icon ?? m?.icon ?? null,
        parentId: null,
        spent: base?.spent ?? 0,
        priorSpent: base?.priorSpent ?? 0,
        budgeted: base?.budgeted ?? 0,
        children: [],
      };
      rolledUp.set(id, n);
    }
    return n;
  };
  for (const node of nodes.values()) {
    if (node.parentId == null) {
      ensureRolled(node.id);
    } else {
      const parent = ensureRolled(node.parentId);
      parent.spent += node.spent;
      parent.priorSpent += node.priorSpent;
      parent.children.push({ ...node, children: [] });
    }
  }

  const compare =
    sort === "name"
      ? (a: NodeRow, b: NodeRow) => a.name.localeCompare(b.name)
      : sort === "delta"
        ? (a: NodeRow, b: NodeRow) =>
            Math.abs(b.spent - b.priorSpent) - Math.abs(a.spent - a.priorSpent)
        : (a: NodeRow, b: NodeRow) => b.spent - a.spent;

  return Array.from(rolledUp.values())
    .filter((n) => n.spent > 0 || n.priorSpent > 0 || n.budgeted > 0)
    .sort(compare);
}
