import { Switch } from '@/components/ui/switch';
import { SettingRow } from '../setting-row';
import { useUpdatesSection } from './use-updates-section';

export function UpdatesSection() {
  const { betaUpdates, isLoading, setBetaUpdates } = useUpdatesSection();

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
      <SettingRow label="Versão instalada">
        <span className="font-mono text-small text-fg-muted">
          v{__APP_VERSION__}
        </span>
      </SettingRow>
    </>
  );
}
