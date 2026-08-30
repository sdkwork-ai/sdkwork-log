import { enMessages } from "./en-US/log/adminRequestLog/messages";
import { zhMessages } from "./zh-CN/log/adminRequestLog/messages";

export interface LogAdminI18nBundle {
  en: Record<string, string>;
  zh: Record<string, string>;
}

export const logAdminI18n: LogAdminI18nBundle = {
  en: enMessages,
  zh: zhMessages,
};