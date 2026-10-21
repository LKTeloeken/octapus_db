import type { OctapusSceneId } from '@/components/octapus-animated/scene-id';
import type { TurnPhase } from '@/lib/assistant/turn-reducer';
import type { TurnOutcome } from './turn-view';

/** A cena do polvo enquanto o turno anda: uma por fase */
export function turnScene(
  phase: TurnPhase,
  streaming: boolean,
): OctapusSceneId {
  switch (phase) {
    case 'opening':
    case 'structure':
      return 'magnifier';
    case 'generating':
      // O stream chegando é o polvo "escrevendo" (também numa correção)
      return streaming ? 'computer' : 'thinking';
    case 'checking':
      return 'clipboard';
    case 'executing':
      return 'tables';
    case 'waiting':
      return 'hourglass';
    case 'finished':
      return 'thinking';
  }
}

/**
 * A cena de fim, quando o turno acaba de terminar na tela: comemora a
 * resposta, coça a cabeça na falha. Pergunta de volta e cancelado não têm.
 */
export function endingScene(outcome: TurnOutcome): OctapusSceneId | null {
  switch (outcome) {
    case 'answered':
    case 'code':
      return 'celebrate';
    case 'failed':
    case 'exhausted':
      return 'scratch';
    default:
      return null;
  }
}
