import { Button } from '@/components/ui/button';
import { SettingRow } from '../setting-row';
import { useDiagnosticsSection } from './use-diagnostics-section';

export function DiagnosticsSection() {
  const { openLogs, isOpening, reloadInterface } = useDiagnosticsSection();

  return (
    <>
      <SettingRow
        label="Log do app"
        description="Conexões, sincronização do catálogo, backups e erros, para anexar a um relato de bug. Não guarda senhas nem o texto das suas queries."
      >
        <Button variant="outline" disabled={isOpening} onClick={openLogs}>
          Abrir pasta
        </Button>
      </SettingRow>
      <SettingRow
        label="Recarregar a interface"
        description="Se a tela parar de responder, recarrega a janela sem fechar o app. As abas voltam; alterações pendentes que não foram salvas se perdem."
      >
        <Button variant="outline" onClick={reloadInterface}>
          Recarregar
        </Button>
      </SettingRow>
    </>
  );
}
