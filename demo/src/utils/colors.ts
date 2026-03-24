/** Semantic type to color mapping for bounding box overlays. */

import type { ContentElement, BBox, FlatElement, SemanticType } from '../types';

export const SEMANTIC_COLORS: Record<SemanticType, string> = {
  TextBlock: 'rgba(59, 130, 246, 0.30)',    // blue
  Heading: 'rgba(168, 85, 247, 0.35)',      // purple
  Table: 'rgba(34, 197, 94, 0.30)',         // green
  TableBorder: 'rgba(16, 185, 129, 0.25)',  // emerald
  Figure: 'rgba(249, 115, 22, 0.30)',       // orange
  Image: 'rgba(236, 72, 153, 0.30)',        // pink
  Line: 'rgba(107, 114, 128, 0.20)',        // gray
  LineArt: 'rgba(156, 163, 175, 0.20)',     // light gray
  List: 'rgba(14, 165, 233, 0.30)',         // sky
  Other: 'rgba(161, 161, 170, 0.25)',       // zinc
};

export const SEMANTIC_BORDER_COLORS: Record<SemanticType, string> = {
  TextBlock: 'rgba(59, 130, 246, 0.70)',
  Heading: 'rgba(168, 85, 247, 0.80)',
  Table: 'rgba(34, 197, 94, 0.70)',
  TableBorder: 'rgba(16, 185, 129, 0.60)',
  Figure: 'rgba(249, 115, 22, 0.70)',
  Image: 'rgba(236, 72, 153, 0.70)',
  Line: 'rgba(107, 114, 128, 0.50)',
  LineArt: 'rgba(156, 163, 175, 0.50)',
  List: 'rgba(14, 165, 233, 0.70)',
  Other: 'rgba(161, 161, 170, 0.60)',
};

/** Map serde variant name to SemanticType for overlay coloring. */
const VARIANT_TO_SEMANTIC: Record<string, SemanticType> = {
  Paragraph: 'TextBlock',
  TextBlock: 'TextBlock',
  TextChunk: 'TextBlock',
  TextLine: 'TextBlock',
  Heading: 'Heading',
  NumberHeading: 'Heading',
  Table: 'Table',
  TableBorder: 'TableBorder',
  Figure: 'Figure',
  Image: 'Image',
  Line: 'Line',
  LineArt: 'LineArt',
  List: 'List',
  Caption: 'TextBlock',
  HeaderFooter: 'Other',
  Formula: 'Other',
  Picture: 'Image',
};

export function getSemanticType(variantName: string): SemanticType {
  return VARIANT_TO_SEMANTIC[variantName] ?? 'Other';
}

/**
 * Recursively find the `bbox` field in a serde-serialized variant.
 * Handles structures like:
 *   { bbox: {...} }                               — Image, Figure, Table, etc.
 *   { base: { bbox: {...} } }                     — Paragraph, Caption
 *   { base: { base: { bbox: {...} } } }           — Heading
 *   { base: { base: { base: { bbox: {...} } } } } — NumberHeading
 */
function findBbox(data: Record<string, unknown>): BBox | null {
  if (data.bbox && typeof data.bbox === 'object') {
    return data.bbox as BBox;
  }
  if (data.base && typeof data.base === 'object') {
    return findBbox(data.base as Record<string, unknown>);
  }
  return null;
}

/** Find the index field similarly. */
function findIndex(data: Record<string, unknown>): number | null {
  if (data.index != null) return data.index as number;
  if (data.base && typeof data.base === 'object') {
    return findIndex(data.base as Record<string, unknown>);
  }
  return null;
}

/**
 * Flatten a serde-tagged enum ContentElement into a FlatElement
 * for use by the overlay engine.
 */
export function flattenElement(el: ContentElement): FlatElement | null {
  const keys = Object.keys(el);
  if (keys.length === 0) return null;
  const variantName = keys[0];
  const data = el[variantName];
  if (!data || typeof data !== 'object') return null;

  const bbox = findBbox(data as Record<string, unknown>);
  if (!bbox) return null;

  return {
    type: variantName,
    bbox,
    index: findIndex(data as Record<string, unknown>),
    element: el,
  };
}
