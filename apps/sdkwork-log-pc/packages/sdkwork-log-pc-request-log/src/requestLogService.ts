//! Request log list/detail service over the generated log backend SDK.

import { getLogBackendSdkClient } from './logSdkClient';
import type {
  RequestLogDetail,
  RequestLogItem,
  RequestLogListFilters,
  RequestLogPage,
} from './types';

const DEFAULT_PAGE_SIZE = 20;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function readRequiredString(record: Record<string, unknown>, key: string, message: string): string {
  const value = record[key];
  if (typeof value !== 'string' || !value.trim()) {
    throw new Error(message);
  }
  return value.trim();
}

function readOptionalString(record: Record<string, unknown>, key: string): string | null {
  const value = record[key];
  if (value === undefined || value === null) {
    return null;
  }
  return String(value).trim() || null;
}

function readOptionalNumber(record: Record<string, unknown>, key: string): number | null {
  const value = record[key];
  if (value === undefined || value === null || value === '') {
    return null;
  }
  const parsed = typeof value === 'number' ? value : Number(String(value).trim());
  return Number.isFinite(parsed) ? parsed : null;
}

function readOptionalStringNumber(record: Record<string, unknown>, key: string): string | null {
  const value = record[key];
  if (value === undefined || value === null || value === '') {
    return null;
  }
  const normalized = String(value).trim();
  return normalized ? normalized : null;
}

function normalizeRequestLogItem(value: unknown): RequestLogItem {
  if (!isRecord(value)) {
    throw new Error('Request log item is required');
  }
  const item: RequestLogItem = {
    id: readRequiredString(value, 'id', 'Request log id is required'),
    traceId: readRequiredString(value, 'traceId', 'Request log traceId is required'),
    requestId: readRequiredString(value, 'requestId', 'Request log requestId is required'),
    apiSurface: readRequiredString(value, 'apiSurface', 'Request log apiSurface is required') as RequestLogItem['apiSurface'],
    path: readRequiredString(value, 'path', 'Request log path is required'),
    method: readRequiredString(value, 'method', 'Request log method is required'),
    createdAt: readRequiredString(value, 'createdAt', 'Request log createdAt is required'),
    tenantId: readOptionalString(value, 'tenantId'),
    userId: readOptionalString(value, 'userId'),
    operationId: readOptionalString(value, 'operationId'),
    service: readOptionalString(value, 'service'),
    environment: readOptionalString(value, 'environment'),
    authMode: readOptionalString(value, 'authMode'),
    statusCode: readOptionalNumber(value, 'statusCode'),
    durationMs: readOptionalStringNumber(value, 'durationMs'),
    errorCode: readOptionalNumber(value, 'errorCode'),
    failedStage: readOptionalString(value, 'failedStage'),
    queryParams: readOptionalString(value, 'queryParams'),
    requestHeaders: readOptionalString(value, 'requestHeaders'),
    expiresAt: readOptionalStringNumber(value, 'expiresAt'),
  };
  return item;
}

function normalizeRequestLogDetail(value: unknown): RequestLogDetail {
  const item = normalizeRequestLogItem(value);
  if (!isRecord(value)) {
    return item;
  }
  return {
    ...item,
    requestBody: readOptionalString(value, 'requestBody'),
    responseBody: readOptionalString(value, 'responseBody'),
  };
}

function readPageTotal(pageInfo: unknown): number {
  if (!isRecord(pageInfo)) {
    return 0;
  }
  const totalItems = pageInfo.totalItems;
  if (totalItems === undefined || totalItems === null) {
    return 0;
  }
  const parsed = Number(String(totalItems));
  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : 0;
}

export class RequestLogService {
  /** Lists request logs (metadata only; bodies come from `detail`). */
  static async list(filters: RequestLogListFilters = {}): Promise<RequestLogPage> {
    const page = filters.page ?? 1;
    const pageSize = filters.pageSize ?? DEFAULT_PAGE_SIZE;
    const client = getLogBackendSdkClient();
    const data = await client.log.requestLogs.list({
      traceId: filters.traceId || undefined,
      requestId: filters.requestId || undefined,
      apiSurface: filters.apiSurface || undefined,
      method: filters.method || undefined,
      operationId: filters.operationId || undefined,
      service: filters.service || undefined,
      status: filters.status,
      durationMin: filters.durationMin || undefined,
      durationMax: filters.durationMax || undefined,
      createdFrom: filters.createdFrom || undefined,
      createdTo: filters.createdTo || undefined,
      page,
      pageSize,
    });
    if (!isRecord(data)) {
      throw new Error('Request log page data is required');
    }
    const rawItems = Array.isArray(data.items) ? data.items : [];
    return {
      items: rawItems.map(normalizeRequestLogItem),
      total: readPageTotal(data.pageInfo),
      page,
      pageSize,
    };
  }

  /** Fetches one request log row with the full redacted input/output bodies. */
  static async detail(id: string): Promise<RequestLogDetail> {
    const client = getLogBackendSdkClient();
    const data = await client.log.requestLogs.detail(id);
    return normalizeRequestLogDetail(data);
  }
}
