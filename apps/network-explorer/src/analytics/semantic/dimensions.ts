//-------------------------------------
// Semantic Model — Dimensions & Classifications
//
// Classifies DuckDB database columns into semantic roles (dimension, measure, geo, id)
// using name-based patterns and SQL data type heuristics.
//-------------------------------------

import type { ColumnKind } from '../registry/types';

/** SQL data type tokens representing numeric values (measures). */
const MEASURE_TYPE_PATTERNS = [
  'INT', // INTEGER, BIGINT, HUGEINT, SMALLINT, TINYINT, UBIGINT, UINTEGER ...
  'DOUBLE',
  'FLOAT',
  'DECIMAL',
  'NUMERIC',
  'REAL',
];

/** SQL data type tokens representing textual/categorical columns (dimensions). */
const DIMENSION_TYPE_PATTERNS = [
  'VARCHAR',
  'CHAR',
  'TEXT',
  'STRING',
  'ENUM',
  'BOOL',
  'DATE',
  'TIME',
  'INTERVAL',
];

/**
 * Geospatial regex pattern. Matches lat/lon variant names isolated by word/underscore.
 * Correctly classifies stop_lat, stop_lon, longitude, lat, etc.
 */
const GEO_NAME_PATTERN = /(^|_)(lat|lon|lng|latitude|longitude)(_|$)/i;

/** ID pattern. Matches surrogate/foreign key numeric columns (e.g. stop_id, route_id). */
const ID_NAME_PATTERN = /_id$/i;

/**
 * Semantic column classifier.
 *
 * @param columnName - Column name
 * @param dataType   - SQL data type string (e.g. VARCHAR, DOUBLE)
 * @returns          - Classified ColumnKind
 */
export function classifyColumn(columnName: string, dataType: string): ColumnKind {
  const typeUpper = dataType.toUpperCase();

  // 1. Geo heuristic — latitude/longitude coordinates (regardless of type)
  if (GEO_NAME_PATTERN.test(columnName)) {
    return 'geo';
  }

  const isNumeric = MEASURE_TYPE_PATTERNS.some((p) => typeUpper.includes(p));

  // 2. Foreign/surrogate keys (ends with _id, and is numeric)
  if (isNumeric && ID_NAME_PATTERN.test(columnName)) {
    return 'id';
  }

  // 3. General numeric fields -> measure
  if (isNumeric) {
    return 'measure';
  }

  // 4. Textual and date types -> dimension
  if (DIMENSION_TYPE_PATTERNS.some((p) => typeUpper.includes(p))) {
    return 'dimension';
  }

  // Default fallback
  return 'dimension';
}
