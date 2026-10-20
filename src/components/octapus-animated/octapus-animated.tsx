import { OctapusThinking } from '@/components/octapus-thinking/octapus-thinking';
import { cn } from '@/lib/utils';
import type { OctapusSceneId } from './scene-id';
import { useOctapusAnimated } from './use-octapus-animated';

interface OctapusAnimatedProps {
  /** A cena: uma por fase do turno do assistente (ver `scene-id.ts`) */
  scene?: OctapusSceneId;
  /** Tamanho em classes do Tailwind (`size-12`); a cor vem do `text-*` */
  className?: string;
  /** Rótulo para leitores de tela; sem ele, o polvo é decorativo */
  label?: string;
}

/**
 * O polvo do assistente animado com GSAP: o desenho do logo com tentáculos
 * que dobram e objetos de cena (lupa, notebook, prancheta, tabelas…). O
 * motor vem sob demanda; até lá aparece o polvo em CSS. SVG em
 * `currentColor`: acompanha o tema pelo `text-*`, sem cor crua. Para ícones
 * pequenos (abas), use o `OctapusThinking`, que é só CSS.
 */
export function OctapusAnimated({
  scene = 'thinking',
  className,
  label,
}: OctapusAnimatedProps) {
  const { containerRef, ready } = useOctapusAnimated(scene);

  return (
    <span
      className={cn(
        'octapus-animated relative inline-flex size-8 shrink-0 text-fg-muted',
        className,
      )}
      {...(label
        ? { role: 'img', 'aria-label': label }
        : { 'aria-hidden': true })}
    >
      {!ready && <OctapusThinking className="size-full text-current" />}
      <span ref={containerRef} className="absolute inset-0 [&>svg]:size-full" />
    </span>
  );
}
