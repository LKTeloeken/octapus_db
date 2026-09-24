import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { ValueEncoding } from '@/lib/value-format';

/**
 * Painel lateral de valor da aba de tabela: se está aberto e as preferências
 * de formatação. É global, não por aba — como no DBeaver, é preferência de
 * quem usa: aberto numa tabela, continua aberto ao trocar para outra.
 */
interface ValuePanelState {
  isOpen: boolean;
  /** Quebra linhas longas no editor */
  wordWrap: boolean;
  /** Exibe JSON/XML/HTML indentados */
  autoFormat: boolean;
  /** Grava JSON/XML minificados, mesmo editados na versão indentada */
  saveCompact: boolean;
  /** Codificação usada para converter o texto em bytes na visão binária */
  encoding: ValueEncoding;

  toggleOpen: () => void;
  setOpen: (isOpen: boolean) => void;
  setWordWrap: (wordWrap: boolean) => void;
  setAutoFormat: (autoFormat: boolean) => void;
  setSaveCompact: (saveCompact: boolean) => void;
  setEncoding: (encoding: ValueEncoding) => void;
}

export const useValuePanelStore = create<ValuePanelState>()(
  persist(
    set => ({
      isOpen: false,
      wordWrap: true,
      autoFormat: true,
      saveCompact: false,
      encoding: 'utf-8',

      toggleOpen: () => set(state => ({ isOpen: !state.isOpen })),
      setOpen: isOpen => set({ isOpen }),
      setWordWrap: wordWrap => set({ wordWrap }),
      setAutoFormat: autoFormat => set({ autoFormat }),
      setSaveCompact: saveCompact => set({ saveCompact }),
      setEncoding: encoding => set({ encoding }),
    }),
    { name: 'octapus-value-panel' },
  ),
);
