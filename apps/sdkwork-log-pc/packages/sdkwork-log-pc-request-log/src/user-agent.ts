//! User-Agent parsing helpers for request log rows. The raw agent string is
//! stored inside the allow-listed `requestHeaders` JSON (`capture_safe_headers`
//! keeps the `user-agent` header, keys lowercased); the terminal (OS/device)
//! and browser labels are derived client-side so the backend stays free of
//! device parsing.

/** Reads the `user-agent` header value from a `requestHeaders` JSON string.
 *
 * Returns `null` when the JSON is missing, malformed, or carries no usable
 * `user-agent` entry.
 */
export function extractUserAgent(requestHeadersJson: string | null | undefined): string | null {
  if (!requestHeadersJson) {
    return null;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(requestHeadersJson);
  } catch {
    return null;
  }
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
    return null;
  }
  const value = (parsed as Record<string, unknown>)['user-agent'];
  if (typeof value !== 'string' || !value.trim()) {
    return null;
  }
  return value.trim();
}

const CLI_OS_MARKERS = ['curl/', 'httpie/', 'python-requests', 'okhttp', 'go-http-client'];

/** Detects the source terminal (OS/device family) of a User-Agent string. */
export function detectUserAgentOs(userAgent: string): string {
  const value = userAgent.toLowerCase();
  if (CLI_OS_MARKERS.some((marker) => value.includes(marker))) {
    return 'CLI';
  }
  if (value.includes('iphone')) {
    return 'iPhone';
  }
  if (value.includes('ipad')) {
    return 'iPad';
  }
  if (value.includes('android')) {
    return 'Android';
  }
  if (value.includes('windows')) {
    return 'Windows';
  }
  if (value.includes('mac os x') || value.includes('macintosh')) {
    return 'macOS';
  }
  if (value.includes('linux')) {
    return 'Linux';
  }
  return 'Device';
}

/** Detects the browser/client application of a User-Agent string. */
export function detectUserAgentClient(userAgent: string): string {
  const value = userAgent.toLowerCase();
  if (value.includes('edg/')) {
    return 'Edge';
  }
  if (value.includes('firefox/')) {
    return 'Firefox';
  }
  if (value.includes('chrome/') && !value.includes('chromium/') && !value.includes('edg/')) {
    return 'Chrome';
  }
  if (value.includes('chromium/')) {
    return 'Chromium';
  }
  if (value.includes('safari/') && !value.includes('chrome/') && !value.includes('chromium/')) {
    return 'Safari';
  }
  if (value.includes('curl/')) {
    return 'curl';
  }
  if (value.includes('python-requests')) {
    return 'Python';
  }
  if (value.includes('okhttp')) {
    return 'OkHttp';
  }
  if (value.includes('httpie/')) {
    return 'HTTPie';
  }
  if (value.includes('go-http-client')) {
    return 'Go HTTP';
  }
  return 'Client';
}
