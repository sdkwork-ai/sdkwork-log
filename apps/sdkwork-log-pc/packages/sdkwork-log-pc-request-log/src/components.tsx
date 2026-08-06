//! Reusable request log presentation components (PC surface).

import React from 'react';
import type { RequestLogDetail, RequestLogItem } from './types';

/** Formats an epoch-second timestamp into a local date-time string. */
export function formatLogTimestamp(epochSeconds: string, locale = 'en-US'): string {
  const parsed = Number(epochSeconds);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    return epochSeconds || '-';
  }
  return new Date(parsed * 1000).toLocaleString(locale);
}

function statusBadgeClassName(statusCode?: number | null): string {
  if (statusCode === undefined || statusCode === null) {
    return 'bg-slate-100 text-slate-600 border-slate-200';
  }
  if (statusCode >= 500) {
    return 'bg-rose-50 text-rose-600 border-rose-200';
  }
  if (statusCode >= 400) {
    return 'bg-amber-50 text-amber-600 border-amber-200';
  }
  return 'bg-emerald-50 text-emerald-600 border-emerald-200';
}

/** Simple metadata table for one request log row. */
export function RequestLogTable({
  items,
  onSelect,
  renderTimestamp = formatLogTimestamp,
  locale = 'en-US',
}: {
  items: RequestLogItem[];
  onSelect?: (item: RequestLogItem) => void;
  renderTimestamp?: (epochSeconds: string, locale: string) => string;
  locale?: string;
}): React.ReactElement {
  return (
    <div className="w-full overflow-x-auto">
      <table className="w-full text-left text-sm whitespace-nowrap">
        <thead>
          <tr className="border-b border-slate-200 text-xs uppercase font-semibold text-slate-500">
            <th className="px-3 py-2">time</th>
            <th className="px-3 py-2">surface</th>
            <th className="px-3 py-2">method</th>
            <th className="px-3 py-2">path</th>
            <th className="px-3 py-2">status</th>
            <th className="px-3 py-2">duration</th>
            <th className="px-3 py-2">service</th>
            <th className="px-3 py-2">traceId</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-slate-100 text-slate-700">
          {items.map((item) => (
            <tr
              key={item.id}
              onClick={() => onSelect?.(item)}
              className={onSelect ? 'cursor-pointer hover:bg-slate-50' : undefined}
            >
              <td className="px-3 py-2 font-mono text-xs">{renderTimestamp(item.createdAt, locale)}</td>
              <td className="px-3 py-2 font-mono text-xs">{item.apiSurface}</td>
              <td className="px-3 py-2 font-mono text-xs">{item.method}</td>
              <td className="px-3 py-2 font-mono text-xs max-w-[320px] truncate" title={item.path}>
                {item.path}
              </td>
              <td className="px-3 py-2">
                <span
                  className={`inline-flex px-2 py-0.5 rounded-full border text-xs font-medium ${statusBadgeClassName(item.statusCode)}`}
                >
                  {item.statusCode ?? '-'}
                </span>
              </td>
              <td className="px-3 py-2 font-mono text-xs">{item.durationMs ? `${item.durationMs} ms` : '-'}</td>
              <td className="px-3 py-2 font-mono text-xs">{item.service ?? '-'}</td>
              <td className="px-3 py-2 font-mono text-xs max-w-[200px] truncate" title={item.traceId}>
                {item.traceId}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** JSON body viewer for the full redacted request/response bodies. */
export function RequestLogBodyView({
  label,
  body,
}: {
  label: string;
  body?: string | null;
}): React.ReactElement {
  const formatted = body ? prettyPrintBody(body) : null;
  return (
    <div className="flex flex-col gap-1">
      <div className="text-xs font-semibold text-slate-500 uppercase">{label}</div>
      {formatted ? (
        <pre className="max-h-[360px] overflow-auto rounded-lg bg-slate-50 border border-slate-200 p-3 text-xs font-mono whitespace-pre-wrap break-all">
          {formatted}
        </pre>
      ) : (
        <div className="rounded-lg bg-slate-50 border border-slate-200 p-3 text-xs text-slate-400">(no body captured)</div>
      )}
    </div>
  );
}

/** Full input/output panel for one request log detail row. */
export function RequestLogDetailPanel({
  detail,
  renderTimestamp = formatLogTimestamp,
  locale = 'en-US',
}: {
  detail: RequestLogDetail;
  renderTimestamp?: (epochSeconds: string, locale: string) => string;
  locale?: string;
}): React.ReactElement {
  return (
    <div className="flex flex-col gap-4 text-sm">
      <dl className="grid grid-cols-2 gap-x-6 gap-y-2 text-xs">
        <dt className="text-slate-500">Trace ID</dt>
        <dd className="font-mono">{detail.traceId}</dd>
        <dt className="text-slate-500">Request ID</dt>
        <dd className="font-mono">{detail.requestId}</dd>
        <dt className="text-slate-500">Surface / Method</dt>
        <dd className="font-mono">
          {detail.apiSurface} · {detail.method}
        </dd>
        <dt className="text-slate-500">Path</dt>
        <dd className="font-mono">{detail.path}</dd>
        <dt className="text-slate-500">Status</dt>
        <dd className="font-mono">
          {detail.statusCode ?? '-'}
          {detail.errorCode ? ` (${detail.errorCode})` : ''}
          {detail.failedStage ? ` @ ${detail.failedStage}` : ''}
        </dd>
        <dt className="text-slate-500">Duration</dt>
        <dd className="font-mono">{detail.durationMs ? `${detail.durationMs} ms` : '-'}</dd>
        <dt className="text-slate-500">Service</dt>
        <dd className="font-mono">{detail.service ?? '-'}</dd>
        <dt className="text-slate-500">Time</dt>
        <dd className="font-mono">{renderTimestamp(detail.createdAt, locale)}</dd>
        <dt className="text-slate-500">Operation</dt>
        <dd className="font-mono">{detail.operationId ?? '-'}</dd>
        <dt className="text-slate-500">Tenant / User</dt>
        <dd className="font-mono">
          {detail.tenantId ?? '-'} / {detail.userId ?? '-'}
        </dd>
        <dt className="text-slate-500">Query Params</dt>
        <dd className="font-mono break-all">{detail.queryParams ?? '-'}</dd>
        <dt className="text-slate-500">Headers</dt>
        <dd className="font-mono break-all">{detail.requestHeaders ?? '-'}</dd>
      </dl>
      <RequestLogBodyView label="Request Body" body={detail.requestBody} />
      <RequestLogBodyView label="Response Body" body={detail.responseBody} />
    </div>
  );
}

function prettyPrintBody(body: string): string {
  try {
    return JSON.stringify(JSON.parse(body), null, 2);
  } catch {
    return body;
  }
}
