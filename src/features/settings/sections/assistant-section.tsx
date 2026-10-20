import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { SettingRow } from '../setting-row';
import { useAssistantSection } from './use-assistant-section';

export function AssistantSection() {
  const {
    autoExecute,
    isLoading,
    setAutoExecute,
    configured,
    loggedIn,
    participantLabel,
    signOut,
    isSigningOut,
  } = useAssistantSection();

  return (
    <>
      <SettingRow
        htmlFor="settings-assistant-auto-execute"
        label="Executar consultas automaticamente"
        description="O assistente roda sozinho a consulta de leitura que gerou e mostra o resultado. Desligado, a consulta aparece para você revisar, editar e executar. Código que não é consulta nunca é executado."
      >
        <Switch
          id="settings-assistant-auto-execute"
          checked={autoExecute}
          disabled={isLoading}
          onCheckedChange={setAutoExecute}
        />
      </SettingRow>
      <SettingRow
        label="Participante"
        description={
          !configured
            ? 'Esta versão do Octapus não tem o assistente.'
            : loggedIn
              ? 'Sair apaga o acesso guardado nesta máquina. O histórico continua no servidor.'
              : 'Entre com o código de convite pelo painel do assistente.'
        }
      >
        {loggedIn ? (
          <div className="flex items-center gap-3">
            {participantLabel && (
              <span className="font-mono text-small text-fg-muted">
                {participantLabel}
              </span>
            )}
            <Button
              variant="outline"
              size="sm"
              onClick={signOut}
              disabled={isSigningOut}
            >
              Sair
            </Button>
          </div>
        ) : (
          <span className="text-small text-fg-subtle">Sem login</span>
        )}
      </SettingRow>
    </>
  );
}
