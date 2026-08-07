//! PC admin request log management page: filter bar + list + detail drawer.

import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  formatLogTimestamp,
  RequestLogDetailPanel,
  RequestLogService,
  type LogApiSurface,
  type RequestLogDetail,
  type RequestLogItem,
} from '@sdkwork/log-pc-request-log';

const PAGE_SIZE_OPTIONS = [10, 20, 50];
const SURFACE_OPTIONS: LogApiSurface[] = ['open-api', 'app-api', 'backend-api', 'internal-api', 'gateway-api', 'unknown'];
const METHOD_OPTIONS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS', 'HEAD'];

export function RequestLogAdmin(): React.ReactElement {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en-US';

  const [items, setItems] = useState<RequestLogItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);

  const [traceIdFilter, setTraceIdFilter] = useState('');
  const [requestIdFilter, setRequestIdFilter] = useState('');
  const [surfaceFilter, setSurfaceFilter] = useState<LogApiSurface | ''>('');
  const [methodFilter, setMethodFilter] = useState('');
  const [statusFilter, setStatusFilter] = useState('');

  const [selected, setSelected] = useState<RequestLogDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const result = await RequestLogService.list({
        traceId: traceIdFilter || undefined,
        requestId: requestIdFilter || undefined,
        apiSurface: surfaceFilter || undefined,
        method: methodFilter || undefined,
        status: statusFilter ? Number(statusFilter) : undefined,
        page,
        pageSize,
      });
      setItems(result.items);
      setTotal(result.total);
    } catch (error) {
      setLoadError(error instanceof Error ? error.message : 'Failed to load request logs');
    } finally {
      setLoading(false);
    }
  }, [traceIdFilter, requestIdFilter, surfaceFilter, methodFilter, statusFilter, page, pageSize]);

  useEffect(() => {
    void load();
  }, [load]);

  const handleSearch = () => {
    if (page !== 1) {
      setPage(1);
      return;
    }
    void load();
  };

  const handleReset = () => {
    setTraceIdFilter('');
    setRequestIdFilter('');
    setSurfaceFilter('');
    setMethodFilter('');
    setStatusFilter('');
    if (page !== 1) {
      setPage(1);
      return;
    }
    void load();
  };

  const openDetail = async (item: RequestLogItem) => {
    setDetailLoading(true);
    try {
      const detail = await RequestLogService.detail(item.id);
      setSelected(detail);
    } catch (error) {
      setLoadError(error instanceof Error ? error.message : 'Failed to load request log detail');
    } finally {
      setDetailLoading(false);
    }
  };

  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const firstRow = items.length > 0 ? (page - 1) * pageSize + 1 : 0;
  const lastRow = items.length > 0 ? (page - 1) * pageSize + items.length : 0;

  const filterInputClass =
    'min-w-0 flex-1 bg-slate-50 dark:bg-[#121212] border border-slate-200 dark:border-white/10 px-3 py-2 rounded-lg text-sm focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/20 text-slate-800 dark:text-white placeholder:text-slate-400 dark:placeholder:text-slate-500';
  const filterSelectClass =
    'min-w-0 flex-1 appearance-none bg-slate-50 dark:bg-[#121212] border border-slate-200 dark:border-white/10 py-2 pl-3 pr-8 rounded-lg text-sm focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/20 text-slate-800 dark:text-white bg-no-repeat bg-[right_0.6rem_center] bg-[length:1rem_1rem] bg-[url("data:image/svg+xml,%3Csvg xmlns=%27http://www.w3.org/2000/svg%27 viewBox=%270 0 20 20%27 fill=%27%2394a3b8%27%3E%3Cpath fill-rule=%27evenodd%27 d=%27M5.23 7.21a.75.75 0 0 1 1.06.02L10 11.17l3.71-3.94a.75.75 0 1 1 1.08 1.04l-4.25 4.5a.75.75 0 0 1-1.08 0l-4.25-4.5a.75.75 0 0 1 .02-1.06Z%27 clip-rule=%27evenodd%27/%3E%3C/svg%3E")]';

  return (
    <div className="flex h-full min-h-0 w-full flex-col gap-4 overflow-hidden">
      <div className="shrink-0">
        <h1 className="text-lg font-semibold text-slate-800 dark:text-white">{t('admin.requestLog.title')}</h1>
        <p className="text-xs text-slate-500 dark:text-slate-400">{t('admin.requestLog.subtitle')}</p>
      </div>

      {/* Filter Bar — single row, fields share the available width evenly */}
      <div className="shrink-0 bg-white dark:bg-[#1a1a1a] border border-slate-200 dark:border-white/10 rounded-xl p-3 shadow-sm flex items-center gap-2">
        <input
          type="text"
          value={traceIdFilter}
          onChange={(event) => setTraceIdFilter(event.target.value)}
          onKeyDown={(event) => event.key === 'Enter' && handleSearch()}
          placeholder={t('admin.requestLog.filter.traceId')}
          className={`${filterInputClass} flex-[2]`}
        />
        <input
          type="text"
          value={requestIdFilter}
          onChange={(event) => setRequestIdFilter(event.target.value)}
          onKeyDown={(event) => event.key === 'Enter' && handleSearch()}
          placeholder={t('admin.requestLog.filter.requestId')}
          className={`${filterInputClass} flex-[2]`}
        />
        <select
          value={surfaceFilter}
          onChange={(event) => setSurfaceFilter(event.target.value as LogApiSurface | '')}
          className={filterSelectClass}
        >
          <option value="">{t('admin.requestLog.filter.apiSurface')}: {t('admin.requestLog.filter.all')}</option>
          {SURFACE_OPTIONS.map((surface) => (
            <option key={surface} value={surface}>
              {surface}
            </option>
          ))}
        </select>
        <select
          value={methodFilter}
          onChange={(event) => setMethodFilter(event.target.value)}
          className={filterSelectClass}
        >
          <option value="">{t('admin.requestLog.filter.method')}: {t('admin.requestLog.filter.all')}</option>
          {METHOD_OPTIONS.map((method) => (
            <option key={method} value={method}>
              {method}
            </option>
          ))}
        </select>
        <input
          type="number"
          min={100}
          max={599}
          value={statusFilter}
          onChange={(event) => setStatusFilter(event.target.value)}
          onKeyDown={(event) => event.key === 'Enter' && handleSearch()}
          placeholder={t('admin.requestLog.filter.status')}
          className={filterInputClass}
        />
        <div className="ml-auto flex shrink-0 items-center gap-2">
          <button
            onClick={handleSearch}
            className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-sm font-medium transition-colors"
          >
            {t('admin.requestLog.action.search')}
          </button>
          <button
            onClick={handleReset}
            className="px-4 py-2 bg-slate-50 dark:bg-white/5 hover:bg-slate-100 dark:hover:bg-white/10 text-slate-600 dark:text-slate-300 rounded-lg text-sm font-medium transition-colors border border-slate-200 dark:border-white/10"
          >
            {t('admin.requestLog.action.reset')}
          </button>
        </div>
      </div>

      {/* List */}
      <div className="flex-1 min-h-0 overflow-auto rounded-xl border border-slate-200 dark:border-white/10 bg-white dark:bg-[#1a1a1a]">
        <table className="w-full text-left text-sm whitespace-nowrap">
          <thead className="sticky top-0 z-10 bg-slate-50 dark:bg-[#121212] text-slate-500 dark:text-slate-400 border-b border-slate-200 dark:border-white/10 select-none text-xs uppercase font-semibold">
            <tr>
              <th className="px-4 py-3 font-medium">{t('admin.requestLog.col.time')}</th>
              <th className="px-4 py-3 font-medium">{t('admin.requestLog.col.surface')}</th>
              <th className="px-4 py-3 font-medium">{t('admin.requestLog.col.method')}</th>
              <th className="px-4 py-3 font-medium">{t('admin.requestLog.col.path')}</th>
              <th className="px-4 py-3 font-medium">{t('admin.requestLog.col.status')}</th>
              <th className="px-4 py-3 font-medium">{t('admin.requestLog.col.duration')}</th>
              <th className="px-4 py-3 font-medium">{t('admin.requestLog.col.service')}</th>
              <th className="px-4 py-3 font-medium">{t('admin.requestLog.col.traceId')}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-100 dark:divide-white/5 text-slate-700 dark:text-slate-300 text-xs">
            {loading ? (
              <tr>
                <td colSpan={8} className="px-4 py-8 text-center text-slate-400">
                  {t('admin.requestLog.state.loading')}
                </td>
              </tr>
            ) : loadError ? (
              <tr>
                <td colSpan={8} className="px-4 py-8 text-center">
                  <div className="text-rose-600 dark:text-rose-400">{t('admin.requestLog.state.error')}</div>
                  <div className="mt-1 text-slate-400">{loadError}</div>
                  <button
                    onClick={() => void load()}
                    className="mt-3 px-4 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-medium"
                  >
                    {t('admin.requestLog.action.retry')}
                  </button>
                </td>
              </tr>
            ) : items.length === 0 ? (
              <tr>
                <td colSpan={8} className="px-4 py-8 text-center text-slate-400">
                  {t('admin.requestLog.state.empty')}
                  <div className="mt-1 text-xs text-slate-500">{t('admin.requestLog.state.emptyDesc')}</div>
                </td>
              </tr>
            ) : (
              items.map((item) => (
                <tr
                  key={item.id}
                  onClick={() => void openDetail(item)}
                  className="cursor-pointer hover:bg-slate-50 dark:hover:bg-white/[0.02]"
                >
                  <td className="px-4 py-2.5 font-mono">{formatLogTimestamp(item.createdAt, locale)}</td>
                  <td className="px-4 py-2.5 font-mono">{item.apiSurface}</td>
                  <td className="px-4 py-2.5 font-mono">{item.method}</td>
                  <td className="px-4 py-2.5 font-mono max-w-[320px] truncate" title={item.path}>
                    {item.path}
                  </td>
                  <td className="px-4 py-2.5">
                    <span
                      className={`inline-flex px-2 py-0.5 rounded-full border font-medium ${
                        item.statusCode !== null && item.statusCode !== undefined && item.statusCode >= 400
                          ? 'bg-rose-50 dark:bg-rose-500/10 text-rose-600 dark:text-rose-400 border-rose-200 dark:border-rose-500/20'
                          : 'bg-emerald-50 dark:bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-200 dark:border-emerald-500/20'
                      }`}
                    >
                      {item.statusCode ?? '-'}
                    </span>
                  </td>
                  <td className="px-4 py-2.5 font-mono">{item.durationMs ? `${item.durationMs} ms` : '-'}</td>
                  <td className="px-4 py-2.5 font-mono">{item.service ?? '-'}</td>
                  <td className="px-4 py-2.5 font-mono max-w-[200px] truncate" title={item.traceId}>
                    {item.traceId}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div className="shrink-0 flex items-center justify-between text-xs text-slate-500">
        <span>
          {t('admin.requestLog.pagination.showing', {
            first: firstRow,
            last: lastRow,
            total,
          })}
        </span>
        <div className="flex items-center gap-2">
          <button
            onClick={() => setPage((current) => Math.max(1, current - 1))}
            disabled={page <= 1 || loading}
            className="px-2.5 py-1 rounded border border-slate-200 dark:border-white/10 disabled:opacity-40"
          >
            ‹
          </button>
          <span className="min-w-7 text-center font-medium">{page}</span>
          <button
            onClick={() => setPage((current) => Math.min(totalPages, current + 1))}
            disabled={page >= totalPages || loading}
            className="px-2.5 py-1 rounded border border-slate-200 dark:border-white/10 disabled:opacity-40"
          >
            ›
          </button>
          <select
            value={pageSize}
            onChange={(event) => {
              setPageSize(Number(event.target.value));
              setPage(1);
            }}
            className="ml-2 rounded border border-slate-200 dark:border-white/10 bg-white dark:bg-[#1a1a1a] px-2 py-1"
          >
            {PAGE_SIZE_OPTIONS.map((size) => (
              <option key={size} value={size}>
                {t('admin.requestLog.pagination.perPage', { size })}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Detail Drawer */}
      {selected && (
        <div className="fixed inset-0 z-50 flex justify-end bg-black/40" onClick={() => setSelected(null)}>
          <div
            className="flex h-full w-full max-w-4xl flex-col overflow-hidden bg-white dark:bg-[#1a1a1a] p-5 shadow-2xl"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="mb-4 flex shrink-0 items-center justify-between">
              <h2 className="text-base font-semibold text-slate-800 dark:text-white">
                {t('admin.requestLog.detail.title')}
              </h2>
              <button
                onClick={() => setSelected(null)}
                className="rounded-lg bg-slate-100 dark:bg-white/5 px-3 py-1 text-sm text-slate-600 dark:text-slate-300"
              >
                {t('admin.requestLog.detail.close')}
              </button>
            </div>
            {detailLoading ? (
              <div className="flex min-h-0 flex-1 items-center justify-center py-8 text-sm text-slate-400">
                {t('admin.requestLog.state.loading')}
              </div>
            ) : (
              <div className="min-h-0 flex-1">
                <RequestLogDetailPanel detail={selected} locale={locale} />
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
