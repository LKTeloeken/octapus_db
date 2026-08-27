import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input/input';
import { SimpleAlertDialog } from '@/components/ui/simple-alert-dialog';
import { Typography } from '@/components/ui/typography';
import { useSecuritySection } from './use-security-section';

export function SecuritySection() {
  const {
    hasMasterPassword,
    isLocked,
    isBusy,
    password,
    confirm,
    mismatch,
    canEnable,
    canDisable,
    isResetOpen,
    setPassword,
    setConfirm,
    setResetOpen,
    handleEnable,
    handleDisable,
    handleLock,
    handleReset,
  } = useSecuritySection();

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-1">
        <Typography variant="p" className="font-semibold">
          Senha mestre
        </Typography>
        <Typography variant="p" className="text-xs text-muted-foreground">
          Sem ela, as senhas dos servidores ficam criptografadas com uma chave
          guardada neste computador — o que protege contra o banco ser copiado
          sozinho, mas não contra quem tem acesso a esta máquina. Com ela, a
          chave passa a depender de um segredo que só você sabe.
        </Typography>
      </div>

      {!hasMasterPassword && (
        <div className="flex flex-col gap-3">
          <Input
            type="password"
            label="Senha mestre"
            placeholder="Mínimo de 8 caracteres"
            value={password}
            onChange={e => setPassword(e.target.value)}
          />
          <Input
            type="password"
            label="Confirme a senha"
            placeholder="Digite de novo"
            value={confirm}
            error={mismatch}
            helperText={mismatch ? 'As senhas não conferem.' : undefined}
            onChange={e => setConfirm(e.target.value)}
          />

          <Typography variant="p" className="text-xs text-muted-foreground">
            Não há recuperação: se esquecer, as senhas guardadas se perdem e
            precisam ser cadastradas de novo. Ativar é instantâneo — nada é
            recriptografado.
          </Typography>

          <div className="flex justify-end">
            <Button disabled={!canEnable} onClick={handleEnable}>
              Ativar senha mestre
            </Button>
          </div>
        </div>
      )}

      {hasMasterPassword && (
        <div className="flex flex-col gap-3">
          <Typography variant="p" className="text-xs text-muted-foreground">
            {isLocked
              ? 'O cofre está trancado nesta sessão.'
              : 'O cofre está destravado nesta sessão.'}
          </Typography>

          {!isLocked && (
            <div className="flex justify-start">
              <Button variant="outline" onClick={handleLock}>
                Trancar agora
              </Button>
            </div>
          )}

          <Input
            type="password"
            label="Senha mestre atual"
            placeholder="Necessária para desativar"
            value={password}
            onChange={e => setPassword(e.target.value)}
          />
          <div className="flex justify-end">
            <Button
              variant="outline"
              disabled={!canDisable}
              onClick={handleDisable}
            >
              Desativar senha mestre
            </Button>
          </div>
        </div>
      )}

      <div className="flex flex-col gap-2 border-t border-border pt-4">
        <Typography variant="p" className="font-semibold">
          Recomeçar do zero
        </Typography>
        <Typography variant="p" className="text-xs text-muted-foreground">
          Descarta a chave do cofre e apaga todas as senhas e URIs guardadas. Os
          servidores continuam cadastrados, mas as credenciais precisam ser
          digitadas de novo. É o único caminho para quem esqueceu a senha mestre.
        </Typography>
        <div className="flex justify-start">
          <Button
            variant="destructive"
            disabled={isBusy}
            onClick={() => setResetOpen(true)}
          >
            Apagar senhas e recriar o cofre
          </Button>
        </div>
      </div>

      <SimpleAlertDialog
        open={isResetOpen}
        onOpenChange={setResetOpen}
        title="Apagar todas as senhas guardadas?"
        description="Esta ação não pode ser desfeita. As senhas e URIs de conexão de todos os servidores serão apagadas e você precisará cadastrá-las de novo."
        acceptLabel="Apagar tudo"
        cancelLabel="Cancelar"
        onAccept={handleReset}
      />
    </div>
  );
}
