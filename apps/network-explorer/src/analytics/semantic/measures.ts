//-------------------------------------
// Semantic Model — Aggregation Capability Model
//
// Governs which aggregate functions are valid for which types of columns.
// Moves aggregation metadata out of components and configurations.
//-------------------------------------

import type { AggregationFn, ColumnKind } from '../registry/types';

/**
 * Reusable aggregation capability model.
 * All columns support count operations. Measures support mathematical summaries.
 *
 * @param kind - The classified role of the column
 * @returns    - Array of valid AggregationFn options
 */
export function getAggregationCapabilities(kind: ColumnKind): AggregationFn[] {
  const base: AggregationFn[] = ['none', 'COUNT', 'COUNT_DISTINCT'];

  if (kind === 'measure') {
    return ['none', 'COUNT', 'COUNT_DISTINCT', 'SUM', 'AVG', 'MIN', 'MAX'];
  }

  return base;
}

/** Formatted label for display in the builder UI. */
export const AGG_LABELS: Record<AggregationFn, string> = {
  none: 'None (detail value)',
  COUNT: 'COUNT (Total rows)',
  COUNT_DISTINCT: 'COUNT DISTINCT (Unique values)',
  SUM: 'SUM (Total sum)',
  AVG: 'AVG (Average)',
  MIN: 'MIN (Minimum)',
  MAX: 'MAX (Maximum)',
};
