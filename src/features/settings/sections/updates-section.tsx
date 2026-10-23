import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { SettingRow } from '../setting-row';
import { useUpdatesSection } from './use-updates-section';

export function UpdatesSection() {
  const {
    betaUpdates,
    isLoading,
    setBetaUpdates,
    status,
    statusText,
    checking,
    checkNow,
    install,
    restart,
  } = useUpdatesSection();

  return (
    <>
      <SettingRow
        htmlFor="settings-beta-updates"
        label="Participar das versões beta"
        description="Recebe também as pre-releases, com novidades antes da versão estável — e mais chance de bugs. Desligar não volta a versão já instalada: a próxima estável chega quando for mais nova."
      >
        <Switch
          id="settings-beta-updates"
          checked={betaUpdates}
          disabled={isLoading}
          onCheckedChange={setBetaUpdates}
        />
      </SettingRow>
      <SettingRow label="Versão nova" description={statusText}>
        {status === 'available' ? (
          <Button onClick={install}>Atualizar agora</Button>
        ) : status === 'ready' ? (
          <Button onClick={restart}>Reiniciar</Button>
        ) : (
          <Button
            variant="outline"
            disabled={checking || isLoading || status === 'downloading'}
            onClick={checkNow}
          >
            {checking && <Spinner className="size-3.5" />}
            Verificar agora
          </Button>
        )}
      </SettingRow>
      <SettingRow label="Versão instalada">
        <span className="font-mono text-small text-fg-muted">
          v{__APP_VERSION__}
        </span>
      </SettingRow>
    </>
  );
}
