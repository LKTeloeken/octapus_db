import { Key01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input/input';
import { useLoginForm } from './use-login-form';

/** Entrada com o código de convite do participante (C-14) */
export function LoginForm() {
  const { inviteCode, setInviteCode, canSubmit, submit, isSubmitting, error } =
    useLoginForm();

  return (
    <div className="flex h-full items-center justify-center p-6">
      <form
        onSubmit={submit}
        className="flex w-full max-w-xs flex-col items-center gap-4 text-center"
      >
        <span className="flex size-11 items-center justify-center rounded-lg bg-hover text-fg-muted shadow-[inset_0_0_0_1px_var(--line-subtle)]">
          <HugeiconsIcon icon={Key01Icon} className="size-[22px]" />
        </span>
        <div className="flex flex-col gap-1">
          <p className="text-title font-semibold text-fg">
            Entrar no assistente
          </p>
          <p className="text-body text-fg-muted">
            Use o código de convite que você recebeu. Ele fica guardado só nesta
            máquina, cifrado.
          </p>
        </div>
        <Input
          aria-label="Código de convite"
          placeholder="Código de convite"
          value={inviteCode}
          onChange={event => setInviteCode(event.target.value)}
          error={!!error}
          helperText={error ?? undefined}
          autoFocus
          autoComplete="off"
          spellCheck={false}
          inputClassName="font-mono"
        />
        <Button type="submit" className="w-full" disabled={!canSubmit}>
          {isSubmitting ? 'Entrando…' : 'Entrar'}
        </Button>
      </form>
    </div>
  );
}
