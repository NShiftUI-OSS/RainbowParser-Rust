import type { LspPosition } from "./types";

/** Character offset of a 0-based position inside `body`. */
export function offsetAt(body: string, position: LspPosition): number {
  if (position.line < 0) {
    return 0;
  }
  const lines = splitLines(body);
  if (position.line >= lines.length) {
    return body.length;
  }
  let offset = 0;
  for (let i = 0; i < position.line; i++) {
    offset += lines[i].length;
  }
  const lineText = lines[position.line].replace(/\r?\n$/, "");
  return offset + Math.min(position.character, lineText.length);
}

export function positionAt(body: string, offset: number): LspPosition {
  const clamped = Math.max(0, Math.min(offset, body.length));
  let line = 0;
  let character = 0;
  for (let i = 0; i < clamped; i++) {
    if (body[i] === "\n") {
      line += 1;
      character = 0;
    } else {
      character += 1;
    }
  }
  return { line, character };
}

export function splitLines(text: string): string[] {
  if (text.length === 0) {
    return [""];
  }
  const lines: string[] = [];
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    if (text[i] === "\n") {
      lines.push(text.slice(start, i + 1));
      start = i + 1;
    }
  }
  lines.push(text.slice(start));
  return lines;
}

export function offsetToPosition(source: string, offset: number): LspPosition {
  return positionAt(source, offset);
}

export function positionToOffset(source: string, position: LspPosition): number {
  return offsetAt(source, position);
}
