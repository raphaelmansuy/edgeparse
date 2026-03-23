import type { ConvertOptions } from './types.js';

// Native addon — resolved at runtime via optional dep lookup
function loadNative(): {
  convert: (inputPath: string, options?: Record<string, unknown>) => string;
  version: () => string;
} {
  const platforms: Record<string, string> = {
    'linux-x64':    'edgeparse-linux-x64-gnu',
    'linux-arm64':  'edgeparse-linux-arm64-gnu',
    'darwin-x64':   'edgeparse-darwin-x64',
    'darwin-arm64': 'edgeparse-darwin-arm64',
    'win32-x64':    'edgeparse-win32-x64-msvc',
  };
  const key = `${process.platform}-${process.arch}`;
  const pkg = platforms[key];
  if (!pkg) throw new Error(`edgeparse: unsupported platform: ${key}`);
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  return require(pkg);
}

let native: ReturnType<typeof loadNative> | undefined;

function getNative() {
  if (!native) {
    native = loadNative();
  }
  return native;
}

/**
 * Convert a PDF file and return the extracted content as a string.
 *
 * @param inputPath - Path to the PDF file.
 * @param options   - Conversion options.
 * @returns The extracted content as a string.
 */
export function convert(
  inputPath: string,
  options?: ConvertOptions,
): string {
  const n = getNative();
  return n.convert(inputPath, options ? {
    format: options.format,
    pages: options.pages,
    password: options.password,
    reading_order: options.readingOrder,
    table_method: options.tableMethod,
    image_output: options.imageOutput,
  } : undefined);
}

/**
 * Return the edgeparse version string.
 */
export function version(): string {
  return getNative().version();
}

export type { ConvertOptions };
