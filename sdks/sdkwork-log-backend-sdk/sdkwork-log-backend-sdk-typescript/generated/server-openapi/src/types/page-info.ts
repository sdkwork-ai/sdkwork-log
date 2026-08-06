export interface PageInfo {
  hasMore?: boolean;
  mode: 'offset' | 'cursor';
  nextCursor?: string | null;
  page?: number;
  pageSize?: number;
  totalItems?: string;
  totalPages?: number;
}
