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
  lastUpdated: "2026-03-28",
  hardware: "Apple M4 Max",
  documentCount: 200,
  tools: [
    {
      name: "EdgeParse",
      nid: 0.8846,
      teds: 0.5590,
      mhs: 0.5540,
      overall: 0.7811,
      speedSeconds: 0.007,
      isHighlight: true,
    },
    {
      name: "Docling (IBM)",
      nid: 0.8665,
      teds: 0.5404,
      mhs: 0.4384,
      overall: 0.7452,
      speedSeconds: 0.584,
    },
    {
      name: "OpenDataLoader",
      nid: 0.8611,
      teds: 0.3234,
      mhs: 0.4360,
      overall: 0.7233,
      speedSeconds: 0.014,
    },
    {
      name: "PyMuPDF4LLM",
      nid: 0.8522,
      teds: 0.3233,
      mhs: 0.4066,
      overall: 0.7103,
      speedSeconds: 0.327,
    },
    {
      name: "LiteParse",
      nid: 0.8148,
      teds: 0.0000,
      mhs: 0.0012,
      overall: 0.5642,
      speedSeconds: 0.160,
    },
    {
      name: "MarkItDown",
      nid: 0.8075,
      teds: 0.1925,
      mhs: 0.0012,
      overall: 0.5639,
      speedSeconds: 0.123,
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
