import { Position, Range } from "vscode";
import {
  offsetAt,
  offsetToPosition,
  positionAt,
  positionToOffset,
} from "./offsets";
import type { EmbeddedRegion, LspPosition, LspRange } from "./types";

export { offsetAt, positionAt } from "./offsets";

/** Absolute start offset of the region body in `source`. */
export function regionStartOffset(source: string, region: EmbeddedRegion): number {
  return positionToOffset(source, region.bodyRange.start);
}

export function mapLocalPositionToAbsolute(
  source: string,
  region: EmbeddedRegion,
  local: LspPosition
): Position {
  const absoluteOffset = regionStartOffset(source, region) + offsetAt(region.body, local);
  const lsp = offsetToPosition(source, absoluteOffset);
  return new Position(lsp.line, lsp.character);
}

export function mapLocalRangeToAbsolute(
  source: string,
  region: EmbeddedRegion,
  local: LspRange
): Range {
  return new Range(
    mapLocalPositionToAbsolute(source, region, local.start),
    mapLocalPositionToAbsolute(source, region, local.end)
  );
}

/** Absolute document position → local body position, or undefined if outside. */
export function mapAbsoluteToLocal(
  source: string,
  region: EmbeddedRegion,
  absolute: Position
): LspPosition | undefined {
  const absOffset = positionToOffset(source, {
    line: absolute.line,
    character: absolute.character,
  });
  const start = regionStartOffset(source, region);
  const end = start + region.body.length;
  if (absOffset < start || absOffset > end) {
    return undefined;
  }
  return positionAt(region.body, absOffset - start);
}

export function toVsRange(range: LspRange): Range {
  return new Range(
    new Position(range.start.line, range.start.character),
    new Position(range.end.line, range.end.character)
  );
}
