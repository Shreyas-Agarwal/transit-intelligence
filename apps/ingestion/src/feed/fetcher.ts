import https from 'node:https';
import http from 'node:http';

/**
 * Fetches the binary GTFS-RT protobuf payload from the feed URL.
 *
 * Open Data Swiss requires a Bearer token in the Authorization header.
 * The feed returns a binary Protocol Buffer encoded body.
 *
 * This function follows HTTP redirects (301, 302, 303, 307, 308) up to a limit.
 *
 * @param feedUrl        The full URL to the GTFS-RT feed endpoint.
 * @param apiToken       Bearer token for authentication.
 * @param redirectLimit  Maximum number of redirects to follow (default: 5).
 * @returns              Raw binary buffer of the protobuf response body.
 */
export function fetchFeedBuffer(
  feedUrl: string,
  apiToken: string,
  redirectLimit = 5,
): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    if (redirectLimit < 0) {
      return reject(new Error(`Too many redirects followed for ${feedUrl}`));
    }

    const url = new URL(feedUrl);
    const transport = url.protocol === 'https:' ? https : http;

    // Only send the Authorization header if the redirect hostname is still within opentransportdata.swiss.
    const headers: Record<string, string> = {
      Accept: 'application/x-protobuf',
      'User-Agent': 'transit-intelligence-ingestion/1.0',
    };

    if (url.hostname.endsWith('opentransportdata.swiss')) {
      headers.Authorization = `Bearer ${apiToken}`;
    }

    const options = {
      hostname: url.hostname,
      port: url.port || (url.protocol === 'https:' ? 443 : 80),
      path: url.pathname + url.search,
      method: 'GET',
      headers,
    };

    const req = transport.request(options, (res) => {
      // Check for redirect status codes
      if (
        res.statusCode !== undefined &&
        [301, 302, 303, 307, 308].includes(res.statusCode)
      ) {
        const redirectLocation = res.headers.location;
        if (!redirectLocation) {
          reject(
            new Error(`Redirect status ${res.statusCode} received but no Location header found`),
          );
          res.resume();
          return;
        }

        // Resolve absolute URL
        const absoluteRedirectUrl = new URL(redirectLocation, feedUrl).toString();
        res.resume(); // drain current response

        // Recurse to follow redirect
        fetchFeedBuffer(absoluteRedirectUrl, apiToken, redirectLimit - 1)
          .then(resolve)
          .catch(reject);
        return;
      }

      if (res.statusCode === undefined || res.statusCode < 200 || res.statusCode >= 300) {
        reject(
          new Error(`Feed HTTP error: ${res.statusCode} ${res.statusMessage} from ${feedUrl}`),
        );
        res.resume(); // drain
        return;
      }

      const chunks: Buffer[] = [];
      res.on('data', (chunk: Buffer) => chunks.push(chunk));
      res.on('end', () => resolve(Buffer.concat(chunks)));
      res.on('error', reject);
    });

    req.on('error', reject);
    req.setTimeout(15_000, () => {
      req.destroy(new Error(`Feed request timed out after 15s for ${feedUrl}`));
    });

    req.end();
  });
}
