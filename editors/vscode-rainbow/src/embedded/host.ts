import {
  CancellationToken,
  CompletionItem,
  CompletionItemKind,
  CompletionList,
  Diagnostic,
  DiagnosticSeverity,
  Hover,
  languages,
  MarkdownString,
  Position,
  TextDocument,
  workspace,
} from "vscode";
import type { LanguageClient, Middleware } from "vscode-languageclient/node";

import { mapAbsoluteToLocal, mapLocalRangeToAbsolute } from "./mapping";
import { BundledEmbeddedServices } from "./services";
import type { EmbeddedRegion } from "./types";

const METHOD = "rainbow/embeddedRegions";
const COLLECTION_NAME = "rainbow-embedded";

export class EmbeddedLanguageHost {
  private readonly services = new BundledEmbeddedServices();
  private readonly collection = languages.createDiagnosticCollection(COLLECTION_NAME);
  private readonly regionsByUri = new Map<string, EmbeddedRegion[]>();
  private refreshTimer: NodeJS.Timeout | undefined;
  private enabled = true;
  private readonly disposables: { dispose(): void }[] = [];

  constructor(
    private readonly client: LanguageClient,
    private readonly output: { appendLine(line: string): void }
  ) {}

  dispose(): void {
    if (this.refreshTimer) {
      clearTimeout(this.refreshTimer);
    }
    for (const disposable of this.disposables) {
      disposable.dispose();
    }
    this.collection.dispose();
  }

  /** Strip Rust `rainbow.embedded.*` diagnostics when bundled services own IDE validation. */
  middleware(): Middleware {
    return {
      handleDiagnostics: (uri, diagnostics, next) => {
        if (!this.enabled) {
          next(uri, diagnostics);
          return;
        }
        const filtered = diagnostics.filter((diagnostic) => {
          const code =
            typeof diagnostic.code === "object" &&
            diagnostic.code &&
            "value" in diagnostic.code
              ? String(diagnostic.code.value)
              : String(diagnostic.code ?? "");
          return !code.startsWith("rainbow.embedded.");
        });
        next(uri, filtered);
      },
    };
  }

  register(): void {
    this.enabled = workspace
      .getConfiguration("rainbow")
      .get<boolean>("embedded.bundledLanguageServices", true);

    this.disposables.push(
      workspace.onDidChangeConfiguration((event) => {
        if (event.affectsConfiguration("rainbow.embedded.bundledLanguageServices")) {
          this.enabled = workspace
            .getConfiguration("rainbow")
            .get<boolean>("embedded.bundledLanguageServices", true);
          void this.refreshOpenDocuments();
        }
      }),
      workspace.onDidOpenTextDocument((document) => {
        if (document.languageId === "rainbow") {
          this.scheduleRefresh(document);
        }
      }),
      workspace.onDidChangeTextDocument((event) => {
        if (event.document.languageId === "rainbow") {
          this.scheduleRefresh(event.document);
        }
      }),
      workspace.onDidCloseTextDocument((document) => {
        this.regionsByUri.delete(document.uri.toString());
        this.collection.delete(document.uri);
      }),
      languages.registerCompletionItemProvider(
        { language: "rainbow" },
        {
          provideCompletionItems: (document, position, token) =>
            this.provideCompletions(document, position, token),
        },
        ".",
        '"',
        "'",
        "<",
        "/",
        "#",
        ":",
        " "
      ),
      languages.registerHoverProvider(
        { language: "rainbow" },
        {
          provideHover: (document, position, token) =>
            this.provideHover(document, position, token),
        }
      )
    );

    void this.refreshOpenDocuments();
  }

  private async refreshOpenDocuments(): Promise<void> {
    for (const document of workspace.textDocuments) {
      if (document.languageId === "rainbow") {
        await this.refresh(document);
      }
    }
  }

  private scheduleRefresh(document: TextDocument): void {
    if (this.refreshTimer) {
      clearTimeout(this.refreshTimer);
    }
    this.refreshTimer = setTimeout(() => {
      void this.refresh(document);
    }, 200);
  }

  private async refresh(document: TextDocument): Promise<void> {
    if (!this.enabled) {
      this.collection.delete(document.uri);
      this.regionsByUri.delete(document.uri.toString());
      return;
    }

    try {
      const regions = await this.client.sendRequest<EmbeddedRegion[]>(METHOD, {
        textDocument: { uri: document.uri.toString() },
      });
      this.regionsByUri.set(document.uri.toString(), regions);

      const diagnostics: Diagnostic[] = [];
      for (const region of regions) {
        const serviceDiags = await this.services.validate(region);
        for (const diagnostic of serviceDiags) {
          const mapped = new Diagnostic(
            mapLocalRangeToAbsolute(document.getText(), region, diagnostic.range),
            diagnostic.message,
            mapSeverity(diagnostic.severity)
          );
          mapped.source = diagnostic.source;
          if (diagnostic.code !== undefined) {
            mapped.code = diagnostic.code;
          }
          diagnostics.push(mapped);
        }
      }
      this.collection.set(document.uri, diagnostics);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      this.output.appendLine(`embedded regions refresh failed: ${detail}`);
    }
  }

  private regionAt(
    document: TextDocument,
    position: Position
  ): EmbeddedRegion | undefined {
    const regions = this.regionsByUri.get(document.uri.toString()) ?? [];
    const source = document.getText();
    return regions.find(
      (region) => mapAbsoluteToLocal(source, region, position) !== undefined
    );
  }

  private async provideCompletions(
    document: TextDocument,
    position: Position,
    token: CancellationToken
  ): Promise<CompletionList | undefined> {
    if (!this.enabled || token.isCancellationRequested) {
      return undefined;
    }
    const region = this.regionAt(document, position);
    if (!region) {
      return undefined;
    }
    const local = mapAbsoluteToLocal(document.getText(), region, position);
    if (!local) {
      return undefined;
    }
    const items = await this.services.complete(region, local);
    return new CompletionList(
      items.map((item) => {
        const completion = new CompletionItem(
          item.label,
          (item.kind as CompletionItemKind | undefined) ?? CompletionItemKind.Text
        );
        completion.detail = item.detail;
        if (item.documentation) {
          completion.documentation = new MarkdownString(item.documentation);
        }
        completion.insertText = item.insertText;
        completion.filterText = item.filterText;
        completion.sortText = item.sortText;
        return completion;
      }),
      false
    );
  }

  private async provideHover(
    document: TextDocument,
    position: Position,
    token: CancellationToken
  ): Promise<Hover | undefined> {
    if (!this.enabled || token.isCancellationRequested) {
      return undefined;
    }
    const region = this.regionAt(document, position);
    if (!region) {
      return undefined;
    }
    const local = mapAbsoluteToLocal(document.getText(), region, position);
    if (!local) {
      return undefined;
    }
    const hover = await this.services.hover(region, local);
    if (!hover) {
      return undefined;
    }
    const range = hover.range
      ? mapLocalRangeToAbsolute(document.getText(), region, hover.range)
      : undefined;
    return new Hover(new MarkdownString(hover.contents), range);
  }
}

function mapSeverity(severity: number): DiagnosticSeverity {
  switch (severity) {
    case 1:
      return DiagnosticSeverity.Error;
    case 2:
      return DiagnosticSeverity.Warning;
    case 3:
      return DiagnosticSeverity.Information;
    case 4:
      return DiagnosticSeverity.Hint;
    default:
      return DiagnosticSeverity.Error;
  }
}
