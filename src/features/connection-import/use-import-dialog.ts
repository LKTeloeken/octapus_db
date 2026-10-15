import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
import { useCallback, useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import type {
  ImportPreview,
  ImportPreviewRequest,
  ImportResult,
  ImportSourceInfo,
} from '@/api/types/import.types';
import {
  useImportCommit,
  useImportDiscard,
  useImportPreview,
  useImportSources,
} from '@/queries/use-connection-import';
import { useServerGroups } from '@/queries/use-server-groups';
import { useServers } from '@/queries/use-servers';
import { useImportDialogStore } from '@/stores/import-dialog-store';
import {
  SOURCE_LABELS,
  SOURCE_ORDER,
  selectedByDefault,
} from './import-labels';

/** "3 conexões importadas. 1 sem senha — informe ao editar o servidor." */
export function resultMessage(result: ImportResult): string {
  const parts = [
    result.imported === 1
      ? '1 conexão importada.'
      : `${result.imported} conexões importadas.`,
  ];
  if (result.groupsCreated > 0) {
    parts.push(
      result.groupsCreated === 1
        ? '1 grupo criado.'
        : `${result.groupsCreated} grupos criados.`,
    );
  }
  if (result.withoutPassword > 0) {
    parts.push(
      `${result.withoutPassword} sem senha — informe ao editar o servidor.`,
    );
  }
  if (result.keychainDenied) {
    parts.push('O acesso ao Keychain foi negado: o resto veio sem senha.');
  }
  return parts.join(' ');
}

/**
 * Importar conexões em dois passos: escolher a origem (apps detectados, um
 * arquivo ou URLs coladas) e revisar o que entra. As senhas nunca chegam
 * aqui — o backend as guarda presas ao token da prévia.
 */
export function useImportDialog(open: boolean) {
  const close = useImportDialogStore(state => state.close);
  const sources = useImportSources(open);
  const { data: groups = [] } = useServerGroups();
  const { data: servers = [] } = useServers();
  const preview = useImportPreview();
  const commit = useImportCommit();
  const discard = useImportDiscard();

  const [result, setResult] = useState<ImportPreview | null>(null);
  const [urls, setUrls] = useState('');
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [includePasswords, setIncludePasswords] = useState(true);
  const [keepGroups, setKeepGroups] = useState(true);
  const [targetGroupId, setTargetGroupId] = useState<number | null>(null);

  const orderedSources = useMemo(() => {
    const byKey = new Map(sources.data?.map(info => [info.source, info]));
    return SOURCE_ORDER.flatMap(source => byKey.get(source) ?? []);
  }, [sources.data]);

  const read = useCallback(
    async (request: ImportPreviewRequest) => {
      try {
        const data = await preview.mutateAsync(request);
        if (data.candidates.length === 0) {
          toast.error(
            request.source === 'urls'
              ? 'Nenhuma URL de conexão reconhecida.'
              : 'Nenhuma conexão encontrada nesse arquivo.',
          );
          return;
        }
        setResult(data);
        setSelected(
          new Set(data.candidates.filter(selectedByDefault).map(c => c.key)),
        );
      } catch (error) {
        toast.error(error instanceof Error ? error.message : String(error));
      }
    },
    [preview],
  );

  const readSource = useCallback(
    (info: ImportSourceInfo) => void read({ source: info.source }),
    [read],
  );

  const chooseFile = useCallback(
    async (info: ImportSourceInfo) => {
      const path = await openFileDialog({
        multiple: false,
        directory: false,
        title: `Conexões do ${SOURCE_LABELS[info.source]}`,
        filters: [
          { name: SOURCE_LABELS[info.source], extensions: info.fileExtensions },
          { name: 'Todos os arquivos', extensions: ['*'] },
        ],
      });
      if (typeof path === 'string') void read({ source: info.source, path });
    },
    [read],
  );

  const readUrls = useCallback(
    () => void read({ source: 'urls', text: urls }),
    [read, urls],
  );

  const candidates = result?.candidates ?? [];
  const selectable = candidates.filter(c => c.dbType !== null);
  const chosen = selectable.filter(c => selected.has(c.key));
  const allChosen =
    selectable.length > 0 && chosen.length === selectable.length;

  const toggle = useCallback((key: number) => {
    setSelected(current => {
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  const toggleAll = useCallback(() => {
    setSelected(allChosen ? new Set() : new Set(selectable.map(c => c.key)));
  }, [allChosen, selectable]);

  const serverNames = useMemo(
    () => new Map(servers.map(server => [server.id, server.name])),
    [servers],
  );

  const back = useCallback(() => {
    setResult(null);
    void discard.mutateAsync();
  }, [discard]);

  /** Fechar sem importar descarta a prévia (e as senhas) no backend */
  const dismiss = useCallback(() => {
    if (result) void discard.mutateAsync();
    close();
  }, [close, discard, result]);

  const submit = useCallback(async () => {
    if (!result || chosen.length === 0) return;
    try {
      const outcome = await commit.mutateAsync({
        token: result.token,
        keys: chosen.map(c => c.key),
        includePasswords,
        keepGroups: keepGroups && result.groups.length > 0,
        targetGroupId,
      });
      toast.success(resultMessage(outcome));
      close();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  }, [
    chosen,
    close,
    commit,
    includePasswords,
    keepGroups,
    result,
    targetGroupId,
  ]);

  return {
    sources: orderedSources,
    isDetecting: sources.isLoading,
    detectError: sources.error,
    isReading: preview.isPending,
    readingSource: preview.isPending ? preview.variables?.source : undefined,
    readSource,
    chooseFile,
    urls,
    setUrls,
    readUrls,
    result,
    candidates,
    selected,
    chosenCount: chosen.length,
    allChosen,
    someChosen: chosen.length > 0 && !allChosen,
    toggle,
    toggleAll,
    serverNames,
    sshCount: chosen.filter(c => c.warnings.some(w => w.kind === 'sshTunnel'))
      .length,
    hasPasswords: chosen.some(c => c.hasPassword || c.passwordInKeychain),
    keychainCount: chosen.filter(c => c.passwordInKeychain).length,
    includePasswords,
    setIncludePasswords,
    keepGroups,
    setKeepGroups,
    groups,
    targetGroupId,
    setTargetGroupId,
    isImporting: commit.isPending,
    back,
    dismiss,
    submit,
  };
}
