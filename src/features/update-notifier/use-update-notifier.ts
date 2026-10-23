import { useCallback, useEffect, useRef } from 'react';
import {
  CHECK_INTERVAL_MS,
  FIRST_CHECK_DELAY_MS,
  FOCUS_CHECK_MIN_MS,
  isCheckDue,
  SCHEDULE_TICK_MS,
} from '@/lib/update-schedule';
import { useSettings } from '@/queries/use-settings';
import { useUpdateStore } from '@/stores/update-store';

/** Checar atualização é best-effort: falha de rede não incomoda o usuário. */
const logCheckError = (error: unknown) =>
  console.error('Falha ao verificar atualizações:', error);

/**
 * Agenda as checagens de versão nova (ver `lib/update-schedule.ts`): uma logo
 * depois da partida, outra ao trocar de canal e, com o app aberto, a cada
 * `CHECK_INTERVAL_MS` — ou ao voltar o foco para a janela, se já faz
 * `FOCUS_CHECK_MIN_MS` desde a última.
 */
export const useUpdateSchedule = () => {
  // O canal vem das preferências (SQLite): sem elas carregadas, não checa.
  const beta = useSettings().data?.betaUpdates;
  const betaRef = useRef(beta);
  betaRef.current = beta;
  /** Canal da última checagem agendada — `null` antes da primeira */
  const checkedChannel = useRef<boolean | null>(null);

  const checkIfDue = useCallback((minIntervalMs: number) => {
    const channel = betaRef.current;
    if (channel === undefined) return;
    const { lastCheckedAt, status, checking, check } =
      useUpdateStore.getState();
    if (
      !isCheckDue({
        now: Date.now(),
        lastCheckedAt,
        status,
        checking,
        minIntervalMs,
      })
    ) {
      return;
    }
    check(channel).catch(logCheckError);
  }, []);

  // Partida e troca de canal.
  useEffect(() => {
    if (beta === undefined) return;
    // O StrictMode monta o efeito duas vezes em dev; e mudar outra preferência
    // não pode checar de novo — só a troca de canal.
    if (checkedChannel.current === beta) return;
    const isFirstCheck = checkedChannel.current === null;
    checkedChannel.current = beta;

    // Na partida, espera a carga inicial; ao trocar de canal, checa na hora.
    let fired = false;
    const timer = setTimeout(
      () => {
        fired = true;
        checkIfDue(0);
      },
      isFirstCheck ? FIRST_CHECK_DELAY_MS : 0,
    );

    return () => {
      clearTimeout(timer);
      // Saiu antes de checar (StrictMode, troca de canal no meio da espera):
      // o próximo efeito é que faz a checagem.
      if (!fired) checkedChannel.current = isFirstCheck ? null : !beta;
    };
  }, [beta, checkIfDue]);

  // Com o app aberto: o relógio, conferido de tempos em tempos, e a volta do
  // foco. Antes da primeira checagem quem manda é a da partida.
  useEffect(() => {
    const onTick = () => {
      if (useUpdateStore.getState().lastCheckedAt === null) return;
      checkIfDue(CHECK_INTERVAL_MS);
    };
    const onFocus = () => {
      if (document.visibilityState === 'hidden') return;
      if (useUpdateStore.getState().lastCheckedAt === null) return;
      checkIfDue(FOCUS_CHECK_MIN_MS);
    };

    const interval = setInterval(onTick, SCHEDULE_TICK_MS);
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onFocus);
    return () => {
      clearInterval(interval);
      window.removeEventListener('focus', onFocus);
      document.removeEventListener('visibilitychange', onFocus);
    };
  }, [checkIfDue]);
};

export const useUpdateNotifier = () => {
  useUpdateSchedule();

  const status = useUpdateStore(state => state.status);
  const version = useUpdateStore(state => state.version);
  const progress = useUpdateStore(state => state.progress);
  const dismissed = useUpdateStore(state => state.dismissed);
  const install = useUpdateStore(state => state.install);
  const restart = useUpdateStore(state => state.restart);
  const dismiss = useUpdateStore(state => state.dismiss);

  // O progresso do download aparece mesmo depois de um "Depois" antigo
  const visible = status === 'downloading' || (status !== 'idle' && !dismissed);

  return { visible, status, version, progress, install, restart, dismiss };
};
