export interface BenchmarkDefinition {
  name: string;
  query: string;
}

export interface BenchmarkResult {
  name: string;
  coldMs: number;
  warmMs: number;
}
