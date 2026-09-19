/** LSP Position / Range as returned by rainbow-lsp JSON. */
export type LspPosition = { line: number; character: number };
export type LspRange = { start: LspPosition; end: LspPosition };

/** Mirrors `rainbow/embeddedRegions` from rainbow-lsp. */
export type EmbeddedRegion = {
  language: string;
  languageId: "json" | "yaml" | "xml" | "html" | "markdown" | string;
  body: string;
  bodyRange: LspRange;
  isPlaceholder: boolean;
};

export type ServiceDiagnostic = {
  message: string;
  /** 1=Error, 2=Warning, 3=Info, 4=Hint (LSP DiagnosticSeverity). */
  severity: number;
  /** Range relative to the embedded body (0-based). */
  range: LspRange;
  source: string;
  code?: string | number;
};
