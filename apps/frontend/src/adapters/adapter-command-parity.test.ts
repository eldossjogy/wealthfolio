import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it, vi } from "vitest";

import { COMMANDS, invoke } from "./web/core";

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const frontendSrcDir = path.resolve(currentDir, "..");
const repoRoot = path.resolve(currentDir, "../../../..");

const INVOKE_COMMAND_RE = /invoke(?:<[^>]+>)?\(\s*['"`]([a-zA-Z0-9_]+)['"`]/g;
const TAURI_REGISTERED_COMMAND_RE = /commands::[a-z_]+::([a-zA-Z0-9_]+)/g;

afterEach(() => {
  vi.unstubAllGlobals();
});

function collectSourceFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      return collectSourceFiles(entryPath);
    }
    return /\.(ts|tsx)$/.test(entry.name) && !entry.name.endsWith(".test.ts") ? [entryPath] : [];
  });
}

/**
 * Collect all `features/*\/adapters/*.ts` files. Feature-local adapters call
 * `invoke(...)` and must round-trip through both the Tauri command registry
 * and the web COMMANDS dispatch.
 */
function collectFeatureAdapterFiles(): string[] {
  const featuresDir = path.join(frontendSrcDir, "features");
  const featureEntries = readdirSync(featuresDir, { withFileTypes: true });
  return featureEntries.flatMap((feature) => {
    if (!feature.isDirectory()) return [];
    const adaptersDir = path.join(featuresDir, feature.name, "adapters");
    try {
      return collectSourceFiles(adaptersDir);
    } catch {
      return [];
    }
  });
}

function collectInvokedCommands(files: string[]): Map<string, string[]> {
  const commands = new Map<string, string[]>();

  for (const file of files) {
    const source = readFileSync(file, "utf8");
    for (const match of source.matchAll(INVOKE_COMMAND_RE)) {
      const command = match[1];
      const relativePath = path.relative(repoRoot, file);
      const existingFiles = commands.get(command) ?? [];
      if (!existingFiles.includes(relativePath)) {
        existingFiles.push(relativePath);
      }
      commands.set(command, existingFiles);
    }
  }

  return commands;
}

function collectRegisteredTauriCommands(): Set<string> {
  const source = readFileSync(path.join(repoRoot, "apps/tauri/src/lib.rs"), "utf8");
  return new Set([...source.matchAll(TAURI_REGISTERED_COMMAND_RE)].map((match) => match[1]));
}

describe("adapter command parity", () => {
  it("registers every command reachable from the web adapter", () => {
    const files = [
      ...collectSourceFiles(path.join(frontendSrcDir, "adapters/shared")),
      ...collectSourceFiles(path.join(frontendSrcDir, "adapters/web")),
      ...collectFeatureAdapterFiles(),
    ];
    const invokedCommands = collectInvokedCommands(files);

    const missing = [...invokedCommands.entries()]
      .filter(([command]) => COMMANDS[command] === undefined)
      .map(([command, files]) => `${command}: ${files.join(", ")}`)
      .sort();

    expect(missing).toEqual([]);
  });

  it("registers every command reachable from the Tauri adapter", () => {
    const files = [
      ...collectSourceFiles(path.join(frontendSrcDir, "adapters/shared")),
      ...collectSourceFiles(path.join(frontendSrcDir, "adapters/tauri")),
      ...collectFeatureAdapterFiles(),
    ];
    const invokedCommands = collectInvokedCommands(files);
    const registeredCommands = collectRegisteredTauriCommands();

    const missing = [...invokedCommands.entries()]
      .filter(([command]) => !registeredCommands.has(command))
      .map(([command, files]) => `${command}: ${files.join(", ")}`)
      .sort();

    expect(missing).toEqual([]);
  });

  it("routes allocation drilldown requests with all required filters", async () => {
    const response = new Response(JSON.stringify({ holdings: [] }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(response);
    vi.stubGlobal("fetch", fetchMock);

    await invoke("get_holdings_by_allocation", {
      accountId: "PORTFOLIO",
      taxonomyId: "asset_classes",
      categoryId: "EQUITY",
    });

    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe(
      "/api/v1/allocations/holdings?accountId=PORTFOLIO&taxonomyId=asset_classes&categoryId=EQUITY",
    );
    expect(init.method).toBe("GET");
  });
});
