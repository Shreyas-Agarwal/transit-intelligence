import fs from 'node:fs';
import path from 'node:path';
import protobuf from 'protobufjs';
import type { RawFeedMessage } from '../types/gtfs-rt.js';

// Resolve the proto file path relative to this module.
// __dirname differs depending on source (Vitest) vs compiled (dist) execution.
const PROTO_PATH_SRC = path.resolve(__dirname, '../../proto/gtfs-realtime.proto');
const PROTO_PATH_DIST = path.resolve(__dirname, '../../../proto/gtfs-realtime.proto');
const PROTO_PATH = fs.existsSync(PROTO_PATH_SRC) ? PROTO_PATH_SRC : PROTO_PATH_DIST;

let _root: protobuf.Root | null = null;

/**
 * Lazily loads and caches the parsed protobuf root.
 * Parsing is I/O once and then reused for all subsequent decode calls.
 */
async function getRoot(): Promise<protobuf.Root> {
  if (_root) return _root;
  const root = new protobuf.Root();
  _root = await root.load(PROTO_PATH, { keepCase: true });
  return _root;
}

/**
 * Decodes a binary GTFS-RT protobuf buffer into a plain JavaScript object.
 *
 * The decode pipeline is:
 *   binary buffer
 *   → protobufjs decode (using the official gtfs-realtime.proto)
 *   → toObject() with enum strings and long as numbers
 *   → typed as RawFeedMessage
 *
 * The resulting object is JSON-serialisable, which is the canonical format
 * published to Redpanda in Sprint 02.
 *
 * @param buffer Raw binary payload from the GTFS-RT feed HTTP response.
 * @returns      Decoded feed message as a plain JS object.
 */
export async function decodeFeedBuffer(buffer: Buffer): Promise<RawFeedMessage> {
  const root = await getRoot();
  const FeedMessage = root.lookupType('transit_realtime.FeedMessage');

  const decoded = FeedMessage.decode(buffer);

  // toObject() converts:
  //   - enum numeric values → string names (enums: true)
  //   - Long instances      → JS numbers  (longs: Number)
  //   - bytes               → base64 strings (bytes: String)
  const obj = FeedMessage.toObject(decoded, {
    enums: String,
    longs: Number,
    bytes: String,
    defaults: false,
    arrays: true,
    objects: true,
    oneofs: true,
  }) as RawFeedMessage;

  return obj;
}
