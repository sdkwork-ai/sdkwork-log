//! Reusable request log presentation components (PC surface).

/// <reference path="./monaco-env.d.ts" />

import React, { useEffect, useMemo, useState } from 'react';
import Editor, { loader } from '@monaco-editor/react';
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker';
import type { RequestLogDetail, RequestLogItem } from './types';

// Monaco (VS Code-compatible) worker wiring: only the editor + JSON workers are
// registered; the heavy `monaco-editor` package itself is loaded lazily on the
// first body-tab mount (offline-safe, no CDN). Same pattern as the Cloud Router
// console config editor.
(self as unknown as {
  MonacoEnvironment?: {
    getWorker: (_workerId: string, label: string) => Worker;
  };
}).MonacoEnvironment = {
  getWorker(_workerId: string, label: string) {
    if (label === 'json') {
      return new jsonWorker();
    }
    return new editorWorker();
  },
};

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

/** Loads the local Monaco instance once and binds it to the react loader. */
function useMonacoReady(): boolean {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    let disposed = false;
    void import('monaco-editor').then((monacoModule) => {
      if (disposed) {
        return;
      }
      loader.config({ monaco: monacoModule });
      setReady(true);
    });
    return () => {
      disposed = true;
    };
  }, []);
  return ready;
}

/** Tracks the host application resolved dark theme so the editor theme matches.
 *
 * The Cloud Router portal applies `dark` on `<html>` and mirrors the resolved
 * value in `data-resolved-theme` (themePreference.ts); both signals are
 * observed so the editor stays in sync with light/dark toggles. */
function useHostDarkTheme(): boolean {
  const [dark, setDark] = useState(readHostResolvedDarkTheme);
  useEffect(() => {
    const root = document.documentElement;
    const observer = new MutationObserver(() => {
      setDark(readHostResolvedDarkTheme());
    });
    observer.observe(root, { attributes: true, attributeFilter: ['class', 'data-resolved-theme'] });
    return () => observer.disconnect();
  }, []);
  return dark;
}

function readHostResolvedDarkTheme(): boolean {
  if (typeof document === 'undefined') {
    return false;
  }
  const root = document.documentElement;
  if (root.classList.contains('dark')) {
    return true;
  }
  return root.dataset.resolvedTheme === 'dark';
}

/** Read-only editor options tuned for body inspection (VS Code look & feel). */
const BODY_EDITOR_OPTIONS: import('monaco-editor').editor.IStandaloneEditorConstructionOptions = {
  readOnly: true,
  minimap: { enabled: false },
  lineNumbers: 'on',
  fontSize: 12.5,
  fontLigatures: true,
  wordWrap: 'on',
  scrollBeyondLastLine: false,
  scrollbar: {
    verticalScrollbarSize: 8,
    horizontalScrollbarSize: 8,
    useShadows: false,
  },
  renderLineHighlight: 'line',
  lineHeight: 19,
  padding: { top: 12, bottom: 12 },
  contextmenu: false,
  folding: true,
  glyphMargin: false,
  lineDecorationsWidth: 8,
  overviewRulerBorder: false,
  hideCursorInOverviewRuler: true,
  cursorBlinking: 'solid',
  cursorStyle: 'block-outline',
  stickyScroll: { enabled: false },
  smoothScrolling: true,
  automaticLayout: true,
};

/** Infers the Monaco language from the body content (JSON when parseable). */
function inferBodyLanguage(body: string | null | undefined): string {
  if (!body) {
    return 'plaintext';
  }
  try {
    JSON.parse(body);
    return 'json';
  } catch {
    return 'plaintext';
  }
}

function prettyPrintBody(body: string): string {
  try {
    return JSON.stringify(JSON.parse(body), null, 2);
  } catch {
    return body;
  }
}

const BODY_TAB_CLASSES = {
  // VS Code file-tab look: the active tab shares the editor background and
  // carries a host-accent top bar; inactive tabs float on the tab bar. All
  // backgrounds follow the host light/dark theme (`dark:` variants) and the
  // accent color follows the portal theme color preference
  // (--cloud-router-accent; indigo fallback for hosts that do not define it).
  base:
    'flex items-center rounded-t-md border px-3 py-1.5 -mb-px text-xs font-medium transition-colors select-none',
  active:
    'border-slate-200 dark:border-white/10 border-b-0 bg-white dark:bg-[#1e1e1e]',
  inactive:
    'border-transparent text-slate-500 hover:bg-slate-200/70 hover:text-slate-700 '
    + 'dark:text-slate-400 dark:hover:bg-white/[0.06] dark:hover:text-slate-200',
};

/** Request/Response body tabs rendered with a read-only Monaco (VS Code) editor. */
export function RequestLogBodyTabs({
  requestBody,
  responseBody,
}: {
  requestBody?: string | null;
  responseBody?: string | null;
}): React.ReactElement {
  const [active, setActive] = useState<'request' | 'response'>('request');
  const monacoReady = useMonacoReady();
  const dark = useHostDarkTheme();
  const body = active === 'request' ? requestBody : responseBody;
  const formatted = body ? prettyPrintBody(body) : null;
  const language = useMemo(() => inferBodyLanguage(formatted), [formatted]);

  return (
    <div
      className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-slate-200 dark:border-white/10 bg-white dark:bg-[#1e1e1e]"
      style={{ '--cloud-router-accent': 'var(--cloud-router-accent, #6366f1)' } as React.CSSProperties}
    >
      <div className="flex shrink-0 items-end gap-1 border-b border-slate-200 dark:border-white/10 bg-slate-100 dark:bg-[#252526] px-2 pt-1.5">
        <button
          type="button"
          onClick={() => setActive('request')}
          className={`${BODY_TAB_CLASSES.base} ${active === 'request' ? BODY_TAB_CLASSES.active : BODY_TAB_CLASSES.inactive}`}
          style={
            active === 'request'
              ? { color: 'var(--cloud-router-accent)', boxShadow: 'inset 0 2px 0 0 var(--cloud-router-accent)' }
              : undefined
          }
        >
          Request Body
        </button>
        <button
          type="button"
          onClick={() => setActive('response')}
          className={`${BODY_TAB_CLASSES.base} ${active === 'response' ? BODY_TAB_CLASSES.active : BODY_TAB_CLASSES.inactive}`}
          style={
            active === 'response'
              ? { color: 'var(--cloud-router-accent)', boxShadow: 'inset 0 2px 0 0 var(--cloud-router-accent)' }
              : undefined
          }
        >
          Response Body
        </button>
        {formatted ? (
          <span className="ml-auto mr-1 mb-1.5 select-none font-mono text-[10px] font-semibold tracking-wider text-slate-400 dark:text-slate-500">
            {language.toUpperCase()}
          </span>
        ) : null}
      </div>
      <div className="min-h-0 flex-1">
        {formatted ? (
          monacoReady ? (
            <Editor
              height="100%"
              language={language}
              value={formatted}
              theme={dark ? 'vs-dark' : 'vs'}
              options={BODY_EDITOR_OPTIONS}
              loading={null}
            />
          ) : (
            <div className="flex h-full items-center justify-center font-mono text-xs text-slate-400 dark:text-slate-500">
              Loading editor…
            </div>
          )
        ) : (
          <div className="flex h-full items-center justify-center text-xs text-slate-400 dark:text-slate-500">
            (no body captured)
          </div>
        )}
      </div>
    </div>
  );
}

/** JSON body viewer for the full redacted request/response bodies (kept for API compatibility). */
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
    <div className="flex h-full min-h-0 flex-col gap-4 text-sm">
      <dl className="shrink-0 grid grid-cols-2 gap-x-6 gap-y-2 text-xs">
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
      <RequestLogBodyTabs requestBody={detail.requestBody} responseBody={detail.responseBody} />
    </div>
  );
}
