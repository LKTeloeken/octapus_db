import { Alert02Icon } from '@hugeicons/core-free-icons';
import { lazy, Suspense } from 'react';
import { Button } from '@/components/ui/button';
import { ContentState } from '@/components/ui/content-state';
import { Spinner } from '@/components/ui/spinner';
import { useConsentView } from './use-consent-view';

// O termo é a única tela com Markdown: o parser fica num chunk à parte
const Markdown = lazy(() => import('@/components/markdown/markdown'));

const loading = (
  <div className="flex h-full items-center justify-center">
    <Spinner className="size-4 text-fg-subtle" />
  </div>
);

/** Termo de consentimento antes do primeiro uso (RN-11) */
export function ConsentView() {
  const {
    term,
    isLoading,
    error,
    retry,
    acceptTerm,
    isAccepting,
    signOut,
    openLink,
  } = useConsentView();

  if (isLoading) return loading;

  if (error || !term) {
    return (
      <ContentState
        icon={Alert02Icon}
        tone="danger"
        title="Não consegui carregar o termo"
        description={error}
        action={
          <Button variant="outline" size="sm" onClick={retry}>
            Tentar de novo
          </Button>
        }
      />
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4 scrollbar-thin">
        <p className="mb-3 text-small text-fg-subtle">
          Antes de usar, leia o termo. A pergunta, a estrutura do banco e a
          consulta gerada são registradas no servidor para a pesquisa — os dados
          do seu banco não.
        </p>
        <Suspense fallback={loading}>
          <Markdown onOpenLink={openLink}>{term.body}</Markdown>
        </Suspense>
      </div>
      <div className="flex shrink-0 items-center justify-between gap-2 border-t border-line-subtle bg-surface-2 px-4 py-3">
        <span className="font-mono text-small text-fg-subtle">
          {term.version}
        </span>
        <div className="flex gap-2">
          <Button variant="ghost" size="sm" onClick={signOut}>
            Sair
          </Button>
          <Button size="sm" onClick={acceptTerm} disabled={isAccepting}>
            {isAccepting ? 'Aceitando…' : 'Li e aceito'}
          </Button>
        </div>
      </div>
    </div>
  );
}
