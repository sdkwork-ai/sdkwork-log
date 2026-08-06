//! Lightweight mobile-first request log components (H5 surface).

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

/** Mobile card list for request log rows. */
export function RequestLogCardList({
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
    <div className="flex flex-col gap-3">
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          onClick={() => onSelect?.(item)}
          className="flex flex-col gap-1 rounded-xl border border-slate-200 bg-white p-3 text-left shadow-sm active:bg-slate-50"
        >
          <div className="flex items-center justify-between gap-2">
            <span className="font-mono text-xs text-slate-500">{renderTimestamp(item.createdAt, locale)}</span>
            <span
              className={`rounded-full border px-2 py-0.5 text-xs font-medium ${
                item.statusCode !== null && item.statusCode !== undefined && item.statusCode >= 400
                  ? 'border-rose-200 bg-rose-50 text-rose-600'
                  : 'border-emerald-200 bg-emerald-50 text-emerald-600'
              }`}
            >
              {item.statusCode ?? '-'}
            </span>
          </div>
          <div className="truncate font-mono text-sm text-slate-800">
            {item.method} {item.path}
          </div>
          <div className="truncate font-mono text-[11px] text-slate-400">
            {item.apiSurface} · {item.service ?? '-'} · {item.durationMs ? `${item.durationMs}ms` : '-'}
          </div>
          <div className="truncate font-mono text-[11px] text-slate-400">{item.traceId}</div>
        </button>
      ))}
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

/** Full input/output sheet for one request log detail row (mobile). */
export function RequestLogDetailSheet({
  detail,
  onClose,
  renderTimestamp = formatLogTimestamp,
  locale = 'en-US',
}: {
  detail: RequestLogDetail;
  onClose?: () => void;
  renderTimestamp?: (epochSeconds: string, locale: string) => string;
  locale?: string;
}): React.ReactElement {
  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center bg-black/40" onClick={onClose}>
      <div
        className="max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-t-2xl bg-white p-4"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-base font-semibold">Request Log Detail</h2>
          {onClose && (
            <button type="button" onClick={onClose} className="rounded-full bg-slate-100 px-3 py-1 text-sm">
              Close
            </button>
          )}
        </div>
        <dl className="mb-3 grid grid-cols-1 gap-2 text-xs">
          <div className="flex justify-between gap-3">
            <dt className="text-slate-500">Trace ID</dt>
            <dd className="truncate font-mono">{detail.traceId}</dd>
          </div>
          <div className="flex justify-between gap-3">
            <dt className="text-slate-500">Request ID</dt>
            <dd className="truncate font-mono">{detail.requestId}</dd>
          </div>
          <div className="flex justify-between gap-3">
            <dt className="text-slate-500">Route</dt>
            <dd className="truncate font-mono">
              {detail.apiSurface} · {detail.method} {detail.path}
            </dd>
          </div>
          <div className="flex justify-between gap-3">
            <dt className="text-slate-500">Status</dt>
            <dd className="font-mono">
              {detail.statusCode ?? '-'}
              {detail.errorCode ? ` (${detail.errorCode})` : ''}
              {detail.failedStage ? ` @ ${detail.failedStage}` : ''}
            </dd>
          </div>
          <div className="flex justify-between gap-3">
            <dt className="text-slate-500">Duration</dt>
            <dd className="font-mono">{detail.durationMs ? `${detail.durationMs} ms` : '-'}</dd>
          </div>
          <div className="flex justify-between gap-3">
            <dt className="text-slate-500">Time</dt>
            <dd className="font-mono">{renderTimestamp(detail.createdAt, locale)}</dd>
          </div>
        </dl>
        <BodyBlock label="Request Body" body={detail.requestBody} />
        <BodyBlock label="Response Body" body={detail.responseBody} />
      </div>
    </div>
  );
}

function BodyBlock({ label, body }: { label: string; body?: string | null }): React.ReactElement {
  return (
    <div className="mb-3">
      <div className="mb-1 text-xs font-semibold text-slate-500">{label}</div>
      {body ? (
        <pre className="max-h-[300px] overflow-auto rounded-lg bg-slate-50 p-3 text-xs font-mono whitespace-pre-wrap break-all">
          {prettyPrintBody(body)}
        </pre>
      ) : (
        <div className="rounded-lg bg-slate-50 p-3 text-xs text-slate-400">(no body captured)</div>
      )}
    </div>
  );
}
