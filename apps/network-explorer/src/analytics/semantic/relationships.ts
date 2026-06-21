//-------------------------------------
// Semantic Model — GTFS Schema Relationships
//
// Centrally defines the explicit relational schema for the GTFS datasets.
// Provides a BFS pathfinder to resolve join paths between tables automatically.
//-------------------------------------

export interface Relationship {
  fromTable: string;
  fromColumn: string;
  toTable: string;
  toColumn: string;
}

/** Explicit, fixed relational relationships in our GTFS schema. */
export const GTFS_RELATIONSHIPS: Relationship[] = [
  { fromTable: 'stop_times', fromColumn: 'stop_id', toTable: 'stops', toColumn: 'stop_id' },
  { fromTable: 'stop_times', fromColumn: 'trip_id', toTable: 'trips', toColumn: 'trip_id' },
  { fromTable: 'trips', fromColumn: 'route_id', toTable: 'routes', toColumn: 'route_id' },
  { fromTable: 'routes', fromColumn: 'agency_id', toTable: 'agencies', toColumn: 'agency_id' },
];

export interface JoinStep {
  fromTable: string;
  fromColumn: string;
  toTable: string;
  toColumn: string;
}

/**
 * Plan the sequence of JOINs required to connect all requested tables.
 * Resolves paths using BFS. Since the GTFS schema graph is a tree, this resolves
 * the exact set of relationships connecting the tables.
 *
 * @param tables - List of tables that must be joined (e.g. ['stops', 'routes'])
 * @returns      - Ordered list of JoinStep joins
 */
export function findJoinPath(tables: string[]): JoinStep[] {
  if (tables.length <= 1) return [];

  // Build undirected adjacency list for graph traversal
  const adj: Record<string, { table: string; fromCol: string; toCol: string }[]> = {};
  for (const rel of GTFS_RELATIONSHIPS) {
    if (!adj[rel.fromTable]) adj[rel.fromTable] = [];
    adj[rel.fromTable].push({ table: rel.toTable, fromCol: rel.fromColumn, toCol: rel.toColumn });

    if (!adj[rel.toTable]) adj[rel.toTable] = [];
    adj[rel.toTable].push({ table: rel.fromTable, fromCol: rel.toColumn, toCol: rel.fromColumn });
  }

  const root = tables[0];
  const joinedSteps: JoinStep[] = [];
  const visited = new Set<string>([root]);

  // Connect subsequent tables back to the visited tree
  for (let i = 1; i < tables.length; i++) {
    const target = tables[i];
    if (visited.has(target)) continue;

    const queue: { current: string; path: JoinStep[] }[] = [];
    for (const v of visited) {
      if (adj[v]) {
        for (const edge of adj[v]) {
          queue.push({
            current: edge.table,
            path: [
              {
                fromTable: v,
                fromColumn: edge.fromCol,
                toTable: edge.table,
                toColumn: edge.toCol,
              },
            ],
          });
        }
      }
    }

    const pathVisited = new Set<string>(visited);
    let foundPath: JoinStep[] | null = null;

    while (queue.length > 0) {
      const { current, path } = queue.shift()!;
      if (pathVisited.has(current)) continue;
      pathVisited.add(current);

      if (current === target) {
        foundPath = path;
        break;
      }

      if (adj[current]) {
        for (const edge of adj[current]) {
          if (!pathVisited.has(edge.table)) {
            queue.push({
              current: edge.table,
              path: [
                ...path,
                {
                  fromTable: current,
                  fromColumn: edge.fromCol,
                  toTable: edge.table,
                  toColumn: edge.toCol,
                },
              ],
            });
          }
        }
      }
    }

    if (foundPath) {
      for (const step of foundPath) {
        if (!visited.has(step.toTable)) {
          joinedSteps.push(step);
          visited.add(step.toTable);
        }
      }
    }
  }

  return joinedSteps;
}
