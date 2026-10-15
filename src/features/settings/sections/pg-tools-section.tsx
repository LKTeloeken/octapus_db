import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import {
  missingPgTools,
  pgToolsInstallHint,
  pgToolsSourceLabel,
} from '@/lib/pg-tools';
import { SettingRow } from '../setting-row';
import { usePgToolsSection } from './use-pg-tools-section';

export function PgToolsSection() {
  const {
    configuredDir,
    selected,
    others,
    configuredError,
    isDetecting,
    detectError,
    isSaving,
    chooseDir,
    resetToAutomatic,
    setDir,
    redetect,
  } = usePgToolsSection();

  const missing = selected ? missingPgTools(selected) : [];

  return (
    <>
      <SettingRow
        label="Pasta das ferramentas"
        description={
          configuredDir ? (
            <span className="font-mono break-all">{configuredDir}</span>
          ) : (
            'Detecção automática: PATH e os locais de instalação comuns (Homebrew, Postgres.app, pacotes do sistema).'
          )
        }
      >
        <div className="flex items-center gap-1.5">
          {configuredDir && (
            <Button
              variant="ghost"
              disabled={isSaving}
              onClick={resetToAutomatic}
            >
              Automático
            </Button>
          )}
          <Button variant="outline" disabled={isSaving} onClick={chooseDir}>
            Escolher…
          </Button>
        </div>
      </SettingRow>

      <SettingRow
        label="Em uso"
        description={
          isDetecting && !selected && !configuredError ? (
            'Procurando o pg_dump…'
          ) : detectError ? (
            <span className="text-danger">{String(detectError)}</span>
          ) : configuredError ? (
            <span className="text-danger">{configuredError}</span>
          ) : selected ? (
            <span className="flex flex-col gap-1">
              <span>
                <span className="text-fg">pg_dump {selected.version}</span> ·{' '}
                {pgToolsSourceLabel(selected.source)}
              </span>
              <span className="font-mono break-all">{selected.dir}</span>
              {missing.length > 0 && (
                <span className="text-warning">
                  Sem {missing.join(' e ')} nesta pasta — a restauração
                  {missing.includes('pg_restore')
                    ? ' de arquivos .dump'
                    : ' de scripts .sql'}{' '}
                  não vai funcionar.
                </span>
              )}
            </span>
          ) : (
            <span className="text-danger">
              Nenhum pg_dump encontrado. {pgToolsInstallHint()}
            </span>
          )
        }
      >
        <Button variant="ghost" disabled={isDetecting} onClick={redetect}>
          {isDetecting && <Spinner className="size-3.5" />}
          Detectar de novo
        </Button>
      </SettingRow>

      {others.length > 0 && (
        <SettingRow
          label="Outras instalações"
          description="O backup precisa de um pg_dump da mesma versão do servidor ou mais novo."
        >
          <ul className="flex flex-col items-end gap-1">
            {others.map(install => (
              <li
                key={install.dir}
                className="flex items-center gap-2 text-small text-fg-muted"
              >
                <span title={install.dir}>pg_dump {install.version}</span>
                <Button
                  size="xs"
                  variant="ghost"
                  disabled={isSaving}
                  onClick={() => setDir(install.dir)}
                >
                  Usar
                </Button>
              </li>
            ))}
          </ul>
        </SettingRow>
      )}
    </>
  );
}
