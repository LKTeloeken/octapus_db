import { useEffect, useState } from 'react';
import type { OctapusSceneId } from '@/components/octapus-animated/scene-id';
import { endingScene, turnScene } from './turn-mascot';
import type { TurnView } from './turn-view';

/** Quanto a cena de fim fica na tela (as duas tocam uma vez em ~1,5 s) */
const ENDING_MS = 2200;

/**
 * O polvo do cartão do turno: a cena da fase enquanto roda e, se o turno
 * terminou com este cartão aberto (não ao abrir o histórico), a cena de fim
 * por um instante. `null` = sem polvo.
 */
export function useTurnMascot(view: TurnView): OctapusSceneId | null {
  const running = view.outcome === 'running';
  const [wasRunning, setWasRunning] = useState(running);
  const [ending, setEnding] = useState<OctapusSceneId | null>(null);

  // Viu o turno passar de rodando para terminado: guarda a cena de fim
  if (wasRunning !== running) {
    setWasRunning(running);
    setEnding(running ? null : endingScene(view.outcome));
  }

  useEffect(() => {
    if (!ending) return;
    const timer = window.setTimeout(() => setEnding(null), ENDING_MS);
    return () => window.clearTimeout(timer);
  }, [ending]);

  return running ? turnScene(view.phase, view.streaming) : ending;
}
