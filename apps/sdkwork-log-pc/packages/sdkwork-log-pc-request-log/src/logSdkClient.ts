//! Log backend SDK client factory for PC surfaces.
//!
//! Hosting applications configure the shared client once (base URL, token
//! manager, platform) through `configureLogBackendSdkClient`; all services in
//! this package consume the same instance.

import {
  createClient,
  type SdkworkBackendClient,
  type SdkworkBackendConfig,
} from '@sdkwork/log-backend-sdk';

const DEFAULT_BACKEND_API_PREFIX = '/backend/v3/api';

let sharedClient: SdkworkBackendClient | null = null;
let sharedConfig: SdkworkBackendConfig | null = null;

/** Returns the shared log backend SDK client, creating it on first use. */
export function getLogBackendSdkClient(): SdkworkBackendClient {
  if (!sharedClient) {
    sharedClient = createClient({
      baseUrl: DEFAULT_BACKEND_API_PREFIX,
      platform: 'web',
      ...(sharedConfig ?? {}),
    });
  }
  return sharedClient;
}

/**
 * Overrides the shared client configuration (base URL, token manager, auth
 * mode, headers). Must be called before the first `getLogBackendSdkClient`
 * use for the override to take effect.
 */
export function configureLogBackendSdkClient(config: SdkworkBackendConfig): void {
  sharedConfig = { ...(sharedConfig ?? {}), ...config };
  if (sharedClient) {
    sharedClient = createClient({
      ...sharedConfig,
      baseUrl: sharedConfig.baseUrl ?? DEFAULT_BACKEND_API_PREFIX,
    });
  }
}

/** Test/teardown helper: forgets the shared client and config. */
export function resetLogBackendSdkClient(): void {
  sharedClient = null;
  sharedConfig = null;
}
