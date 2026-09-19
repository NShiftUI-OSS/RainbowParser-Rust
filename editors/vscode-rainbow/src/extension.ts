import * as fs from "fs";
import * as path from "path";
import {
  ColorThemeKind,
  ConfigurationTarget,
  ExtensionContext,
  window,
  workspace,
} from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

import type { EmbeddedLanguageHost } from "./embedded/host";

let client: LanguageClient | undefined;
let embeddedHost: EmbeddedLanguageHost | undefined;

type TextMateRule = {
  name?: string;
  scope: string | string[];
  settings: { foreground?: string; fontStyle?: string };
};

function resolveServerPath(context: ExtensionContext): string {
  const configured = workspace
    .getConfiguration("rainbow")
    .get<string>("lsp.path")
    ?.trim();

  if (configured) {
    return configured;
  }

  const repoRootCandidates = [
    path.resolve(context.extensionPath, "../.."),
    path.resolve(context.extensionPath, "../../.."),
  ];

  for (const root of repoRootCandidates) {
    for (const profile of ["release", "debug"]) {
      const candidate = path.join(root, "target", profile, "rainbow-lsp");
      if (fs.existsSync(candidate)) {
        return candidate;
      }
    }
  }

  return "rainbow-lsp";
}

function loadRainbowTokenRules(
  extensionPath: string,
  kind: ColorThemeKind
): TextMateRule[] {
  const file =
    kind === ColorThemeKind.Light || kind === ColorThemeKind.HighContrastLight
      ? "rainbow-tokens-light.json"
      : "rainbow-tokens-dark.json";
  const fullPath = path.join(extensionPath, "themes", file);
  const raw = JSON.parse(fs.readFileSync(fullPath, "utf8")) as {
    textMateRules: TextMateRule[];
  };
  return raw.textMateRules ?? [];
}

function isRainbowRule(rule: TextMateRule): boolean {
  return typeof rule.name === "string" && rule.name.startsWith("Rainbow:");
}

/** Merge Rainbow TextMate colors into user tokenColorCustomizations (Global). */
async function applyRainbowTokenColors(
  context: ExtensionContext
): Promise<void> {
  const rainbowRules = loadRainbowTokenRules(
    context.extensionPath,
    window.activeColorTheme.kind
  );
  const config = workspace.getConfiguration("editor");
  const inspect = config.inspect<Record<string, unknown>>(
    "tokenColorCustomizations"
  );
  const current = {
    ...(inspect?.globalValue ?? {}),
  } as {
    textMateRules?: TextMateRule[];
    [key: string]: unknown;
  };

  const retained = (current.textMateRules ?? []).filter(
    (rule) => !isRainbowRule(rule)
  );

  await config.update(
    "tokenColorCustomizations",
    {
      ...current,
      textMateRules: [...retained, ...rainbowRules],
    },
    ConfigurationTarget.Global
  );
}

export async function activate(context: ExtensionContext): Promise<void> {
  const output = window.createOutputChannel("Rainbow Language Server");
  context.subscriptions.push(output);

  output.appendLine(`Activating Rainbow DSL extension…`);

  try {
    await applyRainbowTokenColors(context);
    output.appendLine("Applied Rainbow syntax token colors.");
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    output.appendLine(`Failed to apply Rainbow token colors: ${detail}`);
  }

  context.subscriptions.push(
    window.onDidChangeActiveColorTheme(() => {
      void applyRainbowTokenColors(context).catch((error) => {
        const detail = error instanceof Error ? error.message : String(error);
        output.appendLine(`Failed to refresh Rainbow token colors: ${detail}`);
      });
    })
  );

  const serverPath = resolveServerPath(context);
  output.appendLine(`Resolved rainbow-lsp path: ${serverPath}`);

  if (serverPath !== "rainbow-lsp" && !fs.existsSync(serverPath)) {
    const message = `rainbow-lsp not found at "${serverPath}". Format Document / diagnostics need the LSP — run \`brew reinstall rainbow && rainbow install cursor\`, or set rainbow.lsp.path.`;
    output.appendLine(message);
    void window.showWarningMessage(message);
    // Still try PATH fallback below via serverPath = "rainbow-lsp" if user fixed PATH later.
    // Do not return: attempt start with configured path so the failure is visible in the log.
  }

  const serverOptions: ServerOptions = {
    run: { command: serverPath, transport: TransportKind.stdio },
    debug: { command: serverPath, transport: TransportKind.stdio },
  };

  const registryPath = workspace
    .getConfiguration("rainbow")
    .get<string>("registryPath")
    ?.trim();

  // Filled after LSP starts so a bundled-service failure cannot block the formatter.
  const embeddedRef: { host?: EmbeddedLanguageHost } = {};

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "rainbow" },
      { scheme: "untitled", language: "rainbow" },
      { scheme: "file", language: "json", pattern: "**/*.registry.json" },
    ],
    synchronize: {
      configurationSection: "rainbow",
      fileEvents: workspace.createFileSystemWatcher("**/*.{rbw,registry.json}"),
    },
    initializationOptions: {
      registryPath: registryPath || "",
    },
    outputChannel: output,
    traceOutputChannel: output,
    middleware: {
      handleDiagnostics: (uri, diagnostics, next) => {
        const host = embeddedRef.host;
        if (host) {
          const middleware = host.middleware();
          if (middleware.handleDiagnostics) {
            middleware.handleDiagnostics(uri, diagnostics, next);
            return;
          }
        }
        next(uri, diagnostics);
      },
    },
  };

  client = new LanguageClient(
    "rainbowLsp",
    "Rainbow Language Server",
    serverOptions,
    clientOptions
  );

  try {
    await client.start();
    output.appendLine(
      "rainbow-lsp started successfully (Format Document / format on save enabled)."
    );
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    const message = `Failed to start rainbow-lsp: ${detail}. Format Document will not work until the LSP is available.`;
    output.appendLine(message);
    void window.showErrorMessage(message);
    client = undefined;
    return;
  }

  // Load bundled @LANG services after LSP so formatter never depends on them.
  try {
    const { EmbeddedLanguageHost } = await import("./embedded/host");
    embeddedHost = new EmbeddedLanguageHost(client, output);
    embeddedRef.host = embeddedHost;
    embeddedHost.register();
    context.subscriptions.push({ dispose: () => embeddedHost?.dispose() });
    output.appendLine(
      "Bundled embedded language services ready (JSON/YAML/XML/HTML/Markdown)."
    );
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    output.appendLine(
      `Bundled embedded language services failed to load (LSP formatter still works): ${detail}`
    );
  }
}

export async function deactivate(): Promise<void> {
  embeddedHost?.dispose();
  embeddedHost = undefined;
  if (client) {
    await client.stop();
    client = undefined;
  }
}
