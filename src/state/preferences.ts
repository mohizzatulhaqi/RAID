import { useEffect, useState } from 'react';

export type ThemeChoice = 'system' | 'ledger' | 'crypt';

const dark = () => window.matchMedia('(prefers-color-scheme: dark)');
const reduced = () => window.matchMedia('(prefers-reduced-motion: reduce)');

/** Applies `data-theme` / `data-motion` on <html>. Theme follows the OS by default. */
export function usePreferences() {
  const [theme, setTheme] = useState<ThemeChoice>('system');
  const [reduceMotion, setReduceMotion] = useState<boolean>(() => reduced().matches);
  const [systemDark, setSystemDark] = useState<boolean>(() => dark().matches);

  useEffect(() => {
    const mq = dark();
    const onChange = () => setSystemDark(mq.matches);
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, []);

  useEffect(() => {
    const resolved = theme === 'system' ? (systemDark ? 'crypt' : 'ledger') : theme;
    document.documentElement.dataset.theme = resolved;
  }, [theme, systemDark]);

  useEffect(() => {
    document.documentElement.dataset.motion = reduceMotion ? 'reduced' : 'full';
  }, [reduceMotion]);

  return { theme, setTheme, reduceMotion, setReduceMotion };
}
