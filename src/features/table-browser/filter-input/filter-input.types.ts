import type { CompletionSource } from '@codemirror/autocomplete';
import type { Extension } from '@codemirror/state';

export interface FilterInputProps {
  value: string;
  onChange: (value: string) => void;
  onApply: (value: string) => void;
  onReset: () => void;
  /** Autocomplete SQL do banco da aba; identidade estável (ver `useSqlCompletion`). */
  completionSource: CompletionSource;
  /** Prefetch + contexto implícito da tabela aberta. Também precisa ser estável. */
  completionExtensions: Extension;
}
