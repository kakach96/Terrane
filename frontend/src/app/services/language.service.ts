import { Injectable } from '@angular/core';
import { TranslateService } from '@ngx-translate/core';

export const SUPPORTED_LANGUAGES = ['zh-CN', 'en-US'] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

const LANG_STORAGE_KEY = 'terrane.lang';
const DEFAULT_LANG: SupportedLanguage = 'zh-CN';

/** Normalize a language tag to one of the supported languages, or null. */
function normalizeLang(lang: string): SupportedLanguage | null {
  const lower = lang.toLowerCase();
  if (lower.startsWith('zh')) {
    return 'zh-CN';
  }
  if (lower.startsWith('en')) {
    return 'en-US';
  }
  return null;
}

/**
 * Detect the current UI language from localStorage (set by the switcher) with a
 * browser-language fallback. Free of DI so it can be used in HTTP interceptors
 * without creating a circular dependency.
 */
export function detectLanguage(): SupportedLanguage {
  const stored = localStorage.getItem(LANG_STORAGE_KEY);
  if (stored) {
    const normalized = normalizeLang(stored);
    if (normalized) {
      return normalized;
    }
  }
  const browser = navigator.language || navigator.languages?.[0] || '';
  return normalizeLang(browser) ?? DEFAULT_LANG;
}

@Injectable({ providedIn: 'root' })
export class LanguageService {
  constructor(private translate: TranslateService) {
    this.translate.addLangs([...SUPPORTED_LANGUAGES]);
    this.translate.setFallbackLang(DEFAULT_LANG);
    this.init();
  }

  /** Current active language (zh-CN / en-US). */
  get currentLang(): SupportedLanguage {
    return (
      this.normalize(this.translate.currentLang() || this.translate.getFallbackLang()) ??
      DEFAULT_LANG
    );
  }

  private normalize(lang: string | null): SupportedLanguage | null {
    return lang ? normalizeLang(lang) : null;
  }

  /** Initialise the active language (called once from the constructor). */
  private init(): void {
    this.setLanguage(detectLanguage());
  }

  /** Switch the active language and persist the preference. */
  setLanguage(lang: SupportedLanguage): void {
    this.translate.use(lang);
    localStorage.setItem(LANG_STORAGE_KEY, lang);
    document.documentElement.lang = lang;
  }

  /** Toggle between the two supported languages. */
  toggle(): void {
    this.setLanguage(this.currentLang === 'zh-CN' ? 'en-US' : 'zh-CN');
  }
}
