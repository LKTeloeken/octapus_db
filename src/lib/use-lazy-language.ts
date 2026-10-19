import type { Extension } from '@codemirror/state';
import { useEffect, useState } from 'react';

/**
 * Linguagens do CodeMirror que só alguns editores usam e que pesam no bundle
 * (o parser de JavaScript sozinho tem ~75 KB): ficam num chunk à parte e
 * carregam na primeira vez que alguém precisa. SQL e JSON, que aparecem o
 * tempo todo, continuam no bundle principal.
 */
export type LazyLanguage = 'javascript' | 'xml' | 'html';

const loaders: Record<LazyLanguage, () => Promise<Extension>> = {
  // Shell do Mongo: JavaScript puro, sem JSX nem TypeScript
  javascript: () =>
    import('@codemirror/lang-javascript').then(m =>
      m.javascript({ jsx: false, typescript: false }),
    ),
  xml: () => import('@codemirror/lang-xml').then(m => m.xml()),
  html: () => import('@codemirror/lang-html').then(m => m.html()),
};

/** Já carregadas: a segunda vez é síncrona, sem piscar sem destaque */
const loaded = new Map<LazyLanguage, Extension>();
const pending = new Map<LazyLanguage, Promise<Extension>>();

export function loadLanguage(language: LazyLanguage): Promise<Extension> {
  const ready = loaded.get(language);
  if (ready) return Promise.resolve(ready);
  let promise = pending.get(language);
  if (!promise) {
    promise = loaders[language]().then(extension => {
      loaded.set(language, extension);
      pending.delete(language);
      return extension;
    });
    pending.set(language, promise);
  }
  return promise;
}

/**
 * A extensão da linguagem (`[]` enquanto carrega, ou sem linguagem): o editor
 * abre na hora e o destaque entra quando o chunk chega.
 */
export function useLazyLanguage(language: LazyLanguage | null): Extension {
  const [, setVersion] = useState(0);

  useEffect(() => {
    if (!language || loaded.has(language)) return;
    let alive = true;
    loadLanguage(language)
      .then(() => alive && setVersion(version => version + 1))
      .catch(error => console.error(`[codemirror] ${language}:`, error));
    return () => {
      alive = false;
    };
  }, [language]);

  return (language && loaded.get(language)) || [];
}
