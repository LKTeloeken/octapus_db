export interface VaultStatus {
  /**
   * `false` quando o `vault.key` da máquina não abre mais o que está guardado
   * no `app.db` (arquivo perdido, trocado ou de outra instalação). As senhas
   * salvas viram irrecuperáveis e precisam ser cadastradas de novo.
   */
  healthy: boolean;
}
