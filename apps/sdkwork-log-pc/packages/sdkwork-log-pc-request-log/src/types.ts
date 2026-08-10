//! Request log wire types (camelCase, aligned with the log backend-api
//! contract `log.requestLogs.list` / `log.requestLogs.detail`).

export type LogApiSurface =
  | 'open-api'
  | 'app-api'
  | 'backend-api'
  | 'internal-api'
  | 'gateway-api'
  | 'unknown';

/** One request log row as served by the list endpoint (bodies excluded). */
export interface RequestLogItem {
  id: string;
  traceId: string;
  requestId: string;
  tenantId?: string | null;
  userId?: string | null;
  apiSurface: LogApiSurface;
  path: string;
  method: string;
  operationId?: string | null;
  service?: string | null;
  environment?: string | null;
  authMode?: string | null;
  statusCode?: number | null;
  durationMs?: string | null;
  errorCode?: number | null;
  failedStage?: string | null;
  queryParams?: string | null;
  requestHeaders?: string | null;
  createdAt: string;
  expiresAt?: string | null;
}

/** Full request log row as served by the detail endpoint. */
export interface RequestLogDetail extends RequestLogItem {
  /** Full redacted request body text (sensitive values are [REDACTED]). */
  requestBody?: string | null;
  /** Full redacted response body text (same hygiene). */
  responseBody?: string | null;
}

/** List query filters accepted by `log.requestLogs.list`. */
export interface RequestLogListFilters {
  traceId?: string;
  requestId?: string;
  apiSurface?: LogApiSurface;
  /** HTTP method filter (for example `GET`). */
  method?: string;
  operationId?: string;
  service?: string;
  status?: number;
  /** Inclusive lower bound on durationMs (wire string). */
  durationMin?: string;
  /** Inclusive upper bound on durationMs (wire string). */
  durationMax?: string;
  /** Inclusive lower bound on createdAt (epoch seconds). */
  createdFrom?: string;
  /** Inclusive upper bound on createdAt (epoch seconds). */
  createdTo?: string;
  page?: number;
  pageSize?: number;
}

export interface RequestLogPage {
  items: RequestLogItem[];
  total: number;
  page: number;
  pageSize: number;
}
