import {
  Alert02Icon,
  Cancel01Icon,
  CheckmarkCircle02Icon,
  InformationCircleIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import React, { useEffect } from 'react';
import {
  toast as hotToast,
  ToastBar,
  Toaster,
  useToasterStore,
  type Toast,
} from 'react-hot-toast';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/utils';

/** No máximo três na tela: o mais antigo sai quando chega o quarto */
const MAX_VISIBLE = 3;

/** Selo redondo com o ícone no tom do toast (DESIGN.md, Toast) */
function ToastSeal({ toast }: { toast: Toast }) {
  if (toast.type === 'loading') {
    return (
      <span className="flex size-[30px] shrink-0 items-center justify-center rounded-full bg-tinta-soft text-tinta-text">
        <Spinner className="size-4" />
      </span>
    );
  }
  const tone =
    toast.type === 'success'
      ? {
          icon: CheckmarkCircle02Icon,
          className: 'bg-success-soft text-success',
        }
      : toast.type === 'error'
        ? { icon: Alert02Icon, className: 'bg-danger-soft text-danger' }
        : { icon: InformationCircleIcon, className: 'bg-info-soft text-info' };
  return (
    <span
      className={cn(
        'flex size-[30px] shrink-0 items-center justify-center rounded-full',
        tone.className,
      )}
    >
      <HugeiconsIcon icon={tone.icon} className="size-4" />
    </span>
  );
}

function useToastLimit() {
  const { toasts } = useToasterStore();
  useEffect(() => {
    toasts
      .filter(item => item.visible)
      .slice(MAX_VISIBLE)
      .forEach(item => hotToast.dismiss(item.id));
  }, [toasts]);
}

/**
 * Toasts do Abissal (DESIGN.md, Toast): vidro, selo redondo no tom, texto e
 * fechar. Sucesso some em 4 s; erro fica até fechar. O estilo vai em `style`
 * porque o react-hot-toast aplica o próprio estilo inline por cima de classes.
 */
export function CustomToaster(): React.JSX.Element {
  useToastLimit();
  return (
    <Toaster
      position="bottom-right"
      // Acima da barra de status (32 px) e com o vão da janela
      containerStyle={{ bottom: 44, right: 12 }}
      reverseOrder={false}
      toastOptions={{
        duration: 4000,
        error: { duration: Infinity },
        style: {
          background: 'var(--glass)',
          backdropFilter: 'blur(20px) saturate(1.4)',
          WebkitBackdropFilter: 'blur(20px) saturate(1.4)',
          border: '1px solid var(--glass-border)',
          borderRadius: '12px',
          boxShadow: 'var(--elev-3), inset 0 1px 0 var(--glass-highlight)',
          color: 'var(--fg)',
          padding: '10px 8px 10px 12px',
          maxWidth: '380px',
          fontFamily:
            'var(--app-font-ui), ui-sans-serif, system-ui, sans-serif',
          fontSize: '13px',
          lineHeight: '20px',
        },
      }}
    >
      {toast => (
        <ToastBar toast={toast}>
          {({ message }) => (
            <div className="group/toast flex w-full items-start gap-2.5">
              <ToastSeal toast={toast} />
              <div className="min-w-0 flex-1 self-center [&>div]:m-0! [&>div]:justify-start!">
                {message}
              </div>
              {toast.type !== 'loading' && (
                <button
                  type="button"
                  aria-label="Fechar aviso"
                  onClick={() => hotToast.dismiss(toast.id)}
                  className="inline-flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-sm text-fg-subtle transition-colors hover:bg-hover hover:text-fg"
                >
                  <HugeiconsIcon icon={Cancel01Icon} className="size-3.5" />
                </button>
              )}
            </div>
          )}
        </ToastBar>
      )}
    </Toaster>
  );
}
