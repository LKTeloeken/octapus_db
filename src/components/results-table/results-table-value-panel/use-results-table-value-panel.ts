import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  formatBytes,
  type ValueEncoding,
  type ValueFormat,
} from '@/lib/value-format';
import { useValuePanelStore } from '@/stores/value-panel-store';
import type {
  ResultsTableValuePanelProps,
  ValueIssue,
} from './results-table-value-panel.types';
import {
  detectValueFormat,
  resolveValueKind,
  toEditorText,
  toStoredValue,
  type EditorTextOptions,
} from './value-conversion';

/** Teto da fila de ecos: digitação muito rápida nunca acumula mais que isso. */
const MAX_PENDING_ECHOES = 50;

/**
 * Lógica do painel de valor. A digitação vira alteração **pendente** na hora
 * (via `updateCell`, a mesma da grade): a célula fica amarela e só persiste no
 * Salvar. Assim não existe um "aplicar" que o usuário possa esquecer antes do
 * Cmd/Ctrl+S, nem edição perdida ao mover o cursor para outra célula.
 *
 * O cuidado é o caminho de volta: o `value` que chega depois de cada tecla é o
 * eco do que o próprio painel gravou e não pode reescrever o editor (o cursor
 * pularia e a formatação sumiria). Só a troca de célula ou uma mudança vinda de
 * fora (editor da grade, descartar, NULL) ressincroniza.
 */
export const useResultsTableValuePanel = ({
  target,
  updateCell,
}: Pick<ResultsTableValuePanelProps, 'target' | 'updateCell'>) => {
  const wordWrap = useValuePanelStore(state => state.wordWrap);
  const autoFormat = useValuePanelStore(state => state.autoFormat);
  const saveCompact = useValuePanelStore(state => state.saveCompact);
  const encoding = useValuePanelStore(state => state.encoding);
  const setWordWrap = useValuePanelStore(state => state.setWordWrap);
  const setAutoFormatPref = useValuePanelStore(state => state.setAutoFormat);
  const setSaveCompact = useValuePanelStore(state => state.setSaveCompact);
  const setEncodingPref = useValuePanelStore(state => state.setEncoding);

  const cellKey = target ? `${target.rowIndex}:${target.column.name}` : null;
  const typeName = target?.column.typeName ?? '';
  const kind = resolveValueKind(typeName);
  const value = target?.value ?? null;

  const [format, setFormat] = useState<ValueFormat>('text');
  const [draft, setDraft] = useState('');
  const [issue, setIssue] = useState<ValueIssue | null>(null);
  // Muda a cada ressincronização: remonta o editor com o histórico de desfazer
  // limpo — senão o Cmd+Z traria de volta o texto de outra célula e o gravaria
  // nesta.
  const [editorKey, setEditorKey] = useState(0);

  /** Formato escolhido à mão, por coluna — vale ao andar entre as linhas. */
  const overridesRef = useRef(new Map<string, ValueFormat>());
  /** Valores gravados pelo painel que ainda podem voltar como `value`. É fila,
   *  não o último: com teclas rápidas, o eco de uma pode chegar depois da outra. */
  const echoesRef = useRef<{ key: string | null; values: (string | null)[] }>({
    key: null,
    values: [],
  });
  /** Texto exibido na sincronização e o valor que ele representa. */
  const baselineRef = useRef<{ text: string; value: string | null }>({
    text: '',
    value: null,
  });

  const sync = useCallback(
    (nextFormat: ValueFormat, options: EditorTextOptions) => {
      const text = toEditorText(value, nextFormat, kind, options);
      echoesRef.current = { key: null, values: [] };
      baselineRef.current = { text, value };
      setFormat(nextFormat);
      setDraft(text);
      setIssue(null);
      setEditorKey(key => key + 1);
    },
    [value, kind],
  );

  // Troca de célula ou valor mudado fora do painel → ressincroniza. As
  // preferências e o formato não entram: são tratados nos próprios handlers.
  useEffect(() => {
    const echoes = echoesRef.current;
    if (echoes.key === cellKey) {
      const index = echoes.values.indexOf(value);
      if (index !== -1) {
        echoes.values = echoes.values.slice(index + 1);
        return;
      }
    }

    const columnName = target?.column.name;
    const override = columnName
      ? overridesRef.current.get(columnName)
      : undefined;
    sync(override ?? detectValueFormat(value, kind, typeName), {
      autoFormat,
      encoding,
    });
  }, [cellKey, value, typeName]);

  // Refs para o `onChange` do editor ter identidade estável: trocar a função
  // reconfigura o CodeMirror, e `target` muda a cada render da grade.
  const latest = useRef({ target, cellKey, format, kind, saveCompact });
  latest.current = { target, cellKey, format, kind, saveCompact };

  const handleChange = useCallback(
    (text: string) => {
      setDraft(text);

      const { target, cellKey, format, kind, saveCompact } = latest.current;
      if (!target || !target.isEditable || format === 'binary') return;

      let next: string | null;
      const baseline = baselineRef.current;
      if (text === baseline.text) {
        // Voltar ao texto exibido grava o valor de antes — senão só reindentar
        // já viraria uma edição pendente.
        next = baseline.value;
        setIssue(null);
      } else {
        const result = toStoredValue(text, format, kind, saveCompact);
        if (!result.ok) {
          setIssue({ message: result.error, blocking: true });
          return;
        }
        setIssue(
          result.warning ? { message: result.warning, blocking: false } : null,
        );
        next = result.value;
      }

      const echoes = echoesRef.current;
      if (echoes.key !== cellKey) {
        echoesRef.current = { key: cellKey, values: [next] };
      } else {
        echoes.values = [...echoes.values, next].slice(-MAX_PENDING_ECHOES);
      }
      updateCell(
        target.rowIndex,
        target.column.name,
        target.originalValue,
        next,
      );
    },
    [updateCell],
  );

  const changeFormat = useCallback(
    (next: ValueFormat) => {
      if (target) overridesRef.current.set(target.column.name, next);
      sync(next, { autoFormat, encoding });
    },
    [target, sync, autoFormat, encoding],
  );

  const changeAutoFormat = useCallback(
    (next: boolean) => {
      setAutoFormatPref(next);
      sync(format, { autoFormat: next, encoding });
    },
    [setAutoFormatPref, sync, format, encoding],
  );

  const changeEncoding = useCallback(
    (next: ValueEncoding) => {
      setEncodingPref(next);
      if (format === 'binary') sync(format, { autoFormat, encoding: next });
    },
    [setEncodingPref, sync, format, autoFormat],
  );

  const setNull = useCallback(() => {
    if (!target) return;
    // Já é NULL: nada muda na célula, então o texto inválido digitado (e não
    // aplicado) só some ressincronizando à mão.
    if (target.value === null) {
      sync(format, { autoFormat, encoding });
      return;
    }
    updateCell(target.rowIndex, target.column.name, target.originalValue, null);
  }, [target, updateCell, sync, format, autoFormat, encoding]);

  /** Desfaz a edição pendente desta célula, voltando ao valor do banco. */
  const revert = useCallback(() => {
    if (!target) return;
    updateCell(
      target.rowIndex,
      target.column.name,
      target.originalValue,
      target.originalValue,
    );
  }, [target, updateCell]);

  const sizeLabel = useMemo(
    () =>
      value === null
        ? null
        : formatBytes(new TextEncoder().encode(value).length),
    [value],
  );

  return {
    kind,
    /** Menu de formatação só para JSON, lista e texto */
    isFormattable: kind !== 'scalar',
    isReadOnly: !target?.isEditable || format === 'binary',
    format,
    draft,
    issue,
    editorKey,
    sizeLabel,
    wordWrap,
    autoFormat,
    saveCompact,
    encoding,
    handleChange,
    changeFormat,
    changeAutoFormat,
    changeEncoding,
    setWordWrap,
    setSaveCompact,
    setNull,
    revert,
  };
};
