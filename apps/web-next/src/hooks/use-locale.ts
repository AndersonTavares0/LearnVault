import { useSyncExternalStore } from 'react';
import { getLocale, subscribeLocale, type Locale } from '@/lib/i18n';

/** Re-renders the caller when the locale changes, so `tr()` calls pick up the new language. */
export function useLocale(): Locale {
	return useSyncExternalStore(subscribeLocale, getLocale, getLocale);
}
