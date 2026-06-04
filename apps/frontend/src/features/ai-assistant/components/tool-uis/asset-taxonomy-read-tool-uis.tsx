import type { ToolCallMessagePartProps } from "@assistant-ui/react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { memo } from "react";
import type {
  GetAssetTaxonomyAssignmentsArgs,
  GetAssetTaxonomyAssignmentsOutput,
  ListAssetTaxonomiesArgs,
  ListAssetTaxonomiesOutput,
} from "../../types";

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function cleanErrorMessage(raw: string): string {
  return raw
    .replace(/^Toolset error:\s*/i, "")
    .replace(/ToolCallError:\s*/g, "")
    .replace(/^Tool execution failed:\s*/i, "")
    .replace(/^JsonError:\s*/i, "")
    .trim();
}

function friendlyErrorMessage(raw: string): string {
  const cleaned = cleanErrorMessage(raw);
  const lower = cleaned.toLowerCase();

  if (
    lower.includes("__placeholder__") ||
    lower.includes("asset-scoped taxonomy") ||
    lower.includes("taxonomy filter")
  ) {
    return "Could not match that taxonomy. I’ll use the available asset taxonomies.";
  }

  if (lower.includes("unknown") && lower.includes("category")) {
    return "Unknown is not a category here. I’ll leave it unallocated.";
  }

  if (lower.includes("ambiguous")) {
    return "More than one asset matched. Ask again with the exchange, currency, or asset ID.";
  }

  if (lower.includes("not found among active assets")) {
    return "Could not find that asset among active assets.";
  }

  return "Could not complete that lookup. I’ll continue with the available information.";
}

function extractToolErrorMessage(value: unknown): string | null {
  if (!value) return null;

  if (typeof value === "string") {
    try {
      return extractToolErrorMessage(JSON.parse(value));
    } catch {
      return friendlyErrorMessage(value);
    }
  }

  if (!isRecord(value)) {
    return typeof value === "number" || typeof value === "boolean"
      ? friendlyErrorMessage(String(value))
      : null;
  }

  if (typeof value.error === "string") return friendlyErrorMessage(value.error);
  if (typeof value.message === "string") return friendlyErrorMessage(value.message);
  if (typeof value.content === "string") return friendlyErrorMessage(value.content);

  if ("data" in value) {
    return extractToolErrorMessage(value.data);
  }

  return null;
}

function normalizeListAssetTaxonomiesResult(value: unknown): ListAssetTaxonomiesOutput | undefined {
  if (!value) return undefined;

  if (typeof value === "string") {
    try {
      return normalizeListAssetTaxonomiesResult(JSON.parse(value));
    } catch {
      return undefined;
    }
  }

  if (!isRecord(value)) return undefined;

  if ("data" in value) {
    const normalized = normalizeListAssetTaxonomiesResult(value.data);
    if (normalized) return normalized;
  }

  if (Array.isArray(value.taxonomies)) {
    return value as unknown as ListAssetTaxonomiesOutput;
  }

  return undefined;
}

function normalizeGetAssetTaxonomyAssignmentsResult(
  value: unknown,
): GetAssetTaxonomyAssignmentsOutput | undefined {
  if (!value) return undefined;

  if (typeof value === "string") {
    try {
      return normalizeGetAssetTaxonomyAssignmentsResult(JSON.parse(value));
    } catch {
      return undefined;
    }
  }

  if (!isRecord(value)) return undefined;

  if ("data" in value) {
    const normalized = normalizeGetAssetTaxonomyAssignmentsResult(value.data);
    if (normalized) return normalized;
  }

  if (Array.isArray(value.assignments)) {
    return value as unknown as GetAssetTaxonomyAssignmentsOutput;
  }

  return undefined;
}

function InlineToolError({ label }: { label: string }) {
  return (
    <div className="text-destructive flex items-center gap-2 px-1 text-xs">
      <Icons.AlertCircle className="h-3 w-3" />
      <span className="break-words">{label}</span>
    </div>
  );
}

function InlineLoading({ label }: { label: string }) {
  return (
    <div className="text-muted-foreground flex items-center gap-2 px-1 text-xs">
      <Icons.Spinner className="h-3 w-3 animate-spin" />
      <span>{label}</span>
    </div>
  );
}

function ListAssetTaxonomiesContentImpl({
  result,
  status,
}: ToolCallMessagePartProps<ListAssetTaxonomiesArgs, ListAssetTaxonomiesOutput>) {
  if (status?.type === "running") return <InlineLoading label="Loading asset taxonomies..." />;
  if (!result) return null;

  const parsedResult = normalizeListAssetTaxonomiesResult(result);
  if (!parsedResult) {
    return (
      <InlineToolError
        label={extractToolErrorMessage(result) ?? "Could not load asset taxonomies."}
      />
    );
  }

  const returnedCategoryCount = parsedResult.taxonomies.reduce(
    (sum, taxonomy) => sum + (taxonomy.categories?.length ?? 0),
    0,
  );
  const totalCategoryCount = parsedResult.taxonomies.reduce(
    (sum, taxonomy) => sum + (taxonomy.categoryCount ?? taxonomy.categories?.length ?? 0),
    0,
  );
  const focusedTaxonomy = parsedResult.taxonomies.length === 1 ? parsedResult.taxonomies[0] : null;

  return (
    <div className="text-muted-foreground flex items-center gap-2 px-1 text-xs">
      <Icons.ListChecks className="h-3 w-3" />
      {returnedCategoryCount > 0 && focusedTaxonomy ? (
        <span>
          Loaded {returnedCategoryCount} categories for {focusedTaxonomy.name}
          {totalCategoryCount > returnedCategoryCount ? ` · ${totalCategoryCount} total` : ""}
        </span>
      ) : (
        <span>Loaded {parsedResult.taxonomies.length} asset taxonomies</span>
      )}
    </div>
  );
}

function GetAssetTaxonomyAssignmentsContentImpl({
  result,
  status,
}: ToolCallMessagePartProps<GetAssetTaxonomyAssignmentsArgs, GetAssetTaxonomyAssignmentsOutput>) {
  if (status?.type === "running") return <InlineLoading label="Loading classifications..." />;
  if (!result) return null;

  const parsedResult = normalizeGetAssetTaxonomyAssignmentsResult(result);
  if (!parsedResult) {
    return (
      <InlineToolError
        label={extractToolErrorMessage(result) ?? "Could not load classifications."}
      />
    );
  }

  return (
    <div className="text-muted-foreground flex items-center gap-2 px-1 text-xs">
      <Icons.ListChecks className="h-3 w-3" />
      <span>
        Loaded {parsedResult.assignments.length} classifications for{" "}
        {parsedResult.resolvedAsset?.label ?? parsedResult.assetQuery}
      </span>
    </div>
  );
}

const ListAssetTaxonomiesContent = memo(ListAssetTaxonomiesContentImpl);
const GetAssetTaxonomyAssignmentsContent = memo(GetAssetTaxonomyAssignmentsContentImpl);

export const ListAssetTaxonomiesToolUI = makeAssistantToolUI<
  ListAssetTaxonomiesArgs,
  ListAssetTaxonomiesOutput
>({
  toolName: "list_asset_taxonomies",
  render: (props) => <ListAssetTaxonomiesContent {...props} />,
});

export const GetAssetTaxonomyAssignmentsToolUI = makeAssistantToolUI<
  GetAssetTaxonomyAssignmentsArgs,
  GetAssetTaxonomyAssignmentsOutput
>({
  toolName: "get_asset_taxonomy_assignments",
  render: (props) => <GetAssetTaxonomyAssignmentsContent {...props} />,
});
