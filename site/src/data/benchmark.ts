export interface BenchmarkTool {
  name: string;
  nid: number;
  teds: number;
  mhs: number;
  overall: number;
  /** Seconds per document; omit when the board does not report speed. */
  speedSeconds?: number;
  isHighlight?: boolean;
  status?: "finished" | "partial" | "skipped";
}

export type BenchmarkMetric = "nid" | "teds" | "mhs" | "overall";

export interface BenchmarkBoard {
  lastUpdated: string;
  hardware: string;
  documentCount: number;
  boardName: string;
  formula: string;
  tools: BenchmarkTool[];
}

/** EdgeParse-native harness (`benchmark/`) — product regression board. */
export const benchmarkSnapshot: BenchmarkBoard = {
  lastUpdated: "2026-10-01",
  hardware: "Apple M4 Max",
  documentCount: 200,
  boardName: "EdgeParse harness",
  formula: "product overall (quality + structure metrics)",
  tools: [
    {
      name: "EdgeParse",
      nid: 0.8855,
      teds: 0.5635,
      mhs: 0.5522,
      overall: 0.7817,
      speedSeconds: 0.019,
      isHighlight: true,
      status: "finished",
    },
    {
      name: "EdgeParse [hybrid]",
      nid: 0.8629,
      teds: 0.6030,
      mhs: 0.5062,
      overall: 0.7666,
      speedSeconds: 0.81,
      status: "finished",
    },
    {
      name: "Docling (IBM)",
      nid: 0.8747,
      teds: 0.5677,
      mhs: 0.4504,
      overall: 0.7579,
      speedSeconds: 2.332,
      status: "finished",
    },
    {
      name: "OpenDataLoader",
      nid: 0.8728,
      teds: 0.3199,
      mhs: 0.4413,
      overall: 0.7327,
      speedSeconds: 0.022,
      status: "finished",
    },
    {
      name: "PyMuPDF4LLM",
      nid: 0.8599,
      teds: 0.5087,
      mhs: 0.4109,
      overall: 0.7318,
      speedSeconds: 0.64,
      status: "finished",
    },
    {
      name: "OpenDataLoader [hybrid]",
      nid: 0.8694,
      teds: 0.4223,
      mhs: 0.4112,
      overall: 0.7311,
      speedSeconds: 2.537,
      status: "finished",
    },
    {
      name: "MarkItDown",
      nid: 0.8075,
      teds: 0.1925,
      mhs: 0.0012,
      overall: 0.5639,
      speedSeconds: 0.189,
      status: "finished",
    },
  ] satisfies BenchmarkTool[],
};

/**
 * Official OpenDataLoader board (`odl-bench/`) — matches public
 * opendataloader-bench formula: overall = mean(NID, TEDS, MHS).
 */
export const odlBenchSnapshot: BenchmarkBoard = {
  lastUpdated: "2026-10-01",
  hardware: "Apple M4 Max",
  documentCount: 200,
  boardName: "Official OpenDataLoader board (odl-bench)",
  formula: "overall = mean(NID, TEDS, MHS) with nulls omitted",
  tools: [
    {
      name: "EdgeParse [hybrid]",
      nid: 0.917,
      teds: 0.928,
      mhs: 0.836,
      overall: 0.9,
      speedSeconds: 4.282,
      isHighlight: true,
      status: "finished",
    },
    {
      name: "Docling",
      nid: 0.905,
      teds: 0.925,
      mhs: 0.827,
      overall: 0.891,
      speedSeconds: 2.332,
      status: "finished",
    },
    {
      name: "OpenDataLoader [hybrid]",
      nid: 0.915,
      teds: 0.676,
      mhs: 0.822,
      overall: 0.877,
      speedSeconds: 2.537,
      status: "finished",
    },
    {
      name: "PyMuPDF4LLM",
      nid: 0.916,
      teds: 0.803,
      mhs: 0.772,
      overall: 0.873,
      speedSeconds: 0.64,
      status: "finished",
    },
    {
      name: "EdgeParse",
      nid: 0.894,
      teds: 0.773,
      mhs: 0.763,
      overall: 0.857,
      speedSeconds: 2.712,
      status: "finished",
    },
    {
      name: "EdgeParse [wasm]",
      nid: 0.891,
      teds: 0.776,
      mhs: 0.761,
      overall: 0.854,
      status: "finished",
    },
    {
      name: "OpenDataLoader",
      nid: 0.912,
      teds: 0.483,
      mhs: 0.757,
      overall: 0.842,
      speedSeconds: 0.022,
      status: "finished",
    },
    {
      name: "MarkItDown",
      nid: 0.844,
      teds: 0.273,
      mhs: 0.0,
      overall: 0.589,
      speedSeconds: 0.189,
      status: "finished",
    },
    {
      name: "Unstructured",
      nid: 0.093,
      teds: 0.0,
      mhs: 0.054,
      overall: 0.086,
      speedSeconds: 3.407,
      status: "partial",
    },
  ] satisfies BenchmarkTool[],
};

export function formatSpeed(seconds: number): string {
  return `${seconds.toFixed(3)} s/doc`;
}

export function getBenchmarkTool(
  name: string,
  board: BenchmarkBoard = benchmarkSnapshot,
): BenchmarkTool {
  const tool = board.tools.find((entry) => entry.name === name);
  if (!tool) {
    throw new Error(`Unknown benchmark tool: ${name}`);
  }
  return tool;
}

/** How many times faster EdgeParse is vs `name` on the given board (default: harness). */
export function speedMultipleVs(
  name: string,
  board: BenchmarkBoard = benchmarkSnapshot,
  baselineName = "EdgeParse",
): number {
  const baseline = getBenchmarkTool(baselineName, board);
  const other = getBenchmarkTool(name, board);
  if (baseline.speedSeconds == null || other.speedSeconds == null) {
    throw new Error(`Missing speed for ${baselineName} or ${name}`);
  }
  if (baseline.speedSeconds <= 0) {
    throw new Error(`Invalid baseline speed for ${baselineName}`);
  }
  return other.speedSeconds / baseline.speedSeconds;
}

/** Format a speed multiple for marketing copy (e.g. "123×"). */
export function formatSpeedMultiple(multiple: number): string {
  if (multiple >= 10) {
    return `${Math.round(multiple)}×`;
  }
  if (multiple >= 2) {
    return `${multiple.toFixed(0)}×`;
  }
  return `${multiple.toFixed(1)}×`;
}

/** Tool with the highest score for `metric` among finished (or all) entries. */
export function leaderFor(
  metric: BenchmarkMetric,
  board: BenchmarkBoard = benchmarkSnapshot,
  opts: { finishedOnly?: boolean } = { finishedOnly: true },
): BenchmarkTool {
  const pool = board.tools.filter((t) =>
    opts.finishedOnly === false ? true : t.status !== "partial" && t.status !== "skipped",
  );
  if (pool.length === 0) {
    throw new Error("No tools available for leaderFor");
  }
  return pool.reduce((best, tool) => (tool[metric] > best[metric] ? tool : best));
}

const edge = getBenchmarkTool("EdgeParse");
const odlHybrid = getBenchmarkTool("EdgeParse [hybrid]", odlBenchSnapshot);
const odlDet = getBenchmarkTool("EdgeParse", odlBenchSnapshot);
const odlWasm = getBenchmarkTool("EdgeParse [wasm]", odlBenchSnapshot);

const vsDocling = speedMultipleVs("Docling (IBM)");
const vsOdl = speedMultipleVs("OpenDataLoader");
const vsPymu = speedMultipleVs("PyMuPDF4LLM");

/** Pre-formatted claims — pages must import these instead of hard-coding numbers. */
export const claims = {
  version: "0.3.0",
  copy: {
    noMlStack:
      "No ML stack required for born-digital PDFs; optional in-browser OCR for image tables",
  },
  harness: {
    boardName: benchmarkSnapshot.boardName,
    lastUpdated: benchmarkSnapshot.lastUpdated,
    hardware: benchmarkSnapshot.hardware,
    documentCount: benchmarkSnapshot.documentCount,
    overall: edge.overall.toFixed(3),
    overallShort: edge.overall.toFixed(2),
    nid: edge.nid.toFixed(3),
    teds: edge.teds.toFixed(3),
    mhs: edge.mhs.toFixed(3),
    speedPerDoc: formatSpeed(edge.speedSeconds!),
    speedSeconds: edge.speedSeconds!,
    docsPerSec: Math.round(1 / edge.speedSeconds!),
    accuracyPercent: Math.round(edge.overall * 100),
    vsDocling: formatSpeedMultiple(vsDocling),
    vsDoclingRaw: vsDocling,
    vsOpenDataLoader: formatSpeedMultiple(vsOdl),
    vsOpenDataLoaderRaw: vsOdl,
    vsPyMuPDF4LLM: formatSpeedMultiple(vsPymu),
    vsPyMuPDF4LLMRaw: vsPymu,
    leaderOverall: leaderFor("overall").name,
  },
  odl: {
    boardName: odlBenchSnapshot.boardName,
    lastUpdated: odlBenchSnapshot.lastUpdated,
    hardware: odlBenchSnapshot.hardware,
    documentCount: odlBenchSnapshot.documentCount,
    formula: odlBenchSnapshot.formula,
    hybridOverall: odlHybrid.overall.toFixed(3),
    hybridOverallShort: "0.900",
    hybridNid: odlHybrid.nid.toFixed(3),
    hybridTeds: odlHybrid.teds.toFixed(3),
    hybridMhs: odlHybrid.mhs.toFixed(3),
    deterministicOverall: odlDet.overall.toFixed(3),
    wasmOverall: odlWasm.overall.toFixed(3),
    wasmTeds: odlWasm.teds.toFixed(3),
    leaderOverall: leaderFor("overall", odlBenchSnapshot).name,
    leaderNid: leaderFor("nid", odlBenchSnapshot).name,
    leaderTeds: leaderFor("teds", odlBenchSnapshot).name,
    leaderMhs: leaderFor("mhs", odlBenchSnapshot).name,
    leadsAllMetrics:
      leaderFor("nid", odlBenchSnapshot).name === "EdgeParse [hybrid]" &&
      leaderFor("teds", odlBenchSnapshot).name === "EdgeParse [hybrid]" &&
      leaderFor("mhs", odlBenchSnapshot).name === "EdgeParse [hybrid]" &&
      leaderFor("overall", odlBenchSnapshot).name === "EdgeParse [hybrid]",
  },
} as const;
