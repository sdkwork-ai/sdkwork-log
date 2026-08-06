//! sdkwork-log-h5-request-log: reusable H5 surface for SDKWork request logs.
//!
//! Shares the same typed client/service contract as the PC surface
//! (`@sdkwork/log-pc-request-log`) with lightweight mobile-first components.

export {
  getLogBackendSdkClient,
  configureLogBackendSdkClient,
  resetLogBackendSdkClient,
} from './client';
export { RequestLogService } from './requestLogService';
export { RequestLogCardList, RequestLogDetailSheet, formatLogTimestamp } from './components';
export type {
  LogApiSurface,
  RequestLogItem,
  RequestLogDetail,
  RequestLogListFilters,
  RequestLogPage,
} from './types';
