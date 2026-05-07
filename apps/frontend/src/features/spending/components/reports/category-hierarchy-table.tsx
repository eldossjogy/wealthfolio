import { useMemo, useState } from "react";

import { Icons, Skeleton } from "@wealthfolio/ui";
import type { TaxonomyCategory } from "@/lib/types";
import { cn, formatAmount } from "@/lib/utils";

import type { BudgetAllocation } from "../../types/budget";
import type { CategoryBreakdownRow } from "../../types/report";

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
}

interface NodeRow {
  id: string;
  name: string;
  color: string | null;
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
}: CategoryHierarchyTableProps) {
  const tree = useMemo(
    () => buildTree({ breakdown, priorBreakdown, allocations, taxonomyCategories }),
    [breakdown, priorBreakdown, allocations, taxonomyCategories],
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
            <th className="px-3 py-2 text-right font-medium">Budgeted</th>
            <th className="px-3 py-2 text-right font-medium">Spent</th>
            <th className="px-3 py-2 text-right font-medium">Balance</th>
            <th className="px-3 py-2 text-right font-medium">Δ vs prior</th>
          </tr>
        </thead>
        <tbody>
          {tree.map((node) => (
            <ParentRow key={node.id} node={node} currency={currency} />
          ))}
        </tbody>
        <tfoot>
          <tr className="border-border/60 border-t text-sm font-semibold">
            <td className="px-3 py-2.5">Total</td>
            <td className="px-3 py-2.5 text-right tabular-nums">
              {formatAmount(totals.budgeted, currency)}
            </td>
            <td className="px-3 py-2.5 text-right tabular-nums">
              −{formatAmount(totals.spent, currency)}
            </td>
            <td
              className={cn(
                "px-3 py-2.5 text-right tabular-nums",
                totals.budgeted - totals.spent >= 0 ? "text-success" : "text-destructive",
              )}
            >
              {formatAmount(Math.abs(totals.budgeted - totals.spent), currency)}
            </td>
            <td
              className={cn(
                "px-3 py-2.5 text-right tabular-nums",
                totals.spent - totals.priorSpent >= 0 ? "text-destructive" : "text-success",
              )}
            >
              {formatDelta(totals.spent - totals.priorSpent, totals.priorSpent, currency)}
            </td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}

function ParentRow({ node, currency }: { node: NodeRow; currency: string }) {
  const [expanded, setExpanded] = useState(false);
  const hasChildren = node.children.length > 0;
  const balance = node.budgeted - node.spent;
  const delta = node.spent - node.priorSpent;

  return (
    <>
      <tr className="border-border/40 hover:bg-muted/30 group cursor-pointer border-b">
        <td className="px-3 py-2">
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            className="text-foreground flex items-center gap-1.5 text-left text-sm font-medium"
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
              className="h-2 w-2 shrink-0 rounded-full"
              style={{ backgroundColor: node.color ?? "var(--muted-foreground)" }}
            />
            {node.name}
          </button>
        </td>
        <td className="text-foreground/90 px-3 py-2 text-right tabular-nums">
          {node.budgeted > 0 ? formatAmount(node.budgeted, currency) : "—"}
        </td>
        <td className="text-foreground/90 px-3 py-2 text-right tabular-nums">
          −{formatAmount(node.spent, currency)}
        </td>
        <td
          className={cn(
            "px-3 py-2 text-right tabular-nums",
            node.budgeted > 0
              ? balance >= 0
                ? "text-success"
                : "text-destructive"
              : "text-muted-foreground",
          )}
        >
          {node.budgeted > 0 ? formatAmount(Math.abs(balance), currency) : "—"}
        </td>
        <td
          className={cn(
            "px-3 py-2 text-right tabular-nums",
            delta === 0
              ? "text-muted-foreground/70"
              : delta > 0
                ? "text-destructive"
                : "text-success",
          )}
        >
          {formatDelta(delta, node.priorSpent, currency)}
        </td>
      </tr>
      {expanded &&
        node.children.map((child) => <ChildRow key={child.id} node={child} currency={currency} />)}
    </>
  );
}

function ChildRow({ node, currency }: { node: NodeRow; currency: string }) {
  const delta = node.spent - node.priorSpent;
  return (
    <tr className="border-border/30 hover:bg-muted/20 border-b text-[13px]">
      <td className="text-muted-foreground/90 px-3 py-1.5 pl-9">{node.name}</td>
      <td className="text-muted-foreground/70 px-3 py-1.5 text-right tabular-nums">—</td>
      <td className="text-muted-foreground/90 px-3 py-1.5 text-right tabular-nums">
        −{formatAmount(node.spent, currency)}
      </td>
      <td className="text-muted-foreground/70 px-3 py-1.5 text-right tabular-nums">—</td>
      <td
        className={cn(
          "px-3 py-1.5 text-right tabular-nums",
          delta === 0
            ? "text-muted-foreground/60"
            : delta > 0
              ? "text-destructive"
              : "text-success",
        )}
      >
        {formatDelta(delta, node.priorSpent, currency)}
      </td>
    </tr>
  );
}

function formatDelta(delta: number, baseline: number, currency: string): string {
  if (delta === 0) return "—";
  const pct = baseline > 0 ? Math.abs(delta) / baseline : null;
  const arrow = delta > 0 ? "↑" : "↓";
  const amount = formatAmount(Math.abs(delta), currency);
  return pct != null ? `${arrow} ${amount} (${(pct * 100).toFixed(0)}%)` : `${arrow} ${amount}`;
}

// ────────────── tree builder ──────────────

function buildTree({
  breakdown,
  priorBreakdown,
  allocations,
  taxonomyCategories,
}: {
  breakdown: CategoryBreakdownRow[];
  priorBreakdown: CategoryBreakdownRow[];
  allocations: BudgetAllocation[];
  taxonomyCategories: TaxonomyCategory[];
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
  for (const id of allocationByCat.keys()) ensureNode(id);

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

  return Array.from(rolledUp.values())
    .filter((n) => n.spent > 0 || n.priorSpent > 0 || n.budgeted > 0)
    .sort((a, b) => b.spent - a.spent);
}
