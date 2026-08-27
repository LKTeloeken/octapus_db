import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input/input';
import { SimpleAlertDialog } from '@/components/ui/simple-alert-dialog';
import { SimpleDialog } from '@/components/ui/simple-dialog';
import { Typography } from '@/components/ui/typography';
import { useVaultUnlock } from './use-vault-unlock';

/**
 * Pedido da senha mestre. Não aparece no boot: a lista de servidores abre sem a
 * chave, e o diálogo só surge quando alguma operação esbarra no cofre trancado.
 */
export function VaultUnlockDialog() {
  const {
    isOpen,
    close,
    password,
    setPassword,
    isSubmitting,
    canSubmit,
    isResetOpen,
    setResetOpen,
    handleUnlock,
    handleReset,
  } = useVaultUnlock();

  return (
    <>
      <SimpleDialog
        open={isOpen}
        onOpenChange={close}
        title="Destravar o cofre"
        description="Digite a senha mestre para usar as senhas salvas nesta sessão."
        footer={
          <>
            <Button variant="ghost" onClick={() => setResetOpen(true)}>
              Esqueci a senha
            </Button>
            <Button disabled={!canSubmit} onClick={handleUnlock}>
              {isSubmitting ? 'Verificando…' : 'Destravar'}
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-3">
          <Input
            type="password"
            label="Senha mestre"
            placeholder="Digite aqui..."
            value={password}
            onChange={e => setPassword(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && canSubmit) handleUnlock();
            }}
          />
          <Typography variant="p" className="text-xs text-muted-foreground">
            A senha é pedida uma vez por sessão, não a cada conexão.
          </Typography>
        </div>
      </SimpleDialog>

      <SimpleAlertDialog
        open={isResetOpen}
        onOpenChange={setResetOpen}
        title="Esqueceu a senha mestre?"
        description="Não existe recuperação — é disso que vem a proteção. A única saída é apagar as senhas e URIs guardadas de todos os servidores e cadastrá-las de novo. Os servidores em si continuam na lista."
        acceptLabel="Apagar e recomeçar"
        cancelLabel="Cancelar"
        onAccept={handleReset}
      />
    </>
  );
}
