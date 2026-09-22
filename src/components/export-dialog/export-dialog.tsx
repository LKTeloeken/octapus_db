import { memo } from 'react';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { SimpleDialog } from '@/components/ui/simple-dialog';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import type { ExportDialogProps, ExportFormat } from './export-dialog.types';
import { DELIMITERS, useExportDialog } from './use-export-dialog';

/**
 * Exportação do resultado da grade. Quando a grade já tem todas as linhas, a
 * serialização é feita direto no front; quando só tem uma página, o resultado
 * inteiro é buscado no banco antes de gravar o arquivo.
 */
export const ExportDialog = memo((props: ExportDialogProps) => {
  const { open, onOpenChange } = props;
  const {
    format,
    setFormat,
    formats,
    delimiter,
    setDelimiter,
    includeHeader,
    setIncludeHeader,
    summary,
    canCopy,
    copyBlockedReason,
    isExporting,
    isCopying,
    isBusy,
    exportNow,
    copyNow,
  } = useExportDialog(props);

  return (
    <SimpleDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Exportar resultados"
      description={summary}
      footer={
        <>
          <Button
            variant="outline"
            size="sm"
            disabled={isBusy}
            onClick={() => onOpenChange(false)}
          >
            Cancelar
          </Button>
          {canCopy && (
            <Button
              variant="outline"
              size="sm"
              disabled={isBusy || copyBlockedReason != null}
              title={copyBlockedReason ?? undefined}
              onClick={copyNow}
            >
              {isCopying && <Spinner className="h-3 w-3" />}
              {isCopying ? 'Copiando...' : 'Copiar'}
            </Button>
          )}
          <Button size="sm" disabled={isBusy} onClick={exportNow}>
            {isExporting && <Spinner className="h-3 w-3" />}
            {isExporting ? 'Exportando...' : 'Exportar'}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <div className="flex flex-col gap-2">
          <Label htmlFor="export-format">Formato</Label>
          <Select
            value={format}
            onValueChange={value => setFormat(value as ExportFormat)}
          >
            <SelectTrigger id="export-format" size="sm" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {formats.map(option => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {format === 'csv' && (
          <>
            <div className="flex flex-col gap-2">
              <Label htmlFor="export-delimiter">Separador</Label>
              <Select value={delimiter} onValueChange={setDelimiter}>
                <SelectTrigger
                  id="export-delimiter"
                  size="sm"
                  className="w-full"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {DELIMITERS.map(option => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="flex items-center justify-between">
              <Label htmlFor="export-header">Incluir cabeçalho</Label>
              <Switch
                id="export-header"
                checked={includeHeader}
                onCheckedChange={setIncludeHeader}
              />
            </div>
          </>
        )}
      </div>
    </SimpleDialog>
  );
});

ExportDialog.displayName = 'ExportDialog';
