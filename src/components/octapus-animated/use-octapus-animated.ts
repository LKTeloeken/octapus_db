import { useEffect, useRef, useState } from 'react';
import type { OctapusSceneId } from './scene-id';

type Engine = typeof import('./engine');
type Stage = InstanceType<Engine['Stage']>;

let engineLoad: Promise<Engine> | null = null;

/** O motor (e o GSAP) vem num chunk só, pedido uma vez para o app todo */
function loadEngine(): Promise<Engine> {
  engineLoad ??= import('./engine').catch((error: unknown) => {
    // Deixa tentar de novo no próximo polvo
    engineLoad = null;
    throw error;
  });
  return engineLoad;
}

/**
 * Liga o palco do polvo a um elemento: carrega o motor sob demanda, toca a
 * `scene` (trocar passa pelo repouso), para fora da tela ou com a janela
 * oculta, e segue o `prefers-reduced-motion`. Enquanto o motor não chega,
 * `ready` fica `false` e quem usa mostra o polvo em CSS.
 */
export function useOctapusAnimated(scene: OctapusSceneId) {
  const containerRef = useRef<HTMLSpanElement>(null);
  const stageRef = useRef<Stage | null>(null);
  const [engine, setEngine] = useState<Engine | null>(null);

  useEffect(() => {
    let cancelled = false;
    loadEngine().then(
      loaded => {
        if (!cancelled) setEngine(loaded);
      },
      // Sem o chunk (offline, build quebrado), fica o polvo em CSS
      () => {},
    );
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const container = containerRef.current;
    if (!engine || !container) return;
    const stage = new engine.Stage(container);
    stageRef.current = stage;

    const motion = window.matchMedia('(prefers-reduced-motion: reduce)');
    const onMotion = () => stage.setReduced(motion.matches);
    onMotion();
    motion.addEventListener('change', onMotion);

    let onScreen = true;
    const sync = () => stage.setActive(onScreen && !document.hidden);
    const observer = new IntersectionObserver(([entry]) => {
      onScreen = entry.isIntersecting;
      sync();
    });
    observer.observe(container);
    document.addEventListener('visibilitychange', sync);

    return () => {
      motion.removeEventListener('change', onMotion);
      observer.disconnect();
      document.removeEventListener('visibilitychange', sync);
      stage.destroy();
      stageRef.current = null;
    };
  }, [engine]);

  useEffect(() => {
    if (engine) stageRef.current?.play(engine.SCENES[scene]);
  }, [engine, scene]);

  return { containerRef, ready: engine !== null };
}
