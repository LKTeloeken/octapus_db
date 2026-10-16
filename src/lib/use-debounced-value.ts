import { useEffect, useState } from 'react';

/**
 * O valor só muda depois de `delayMs` sem mudanças (prévias que vão ao
 * backend). Compara pelo JSON: um objeto montado a cada render, com o mesmo
 * conteúdo, não reinicia a espera.
 */
export function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  const key = JSON.stringify(value);

  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(timer);
    // `key` representa `value`: só o conteúdo importa
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, delayMs]);

  return debounced;
}
