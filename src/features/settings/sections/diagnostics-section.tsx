import { Button } from '@/components/ui/button';
import { SettingRow } from '../setting-row';
import { useDiagnosticsSection } from './use-diagnostics-section';

export function DiagnosticsSection() {
  const { openLogs, isOpening } = useDiagnosticsSection();

  return (
    <SettingRow
      label="Log do app"
      description="Conexões, sincronização do catálogo, backups e erros, para anexar a um relato de bug. Não guarda senhas nem o texto das suas queries."
    >
      <Button variant="outline" disabled={isOpening} onClick={openLogs}>
        Abrir pasta
      </Button>
    </SettingRow>
  );
}
