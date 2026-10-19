import type { ColumnChange, ColumnSpec } from '@/api/types/catalog.types';

export type ChangeTone = 'onlyBase' | 'onlyOther' | 'changed';

export interface ChangeLabel {
  tone: ChangeTone;
  /** Rótulo curto do marcador */
  label: string;
  /** O que muda, em uma linha (base → outro) */
  detail: string;
}

/** `numeric NOT NULL DEFAULT 0` */
export function specText(spec: ColumnSpec): string {
  return [
    spec.dataType,
    spec.nullable ? null : 'NOT NULL',
    spec.defaultValue ? `DEFAULT ${spec.defaultValue}` : null,
  ]
    .filter(Boolean)
    .join(' ');
}

/** Uma coluna diferente entre a base e o outro schema, para a lista do modal */
export function describeChange(change: ColumnChange): ChangeLabel {
  const { expected, actual } = change;
  if (!expected && actual) {
    return {
      tone: 'onlyOther',
      label: 'só no outro',
      detail: specText(actual),
    };
  }
  if (expected && !actual) {
    return {
      tone: 'onlyBase',
      label: 'só na base',
      detail: specText(expected),
    };
  }

  const parts: string[] = [];
  if (expected && actual) {
    if (expected.dataType !== actual.dataType) {
      parts.push(`${expected.dataType} → ${actual.dataType}`);
    }
    if (expected.nullable !== actual.nullable) {
      parts.push(
        actual.nullable ? 'NOT NULL → aceita nulo' : 'aceita nulo → NOT NULL',
      );
    }
    if (expected.defaultValue !== actual.defaultValue) {
      parts.push(
        `default ${expected.defaultValue ?? 'nenhum'} → ${actual.defaultValue ?? 'nenhum'}`,
      );
    }
  }
  return { tone: 'changed', label: 'difere', detail: parts.join(' · ') };
}
