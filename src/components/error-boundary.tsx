import { Alert02Icon } from '@hugeicons/core-free-icons';
import { Component, type ErrorInfo, type ReactNode } from 'react';
import { Button } from '@/components/ui/button';
import { ContentState } from '@/components/ui/content-state';

interface ErrorBoundaryProps {
  children: ReactNode;
  /** Onde fica o limite, já com a preposição ("nesta aba", "no app") — no
   *  log e no título do estado de erro */
  area: string;
  /** Trocar qualquer um destes valores limpa o erro (ex.: o id da aba) */
  resetKeys?: readonly unknown[];
  /** Mostra também "Recarregar o app" (o limite mais externo) */
  offerReload?: boolean;
  className?: string;
}

interface ErrorBoundaryState {
  error: Error | null;
}

const sameKeys = (a: readonly unknown[] = [], b: readonly unknown[] = []) =>
  a.length === b.length && a.every((value, i) => Object.is(value, b[i]));

/**
 * Um erro de render fica preso nesta área em vez de apagar a janela inteira:
 * no lugar dela entra o estado de erro (DESIGN.md §8) com "Tentar de novo".
 * Erro em evento ou em Promise não passa por aqui — esses viram toast.
 */
export class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(`[${this.props.area}]`, error, info.componentStack);
  }

  componentDidUpdate(previous: ErrorBoundaryProps) {
    if (
      this.state.error &&
      !sameKeys(previous.resetKeys, this.props.resetKeys)
    ) {
      this.reset();
    }
  }

  reset = () => this.setState({ error: null });

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;

    return (
      <ContentState
        tone="danger"
        icon={Alert02Icon}
        className={this.props.className}
        title={`Algo deu errado ${this.props.area}`}
        description={
          <span className="font-mono text-small break-all">
            {error.message || String(error)}
          </span>
        }
        action={
          <>
            <Button variant="outline" onClick={this.reset}>
              Tentar de novo
            </Button>
            {this.props.offerReload && (
              <Button variant="ghost" onClick={() => window.location.reload()}>
                Recarregar o app
              </Button>
            )}
          </>
        }
      />
    );
  }
}
