/** Shared types for the EdgeParse demo app. */

export interface BBox {
  page_number: number | null;
  last_page_number?: number | null;
  left_x: number;
  bottom_y: number;
  right_x: number;
  top_y: number;
}

/**
 * A content element as serialized by serde's externally-tagged enum.
 * Each variant is an object with a single key (the variant name) containing
 * the variant data. Examples:
 *   { "Paragraph": { "base": { "bbox": {...}, ... } } }
 *   { "Heading": { "base": { "base": { "bbox": {...} } }, "heading_level": 2 } }
 *   { "Image": { "bbox": {...}, "index": 3 } }
 *   { "TextBlock": { "bbox": {...}, ... } }
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type ContentElement = Record<string, any>;

/** Flattened element for overlay rendering. */
export interface FlatElement {
  type: string;
  bbox: BBox;
  index?: number | null;
  element: ContentElement;
}

export interface PdfDocument {
  file_name: string;
  number_of_pages: number;
  author?: string;
  title?: string;
  kids: ContentElement[];
}

export type OutputFormat = 'json' | 'markdown' | 'html' | 'text';
export type WasmStatus = 'idle' | 'loading' | 'ready' | 'error';
export type ParseStatus = 'idle' | 'parsing' | 'done' | 'error';

export interface PageDimensions {
  pdfWidth: number;
  pdfHeight: number;
  canvasWidth: number;
  canvasHeight: number;
  scale: number;
}

export type SemanticType =
  | 'TextBlock'
  | 'Heading'
  | 'Table'
  | 'TableBorder'
  | 'Figure'
  | 'Image'
  | 'Line'
  | 'LineArt'
  | 'List'
  | 'Other';
