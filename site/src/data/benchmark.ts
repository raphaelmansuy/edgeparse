export interface BenchmarkTool {
  name: string;
  nid: number;
  teds: number;
  mhs: number;
  overall: number;
  speedSeconds: number;
  isHighlight?: boolean;
}

export const benchmarkSnapshot = {
  lastUpdated: "2026-03-26",
  hardware: "Apple M4 Max",
  documentCount: 200,
  tools: [
    {
      name: "EdgeParse",
      nid: 0.889,
      teds: 0.596,
      mhs: 0.553,
      overall: 0.787,
      speedSeconds: 0.064,
      isHighlight: true,
    },
    {
      name: "Docling (IBM)",
      nid: 0.867,
      teds: 0.54,
      mhs: 0.438,
      overall: 0.745,
      speedSeconds: 0.768,
    },
    {
      name: "OpenDataLoader",
      nid: 0.873,
      teds: 0.326,
      mhs: 0.442,
      overall: 0.733,
      speedSeconds: 0.094,
    },
    {
      name: "PyMuPDF4LLM",
      nid: 0.852,
      teds: 0.323,
      mhs: 0.407,
      overall: 0.71,
      speedSeconds: 0.439,
    },
    {
      name: "LiteParse",
      nid: 0.815,
      teds: 0,
      mhs: 0.001,
      overall: 0.564,
      speedSeconds: 0.196,
    },
    {
      name: "MarkItDown",
      nid: 0.808,
      teds: 0.193,
      mhs: 0.001,
      overall: 0.564,
      speedSeconds: 0.149,
    },
  ] satisfies BenchmarkTool[],
};

export function formatSpeed(seconds: number): string {
  return `${seconds.toFixed(3)} s/doc`;
}

export function getBenchmarkTool(name: string): BenchmarkTool {
  const tool = benchmarkSnapshot.tools.find((entry) => entry.name === name);
  if (!tool) {
    throw new Error(`Unknown benchmark tool: ${name}`);
  }
  return tool;
}
