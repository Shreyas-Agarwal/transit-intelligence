//-------------------------------------
// Semantic Model — Query Planner & SQL Compiler
//
// Translates a widget configuration + active filter context + schema catalog
// into a single executable SQL query. Resolves multi-table joins using relationships.
//-------------------------------------

import type { Widget, FilterContext, DiscoveredTable, AggregationFn } from '../registry/types';
import { findJoinPath } from './relationships';

const CHART_ROW_LIMIT = 50;
const TABLE_ROW_LIMIT = 200;
const SCATTER_ROW_LIMIT = 500;

/** Represents a parsed column with its parent table. */
export interface SemanticColumn {
  table: string;
  column: string;
}

/**
 * Parse a table-qualified column name string (e.g. "stops.stop_name" or "stop_name")
 * into a structured SemanticColumn.
 */
export function parseColumn(colStr: string | null, defaultTable: string): SemanticColumn | null {
  if (!colStr) return null;
  if (colStr.includes('.')) {
    const [table, column] = colStr.split('.');
    return { table, column };
  }
  return { table: defaultTable, column: colStr };
}

/** Formats a column expression for SQL queries (e.g. table.column or AGG(table.column)). */
export function formatColumnExpr(col: SemanticColumn, aggregation: AggregationFn): string {
  const fullColName = `"${col.table}"."${col.column}"`;
  
  if (aggregation === 'none') {
    return fullColName;
  }
  if (aggregation === 'COUNT') {
    return `COUNT(${fullColName})`;
  }
  if (aggregation === 'COUNT_DISTINCT') {
    return `COUNT(DISTINCT ${fullColName})`;
  }
  return `${aggregation}(${fullColName})`;
}

/**
 * Generate a multi-table SQL query for the given widget.
 *
 * @param widget        - Widget config
 * @param filterContext - Active cross-filter context
 * @param catalog       - Discovered tables/columns schema
 * @returns             - Executable SQL query
 */
export function planWidgetSql(
  widget: Widget,
  filterContext: FilterContext | null,
  catalog: DiscoveredTable[]
): string {
  // 0. Auto-detect base table from qualified columns if not specified
  let detectedTable = '';
  const checkCol = (colStr: string | null) => {
    if (colStr && colStr.includes('.')) {
      detectedTable = colStr.split('.')[0];
      return true;
    }
    return false;
  };
  
  const found = checkCol(widget.dimensionCol) || 
              checkCol(widget.measureCol) || 
              checkCol(widget.matrixRowCol || null) || 
              checkCol(widget.matrixColCol || null);
              
  if (!found && widget.type === 'table' && widget.tableColumns) {
    for (const tc of widget.tableColumns) {
      if (checkCol(tc.name)) {
        break;
      }
    }
  }

  const baseTable = widget.tableName || detectedTable || (catalog[0] ? catalog[0].name : 'stops');
  const tables = new Set<string>([baseTable]);

  // 1. Parse columns and extract involved tables
  const dim = parseColumn(widget.dimensionCol, baseTable);
  const msr = parseColumn(widget.measureCol, baseTable);
  const rowDim = parseColumn(widget.matrixRowCol || null, baseTable);
  const colDim = parseColumn(widget.matrixColCol || null, baseTable);
  const legend = parseColumn(widget.legendCol || null, baseTable);

  if (dim) tables.add(dim.table);
  if (msr) tables.add(msr.table);
  if (rowDim) tables.add(rowDim.table);
  if (colDim) tables.add(colDim.table);
  if (legend) tables.add(legend.table);

  if (widget.type === 'table' && widget.tableColumns) {
    for (const tc of widget.tableColumns) {
      const parsed = parseColumn(tc.name, baseTable);
      if (parsed) tables.add(parsed.table);
    }
  }

  // 2. Map widget geo dependencies: ensure 'stops' table is joined to provide coordinates
  let geoLat: SemanticColumn | null = null;
  let geoLon: SemanticColumn | null = null;
  if (widget.type === 'map') {
    // If the tables don't include a table with geo columns, add 'stops' which has stop_lat and stop_lon
    const hasGeo = Array.from(tables).some(t => {
      const tableDef = catalog.find(cd => cd.name === t);
      return tableDef?.columns.some(c => c.kind === 'geo') ?? false;
    });

    if (!hasGeo) {
      tables.add('stops');
    }

    // Now find the geo columns in our active tables list
    for (const t of tables) {
      const tableDef = catalog.find(cd => cd.name === t);
      if (tableDef) {
        const latCol = tableDef.columns.find(c => c.kind === 'geo' && c.name.toLowerCase().includes('lat'));
        const lonCol = tableDef.columns.find(c => c.kind === 'geo' && (c.name.toLowerCase().includes('lon') || c.name.toLowerCase().includes('lng')));
        if (latCol && lonCol) {
          geoLat = { table: t, column: latCol.name };
          geoLon = { table: t, column: lonCol.name };
          break;
        }
      }
    }
  }

  // 3. Inject cross-filter table if active and matching
  let hasActiveFilter = false;
  let filterCol: SemanticColumn | null = null;
  if (filterContext && filterContext.tableName && filterContext.column && filterContext.value != null) {
    hasActiveFilter = true;
    filterCol = parseColumn(`${filterContext.tableName}.${filterContext.column}`, filterContext.tableName);
    tables.add(filterContext.tableName);
  }

  // 4. Resolve join steps using relationships finder
  const joinSteps = findJoinPath(Array.from(tables));

  // 5. Build FROM & JOIN clauses
  let fromClause = `FROM "${baseTable}"`;
  for (const step of joinSteps) {
    fromClause += `\n  JOIN "${step.toTable}" ON "${step.fromTable}"."${step.fromColumn}" = "${step.toTable}"."${step.toColumn}"`;
  }

  // 6. Build WHERE clause (cross-filter support)
  let whereClause = '';
  if (hasActiveFilter && filterCol && filterContext?.value != null) {
    const safeValue = filterContext.value.replace(/'/g, "''");
    whereClause = `\nWHERE "${filterCol.table}"."${filterCol.column}" = '${safeValue}'`;
  }

  // 7. Compile specific queries based on WidgetType
  switch (widget.type) {
    case 'metric': {
      let selectExpr = 'COUNT(*) AS "value"';
      if (msr) {
        selectExpr = `${formatColumnExpr(msr, widget.aggregation)} AS "value"`;
      }
      return `SELECT ${selectExpr}\n${fromClause}${whereClause}`;
    }

    case 'table': {
      if (widget.tableColumns && widget.tableColumns.length > 0) {
        const selectCols: string[] = [];
        const groupByCols: string[] = [];
        let hasAggregates = false;

        for (const tc of widget.tableColumns) {
          const parsed = parseColumn(tc.name, baseTable)!;
          const expr = formatColumnExpr(parsed, tc.aggregation);
          const alias = tc.alias || tc.name;
          selectCols.push(`${expr} AS "${alias}"`);

          if (tc.aggregation === 'none') {
            groupByCols.push(`"${parsed.table}"."${parsed.column}"`);
          } else {
            hasAggregates = true;
          }
        }

        let query = `SELECT\n  ${selectCols.join(',\n  ')}\n${fromClause}${whereClause}`;
        if (hasAggregates && groupByCols.length > 0) {
          query += `\nGROUP BY\n  ${groupByCols.join(',\n  ')}`;
        }
        query += `\nLIMIT ${TABLE_ROW_LIMIT}`;
        return query;
      }

      // Default fallback
      return `SELECT *\n${fromClause}${whereClause}\nLIMIT ${TABLE_ROW_LIMIT}`;
    }

    case 'matrix': {
      if (!rowDim || !colDim || !msr) {
        return `SELECT *\n${fromClause}${whereClause}\nLIMIT ${TABLE_ROW_LIMIT}`;
      }
      const valExpr = formatColumnExpr(msr, widget.aggregation);
      return [
        `SELECT`,
        `  "${rowDim.table}"."${rowDim.column}" AS "row",`,
        `  "${colDim.table}"."${colDim.column}" AS "col",`,
        `  ${valExpr} AS "value"`,
        fromClause + whereClause,
        `GROUP BY`,
        `  "${rowDim.table}"."${rowDim.column}",`,
        `  "${colDim.table}"."${colDim.column}"`,
        `ORDER BY "row" ASC, "col" ASC`,
      ].join('\n');
    }

    case 'map': {
      // Map widget fetches latitude, longitude, and descriptive columns
      const latExpr = geoLat ? `"${geoLat.table}"."${geoLat.column}"` : '0';
      const lonExpr = geoLon ? `"${geoLon.table}"."${geoLon.column}"` : '0';
      const nameExpr = dim ? `"${dim.table}"."${dim.column}"` : 'stop_name';
      
      const valExpr = msr ? formatColumnExpr(msr, widget.aggregation) : nameExpr;
      const valSelect = `, ${valExpr} AS "value"`;
      const groupBy = msr ? `\nGROUP BY\n  ${nameExpr},\n  ${latExpr},\n  ${lonExpr}` : '';

      return [
        `SELECT`,
        `  ${nameExpr} AS "name",`,
        `  ${latExpr} AS "lat",`,
        `  ${lonExpr} AS "lon"${valSelect}`,
        fromClause + whereClause,
        groupBy,
        `LIMIT 1000`
      ].filter(Boolean).join('\n');
    }

    // Chart types: bar, bar-horizontal, line, pie, scatter
    case 'bar':
    case 'bar-horizontal':
    case 'line':
    case 'pie': {
      if (!dim || !msr) {
        return `SELECT *\n${fromClause}${whereClause}\nLIMIT ${CHART_ROW_LIMIT}`;
      }
      const valExpr = formatColumnExpr(msr, widget.aggregation);
      const orderCol = widget.aggregation === 'none' ? `"${msr.table}"."${msr.column}"` : `"value"`;

      const selectCols = [
        `"${dim.table}"."${dim.column}" AS "${dim.column}"`,
        `${valExpr} AS "value"`
      ];
      const groupByCols = [
        `"${dim.table}"."${dim.column}"`
      ];

      if (widget.aggregation === 'none') {
        groupByCols.push(`"${msr.table}"."${msr.column}"`);
      }

      if (legend && widget.type !== 'pie') {
        selectCols.push(`"${legend.table}"."${legend.column}" AS "legend"`);
        groupByCols.push(`"${legend.table}"."${legend.column}"`);
      }

      return [
        `SELECT`,
        `  ${selectCols.join(',\n  ')}`,
        fromClause + whereClause,
        `GROUP BY\n  ${groupByCols.join(',\n  ')}`,
        `ORDER BY ${orderCol} DESC`,
        `LIMIT ${CHART_ROW_LIMIT}`,
      ].join('\n');
    }

    case 'scatter': {
      if (!dim || !msr) {
        return `SELECT *\n${fromClause}${whereClause}\nLIMIT ${CHART_ROW_LIMIT}`;
      }
      // Scatter uses dimCol as x and msrCol as y
      return [
        `SELECT`,
        `  "${dim.table}"."${dim.column}" AS "x",`,
        `  "${msr.table}"."${msr.column}" AS "y"`,
        fromClause + whereClause,
        `LIMIT ${SCATTER_ROW_LIMIT}`,
      ].join('\n');
    }
  }
}
