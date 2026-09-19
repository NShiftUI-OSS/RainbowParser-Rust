declare module "htmlhint" {
  export type Hint = {
    type?: string;
    message: string;
    line: number;
    col: number;
    rule?: { id?: string };
  };

  export const HTMLHint: {
    verify(code: string, ruleset?: Record<string, unknown>): Hint[];
    defaultRuleset?: Record<string, unknown>;
  };
}

declare module "markdownlint/sync" {
  export type LintFinding = {
    lineNumber: number;
    ruleNames: string[];
    ruleDescription: string;
    severity?: string;
  };

  export function lint(options: {
    strings: Record<string, string>;
    config?: Record<string, unknown>;
  }): Record<string, LintFinding[]>;
}
