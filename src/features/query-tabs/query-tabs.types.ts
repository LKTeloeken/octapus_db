/** Aba à espera da confirmação de fechar (a grade dela tem pendências) */
export interface ClosingTab {
  tabId: string;
  title: string;
  pendingCount: number;
  /** Falso durante a animação de saída do diálogo */
  open: boolean;
}
