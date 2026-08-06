import type { FieldError } from './field-error';
import type { SdkWorkPlatformErrorCode } from './sdk-work-platform-error-code';

export interface ProblemDetail {
  code: SdkWorkPlatformErrorCode;
  detail?: string;
  errors?: FieldError[];
  /** Failing request occurrence as {METHOD} {routeTemplate}, with a redacted path fallback. */
  instance: string;
  /** Matched OpenAPI operation id; omitted only when no operation resolves. */
  operationId?: string;
  status: number;
  title: string;
  /** Server-owned request correlation id. */
  traceId: string;
  type: string;
}
