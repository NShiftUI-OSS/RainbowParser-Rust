import {
  getLanguageService as getHtmlLanguageService,
  type LanguageService as HtmlLanguageService,
} from "vscode-html-languageservice";
import {
  getLanguageService as getJsonLanguageService,
  type LanguageService as JsonLanguageService,
} from "vscode-json-languageservice";
import { TextDocument } from "vscode-languageserver-textdocument";
import {
  CompletionItem as LspCompletionItem,
  DiagnosticSeverity,
  Hover as LspHover,
  Position as LspPosition,
} from "vscode-languageserver-types";
import { getLanguageService as getYamlLanguageService } from "yaml-language-server";
import { XMLValidator } from "fast-xml-parser";
import { HTMLHint } from "htmlhint";
import { lint as markdownlintSync } from "markdownlint/sync";

import type { EmbeddedRegion, ServiceDiagnostic } from "./types";

const HTML_FRAGMENT_RULES: Record<string, unknown> = {
  "tagname-lowercase": true,
  "attr-lowercase": false,
  "attr-value-double-quotes": true,
  "doctype-first": false,
  "tag-pair": true,
  "spec-char-escape": true,
  "id-unique": true,
  "src-not-empty": true,
  "attr-no-duplication": true,
  "title-require": false,
  "alt-require": true,
  "doctype-html5": false,
  "style-disabled": false,
  "inline-style-disabled": false,
  "inline-script-disabled": false,
};

type YamlLanguageService = ReturnType<typeof getYamlLanguageService>;

export type EmbeddedCompletion = {
  label: string;
  kind?: number;
  detail?: string;
  documentation?: string;
  insertText?: string;
  insertTextFormat?: number;
  filterText?: string;
  sortText?: string;
};

export type EmbeddedHover = {
  contents: string;
  range?: { start: LspPosition; end: LspPosition };
};

/**
 * Bundled language services for `@LANG(...)` bodies.
 * Runs in-process inside the VS Code / Cursor extension host.
 */
export class BundledEmbeddedServices {
  private readonly json: JsonLanguageService;
  private readonly yaml: YamlLanguageService;
  private readonly html: HtmlLanguageService;
  private version = 1;

  constructor() {
    this.json = getJsonLanguageService({
      schemaRequestService: async () => "",
      workspaceContext: {
        resolveRelativePath: (relativePath: string) => relativePath,
      },
    });
    this.json.configure({
      validate: true,
      allowComments: false,
      schemas: [],
    });

    this.yaml = getYamlLanguageService({
      schemaRequestService: async () => "",
      workspaceContext: {
        resolveRelativePath: (relativePath: string) => relativePath,
      },
    });
    this.yaml.configure({
      validate: true,
      hover: true,
      completion: true,
      format: true,
      isKubernetes: false,
      schemas: [],
      customTags: [],
      yamlVersion: "1.2",
    } as Parameters<YamlLanguageService["configure"]>[0]);

    this.html = getHtmlLanguageService();
  }

  async validate(region: EmbeddedRegion): Promise<ServiceDiagnostic[]> {
    if (region.isPlaceholder) {
      return [];
    }

    switch (region.languageId) {
      case "json":
        return this.validateJson(region.body);
      case "yaml":
        return this.validateYaml(region.body);
      case "html":
        return this.validateHtml(region.body);
      case "markdown":
        return this.validateMarkdown(region.body);
      case "xml":
        return this.validateXml(region.body);
      default:
        return [];
    }
  }

  async complete(
    region: EmbeddedRegion,
    position: LspPosition
  ): Promise<EmbeddedCompletion[]> {
    if (region.isPlaceholder) {
      return [];
    }

    const doc = this.documentFor(region);
    switch (region.languageId) {
      case "json": {
        const parsed = this.json.parseJSONDocument(doc);
        const list = await this.json.doComplete(doc, position, parsed);
        return (list?.items ?? []).map(mapCompletion);
      }
      case "yaml": {
        const list = await this.yaml.doComplete(doc, position, false);
        return (list?.items ?? []).map(mapCompletion);
      }
      case "html": {
        const parsed = this.html.parseHTMLDocument(doc);
        const list = this.html.doComplete(doc, position, parsed);
        return (list?.items ?? []).map(mapCompletion);
      }
      case "markdown":
        return markdownCompletions(region.body, position);
      case "xml":
        return xmlCompletions(region.body, position);
      default:
        return [];
    }
  }

  async hover(
    region: EmbeddedRegion,
    position: LspPosition
  ): Promise<EmbeddedHover | undefined> {
    if (region.isPlaceholder) {
      return {
        contents: `Template hole for \`@${region.language}(...)\` — expand before language validation.`,
      };
    }

    const doc = this.documentFor(region);
    switch (region.languageId) {
      case "json": {
        const parsed = this.json.parseJSONDocument(doc);
        const hover = await this.json.doHover(doc, position, parsed);
        return mapHover(hover);
      }
      case "yaml": {
        const hover = await this.yaml.doHover(doc, position);
        return mapHover(hover);
      }
      case "html": {
        const parsed = this.html.parseHTMLDocument(doc);
        const hover = this.html.doHover(doc, position, parsed);
        return mapHover(hover);
      }
      case "markdown":
        return {
          contents: "Embedded **Markdown** body (`@MARKDOWN(...)`).",
        };
      case "xml":
        return {
          contents: "Embedded **XML** body (`@XML(...)`).",
        };
      default:
        return undefined;
    }
  }

  private documentFor(region: EmbeddedRegion): TextDocument {
    this.version += 1;
    return TextDocument.create(
      `rainbow-embedded://${region.languageId}/${this.version}`,
      region.languageId,
      this.version,
      region.body
    );
  }

  private async validateJson(body: string): Promise<ServiceDiagnostic[]> {
    const doc = TextDocument.create("rainbow-embedded://doc.json", "json", 1, body);
    const parsed = this.json.parseJSONDocument(doc);
    const diagnostics = await this.json.doValidation(doc, parsed);
    return diagnostics.map((diagnostic) => ({
      message:
        typeof diagnostic.message === "string"
          ? diagnostic.message
          : diagnostic.message.value,
      severity: diagnostic.severity ?? DiagnosticSeverity.Error,
      range: {
        start: diagnostic.range.start,
        end: diagnostic.range.end,
      },
      source: "rainbow-embedded-json",
      code: diagnostic.code as string | number | undefined,
    }));
  }

  private async validateYaml(body: string): Promise<ServiceDiagnostic[]> {
    const doc = TextDocument.create("rainbow-embedded://doc.yaml", "yaml", 1, body);
    const diagnostics = await this.yaml.doValidation(doc, false);
    return diagnostics.map((diagnostic) => ({
      message: diagnostic.message,
      severity: diagnostic.severity ?? DiagnosticSeverity.Error,
      range: {
        start: diagnostic.range.start,
        end: diagnostic.range.end,
      },
      source: "rainbow-embedded-yaml",
      code: diagnostic.code as string | number | undefined,
    }));
  }

  private validateHtml(body: string): ServiceDiagnostic[] {
    const list = HTMLHint.verify(body, HTML_FRAGMENT_RULES);
    return list.map((message) => {
      const line = Math.max(0, (message.line ?? 1) - 1);
      const character = Math.max(0, (message.col ?? 1) - 1);
      return {
        message: message.message,
        severity:
          message.type === "warning"
            ? DiagnosticSeverity.Warning
            : DiagnosticSeverity.Error,
        range: {
          start: { line, character },
          end: { line, character: character + 1 },
        },
        source: "rainbow-embedded-html",
        code: message.rule?.id,
      };
    });
  }

  private validateMarkdown(body: string): ServiceDiagnostic[] {
    const result = markdownlintSync({
      strings: { content: body },
      config: {
        default: true,
        MD013: false, // line length — noisy inside Rainbow params
        MD041: false, // first line heading — fragments are ok
        MD047: false, // trailing newline — fragments are ok
      },
    });
    const findings = result.content ?? [];
    return findings.map((finding) => {
      const line = Math.max(0, finding.lineNumber - 1);
      return {
        message: `${finding.ruleNames[0]}: ${finding.ruleDescription}`,
        severity:
          finding.severity === "error"
            ? DiagnosticSeverity.Error
            : DiagnosticSeverity.Warning,
        range: {
          start: { line, character: 0 },
          end: { line, character: 1 },
        },
        source: "rainbow-embedded-markdown",
        code: finding.ruleNames[0],
      };
    });
  }

  private validateXml(body: string): ServiceDiagnostic[] {
    const result = XMLValidator.validate(body, {
      allowBooleanAttributes: true,
    });
    if (result === true) {
      return [];
    }
    const line = Math.max(0, (result.err?.line ?? 1) - 1);
    const character = Math.max(0, (result.err?.col ?? 1) - 1);
    return [
      {
        message: result.err?.msg ?? "Invalid XML",
        severity: DiagnosticSeverity.Error,
        range: {
          start: { line, character },
          end: { line, character: character + 1 },
        },
        source: "rainbow-embedded-xml",
        code: result.err?.code,
      },
    ];
  }
}

function mapCompletion(item: LspCompletionItem): EmbeddedCompletion {
  const documentation =
    typeof item.documentation === "string"
      ? item.documentation
      : item.documentation?.value;
  return {
    label: item.label,
    kind: item.kind,
    detail: item.detail,
    documentation,
    insertText:
      typeof item.insertText === "string"
        ? item.insertText
        : item.textEdit && "newText" in item.textEdit
          ? item.textEdit.newText
          : undefined,
    insertTextFormat: item.insertTextFormat,
    filterText: item.filterText,
    sortText: item.sortText,
  };
}

function mapHover(hover: LspHover | null | undefined): EmbeddedHover | undefined {
  if (!hover) {
    return undefined;
  }
  const contents = markupToString(hover.contents);
  if (!contents) {
    return undefined;
  }
  return {
    contents,
    range: hover.range
      ? {
          start: hover.range.start,
          end: hover.range.end,
        }
      : undefined,
  };
}

function markupToString(
  contents: LspHover["contents"]
): string | undefined {
  if (typeof contents === "string") {
    return contents;
  }
  if (Array.isArray(contents)) {
    return contents
      .map((part) => (typeof part === "string" ? part : part.value))
      .join("\n\n");
  }
  if (contents && typeof contents === "object" && "value" in contents) {
    return contents.value;
  }
  return undefined;
}

function markdownCompletions(body: string, position: LspPosition): EmbeddedCompletion[] {
  void body;
  void position;
  return [
    {
      label: "# Heading",
      insertText: "# ${1:Title}",
      insertTextFormat: 2,
      detail: "Markdown heading",
    },
    {
      label: "**bold**",
      insertText: "**${1:text}**",
      insertTextFormat: 2,
      detail: "Bold emphasis",
    },
    {
      label: "[link](url)",
      insertText: "[${1:text}](${2:url})",
      insertTextFormat: 2,
      detail: "Markdown link",
    },
    {
      label: "```code fence",
      insertText: "```${1:lang}\n${2}\n```",
      insertTextFormat: 2,
      detail: "Fenced code block",
    },
  ];
}

function xmlCompletions(body: string, position: LspPosition): EmbeddedCompletion[] {
  void body;
  void position;
  return [
    {
      label: "<element></element>",
      insertText: "<${1:element}>${2}</${1:element}>",
      insertTextFormat: 2,
      detail: "XML element",
    },
    {
      label: "<element />",
      insertText: "<${1:element} />",
      insertTextFormat: 2,
      detail: "Self-closing XML element",
    },
    {
      label: "<!-- comment -->",
      insertText: "<!-- ${1:comment} -->",
      insertTextFormat: 2,
      detail: "XML comment",
    },
  ];
}
