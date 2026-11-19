import type { QueryMessage } from '@/api/types/query.types';
import type { QueryLogEntry } from './query-messages-log.types';

/**
 * O texto que o botão Copiar do log põe na área de transferência: só as
 * mensagens, como a seleção com o mouse — sem hora, severidade, a query de
 * cabeçalho nem as linhas de DETAIL/CONTEXT/SQLSTATE (que ficam só na tela).
 */

/** Linhas extras da mensagem, na ordem em que a tela mostra */
export const messageExtras = (message: QueryMessage): string[] =>
  [
    message.detail && `DETAIL: ${message.detail}`,
    message.hint && `HINT: ${message.hint}`,
    message.context && `CONTEXT: ${message.context}`,
    message.sqlState && `SQLSTATE: ${message.sqlState}`,
    message.position != null && `POSITION: ${message.position}`,
  ].filter(Boolean) as string[];

export const formatMessageText = (message: QueryMessage): string =>
  message.message;

/** As mensagens de uma execução, uma depois da outra */
export const formatEntryText = (entry: QueryLogEntry): string =>
  entry.messages.map(formatMessageText).join('\n');

/** O log inteiro: as execuções separadas por uma linha em branco */
export const formatLogText = (entries: QueryLogEntry[]): string =>
  entries
    .filter(entry => entry.messages.length > 0)
    .map(formatEntryText)
    .join('\n\n');
