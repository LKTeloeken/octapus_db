import { useCallback, useState } from 'react';
import toast from 'react-hot-toast';
import { useReplyAssistantTurn } from '@/queries/use-assistant-conversations';

/** Tamanho máximo da resposta (o mesmo do backend e da API) */
export const REPLY_MAX = 2000;

const messageOf = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

/** Conta caracteres como o backend (code points, não unidades UTF-16) */
const lengthOf = (text: string) => Array.from(text).length;

/**
 * A resposta a uma pergunta do modelo (`ask_user`) no turno em andamento:
 * uma das opções, com um clique, ou o texto livre do campo.
 */
export const useTurnQuestion = ({
  clientTurnId,
}: {
  clientTurnId: string | null;
}) => {
  const [draft, setDraft] = useState('');
  const reply = useReplyAssistantTurn();
  const length = lengthOf(draft.trim());
  const tooLong = length > REPLY_MAX;

  const send = useCallback(
    async (text: string) => {
      const value = text.trim();
      if (
        !clientTurnId ||
        !value ||
        lengthOf(value) > REPLY_MAX ||
        reply.isPending
      ) {
        return;
      }
      try {
        const delivered = await reply.mutateAsync({
          clientTurnId,
          reply: value,
        });
        if (delivered) setDraft('');
        else toast.error('Esta pergunta não está mais esperando resposta.');
      } catch (error) {
        toast.error(messageOf(error));
      }
    },
    [clientTurnId, reply],
  );

  return {
    draft,
    setDraft,
    length,
    tooLong,
    max: REPLY_MAX,
    sending: reply.isPending,
    canSend: !!clientTurnId && length > 0 && !tooLong && !reply.isPending,
    send,
    sendDraft: () => void send(draft),
    choose: (choice: string) => void send(choice),
  };
};
