/** SSR-safe default copy — refreshed onMount after Host locale is known. */
import { withLocaleHint } from "./locale-copy.js";

export const DEFAULT_LOCALE = "zh-hans";

export function bootLocaleCopy<T>(localeId: string, fn: () => T): T {
    return withLocaleHint(localeId, fn);
}
