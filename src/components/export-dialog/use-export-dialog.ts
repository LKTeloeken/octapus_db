import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { save } from '@tauri-apps/plugin-dialog';
import { useCallback, useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import { useWriteExportFile } from '@/queries/use-write-export-file';
import type { ExportDialogProps, ExportFormat } from './export-dialog.types';
import { EXTENSIONS, quoteIdent, serializeRows } from './serialize';

const FORMATS: { value: ExportFormat; label: string }[] = [
  { value: 'csv', label: 'CSV' },
  { value: 'json', label: 'JSON' },
  { value: 'sql', label: 'SQL (INSERT)' },
];

export const DELIMITERS = [
  { value: ',', label: 'Vírgula  ,' },
  { value: ';', label: 'Ponto e vírgula  ;' },
  { value: '\t', label: 'Tabulação' },
];

/** Formatos que se colam em outro lugar: uma migration, um script, um ticket */
const CLIPBOARD_FORMATS: ExportFormat[] = ['json', 'sql'];

/** Acima disso a área de transferência deixa de ser um destino razoável */
const CLIPBOARD_ROW_LIMIT = 5000;

export const useExportDialog = ({
  columns,
  rows,
  hasMore,
  totalCount,
  fileName,
  editableInfo,
  onFetchAllRows,
  onOpenChange,
}: ExportDialogProps) => {
  const [format, setFormat] = useState<ExportFormat>('csv');
  const [delimiter, setDelimiter] = useState(DELIMITERS[0].value);
  const [includeHeader, setIncludeHeader] = useState(true);
  const [busy, setBusy] = useState<'export' | 'copy' | null>(null);
  const writeFile = useWriteExportFile();

  // Sem tabela conhecida não há INSERT que faça sentido (query livre, Redis).
  const sqlTable = useMemo(() => {
    if (!editableInfo) return null;
    const { schema, table } = editableInfo;
    return schema
      ? `${quoteIdent(schema)}.${quoteIdent(table)}`
      : quoteIdent(table);
  }, [editableInfo]);

  const formats = useMemo(
    () => (sqlTable ? FORMATS : FORMATS.filter(f => f.value !== 'sql')),
    [sqlTable],
  );

  // A grade só tem uma página: o resultado inteiro vem do banco na hora de exportar.
  const willFetchAll = hasMore && onFetchAllRows != null;

  // Quantas linhas o arquivo vai ter; null quando o backend não contou o total.
  const rowsToExport = willFetchAll ? totalCount : rows.length;

  const summary = useMemo(() => {
    if (!willFetchAll) return `${rows.length} linhas carregadas`;
    return totalCount != null
      ? `${totalCount} linhas — buscadas no banco ao exportar`
      : 'resultado inteiro — buscado no banco ao exportar';
  }, [willFetchAll, rows.length, totalCount]);

  const canCopy = CLIPBOARD_FORMATS.includes(format);

  // Não-nulo = o botão de copiar fica desabilitado, com este texto no title.
  const copyBlockedReason = useMemo(() => {
    if (rowsToExport == null) {
      return 'Total de linhas desconhecido — exporte para arquivo';
    }
    if (rowsToExport > CLIPBOARD_ROW_LIMIT) {
      return `Resultado grande demais para a área de transferência (máx. ${CLIPBOARD_ROW_LIMIT} linhas)`;
    }
    return null;
  }, [rowsToExport]);

  const buildContents = useCallback(async () => {
    const allRows = willFetchAll ? await onFetchAllRows!() : rows;
    const contents = serializeRows(columns, allRows, {
      format,
      delimiter,
      includeHeader,
      sqlTable: sqlTable ?? quoteIdent(fileName),
    });

    return { contents, rowCount: allRows.length };
  }, [
    columns,
    rows,
    format,
    delimiter,
    includeHeader,
    fileName,
    sqlTable,
    willFetchAll,
    onFetchAllRows,
  ]);

  const exportNow = useCallback(async () => {
    const extension = EXTENSIONS[format];
    const path = await save({
      defaultPath: `${fileName}.${extension}`,
      filters: [{ name: format.toUpperCase(), extensions: [extension] }],
    });
    if (!path) return;

    setBusy('export');
    try {
      const { contents, rowCount } = await buildContents();
      await writeFile.mutateAsync({ path, contents });
      toast.success(`${rowCount} linhas exportadas`);
      onOpenChange(false);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(null);
    }
  }, [format, fileName, buildContents, writeFile, onOpenChange]);

  const copyNow = useCallback(async () => {
    setBusy('copy');
    try {
      const { contents, rowCount } = await buildContents();
      await writeText(contents);
      toast.success(`${rowCount} linhas copiadas`);
      onOpenChange(false);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(null);
    }
  }, [buildContents, onOpenChange]);

  return {
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
    isExporting: busy === 'export',
    isCopying: busy === 'copy',
    isBusy: busy !== null,
    exportNow,
    copyNow,
  };
};
