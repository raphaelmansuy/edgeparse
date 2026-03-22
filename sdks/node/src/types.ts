export interface ConvertOptions {
  /** Output format. Valid values: "markdown", "json", "html", "text". Default: "markdown". */
  format?: string;
  /** Page range string, e.g. "1,3,5-7". */
  pages?: string;
  /** Password for encrypted PDFs. */
  password?: string;
  /** Reading order algorithm: "xycut" (default) or "off". */
  readingOrder?: string;
  /** Table detection method: "default" or "cluster". */
  tableMethod?: string;
  /** Image output mode: "off" (default), "embedded", or "external". */
  imageOutput?: string;
}
