import {
  ArrowRight01Icon,
  FileImportIcon,
  Link01Icon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import type { ImportSourceInfo } from '@/api/types/import.types';
import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/utils';
import {
  SOURCE_FILE_HINTS,
  SOURCE_LABELS,
  sourceSummary,
} from './import-labels';

interface SourceStepProps {
  sources: ImportSourceInfo[];
  isDetecting: boolean;
  detectError: Error | null;
  isReading: boolean;
  readingSource: string | undefined;
  readSource: (info: ImportSourceInfo) => void;
  chooseFile: (info: ImportSourceInfo) => void;
  urls: string;
  setUrls: (value: string) => void;
  readUrls: () => void;
}

/** Passo 1: de onde importar — apps detectados, arquivo escolhido ou URLs */
export function SourceStep({
  sources,
  isDetecting,
  detectError,
  isReading,
  readingSource,
  readSource,
  chooseFile,
  urls,
  setUrls,
  readUrls,
}: SourceStepProps) {
  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-col gap-1">
        {isDetecting && (
          <div className="flex items-center gap-2 px-2 py-3 text-small text-fg-subtle">
            <Spinner className="size-3.5" />
            Procurando apps nesta máquina…
          </div>
        )}
        {detectError && (
          <p className="px-2 py-3 text-small text-danger">
            {detectError.message}
          </p>
        )}
        {sources.map(info => {
          const available = !!info.path && info.count > 0;
          const isThisReading = isReading && readingSource === info.source;
          return (
            <div
              key={info.source}
              className={cn(
                'group flex items-center gap-3 rounded-md px-2.5 py-2 transition-colors',
                available && 'hover:bg-hover',
              )}
            >
              <button
                type="button"
                disabled={!available || isReading}
                onClick={() => readSource(info)}
                className="flex min-w-0 flex-1 cursor-pointer flex-col items-start gap-0.5 text-left outline-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-ring disabled:cursor-default"
              >
                <span
                  className={cn(
                    'text-body font-medium',
                    available ? 'text-fg' : 'text-fg-muted',
                  )}
                >
                  {SOURCE_LABELS[info.source]}
                </span>
                <span className="truncate text-small text-fg-subtle">
                  {sourceSummary(info)}
                </span>
              </button>
              <Button
                variant="ghost"
                size="xs"
                disabled={isReading}
                title={SOURCE_FILE_HINTS[info.source]}
                onClick={() => chooseFile(info)}
              >
                <HugeiconsIcon icon={FileImportIcon} />
                Arquivo…
              </Button>
              <span className="flex size-4 shrink-0 items-center justify-center">
                {isThisReading ? (
                  <Spinner className="size-3.5 text-fg-subtle" />
                ) : (
                  available && (
                    <HugeiconsIcon
                      icon={ArrowRight01Icon}
                      className="size-4 text-fg-subtle"
                    />
                  )
                )}
              </span>
            </div>
          );
        })}
      </div>

      {/* URLs: o caminho para quem não usa nenhum dos apps acima */}
      <div className="flex flex-col gap-2 border-t border-line-subtle pt-4">
        <label
          htmlFor="import-urls"
          className="flex items-center gap-1.5 text-body font-medium text-fg"
        >
          <HugeiconsIcon icon={Link01Icon} className="size-4 text-fg-subtle" />
          Colar URLs de conexão
        </label>
        <textarea
          id="import-urls"
          rows={3}
          spellCheck={false}
          placeholder={
            'postgres://usuario:senha@host:5432/banco\nmongodb+srv://usuario:senha@cluster.exemplo.net\nredis://host:6379'
          }
          value={urls}
          onChange={event => setUrls(event.target.value)}
          className="w-full resize-y rounded-sm border border-line bg-field p-2 font-mono text-small text-fg outline-none placeholder:text-fg-subtle focus:border-iris-text focus:ring-3 focus:ring-iris-soft"
        />
        <div className="flex items-center justify-between gap-2">
          <span className="text-small text-fg-subtle">
            Uma por linha. Caminho de arquivo .db vira SQLite.
          </span>
          <Button
            variant="secondary"
            size="sm"
            disabled={!urls.trim() || isReading}
            onClick={readUrls}
          >
            {isReading && readingSource === 'urls' ? 'Lendo…' : 'Ler URLs'}
          </Button>
        </div>
      </div>
    </div>
  );
}
