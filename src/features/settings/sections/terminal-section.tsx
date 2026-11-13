import { Button } from '@/components/ui/button';
import { SettingRow } from '../setting-row';
import { useTerminalSection } from './use-terminal-section';

export function TerminalSection() {
  const { status, directory, install, isInstalling } = useTerminalSection();

  const description = !status
    ? 'Verificando…'
    : !status.supported
      ? 'Ainda não há um octapus publicado para este sistema.'
      : status.installed
        ? `Instalado em ${status.path}. O botão baixa de novo a versão deste app.`
        : `Baixa o octapus desta versão do app e o instala em ${status.path}, sem pedir senha de administrador.`;

  return (
    <>
      <SettingRow label="octapus no terminal" description={description}>
        <Button
          variant="outline"
          disabled={!status?.supported || isInstalling}
          onClick={install}
        >
          {isInstalling
            ? 'Instalando…'
            : status?.installed
              ? 'Reinstalar'
              : 'Instalar'}
        </Button>
      </SettingRow>
      {status?.installed && !status.onPath && (
        <p className="pt-3 text-small text-fg-muted">
          A pasta <code className="font-mono text-fg">{directory}</code> não
          está no <code className="font-mono text-fg">PATH</code>: acrescente-a
          na configuração do seu shell para chamar só{' '}
          <code className="font-mono text-fg">octapus</code>.
        </p>
      )}
    </>
  );
}
