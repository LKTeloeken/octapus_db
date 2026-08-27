export interface VaultStatus {
  /** Há senha mestre configurada nesta instalação */
  hasMasterPassword: boolean;
  /**
   * Existe senha mestre e ela ainda não foi digitada nesta sessão. A lista de
   * servidores continua funcionando; só conectar exige destravar.
   */
  locked: boolean;
  /**
   * `false` quando a chave não abre o que está guardado (canário não bate) ou
   * o `vault.key` está ilegível. Trancado **não** conta como não-saudável.
   */
  healthy: boolean;
  /** `vault.key` ilegível em qualquer formato — só resta o reset */
  corrupt: boolean;
}
