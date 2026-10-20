import { useId } from 'react';
import { cn } from '@/lib/utils';
import { OCTAPUS_PATH, OCTAPUS_VIEWBOX } from './octapus-path';

interface OctapusThinkingProps {
  /** Tamanho em classes do Tailwind (`size-8`); a cor vem do `text-*` */
  className?: string;
  /** Rótulo para leitores de tela; sem ele, o polvo é decorativo */
  label?: string;
}

/**
 * Recortes do desenho, em unidades do `viewBox`. O polvo é um `path` só: cada
 * tentáculo é uma cópia dele recortada em volta do tentáculo, girando na
 * própria base (o pivô do CSS, em `globals.css`). O corpo, estático, vai por
 * cima e cobre as junções até o topo de cada arco — por isso não aparece
 * emenda. Os recortes dos tentáculos ficam abaixo dos olhos (vazados), senão
 * uma cópia girada taparia um olho.
 */
const CLIPS = {
  body: '120,0 760,0 760,240 660,290 630,340 610,420 570,465 500,500 440,505 395,505 330,505 280,500 240,470 200,400 120,250',
  a: '0,380 240,380 260,470 300,500 260,540 150,620 0,700',
  b: '250,470 400,470 400,760 150,760 150,620 260,540 300,500',
  c: '400,470 500,460 560,420 600,470 640,560 900,760 400,760',
  d: '540,240 900,240 900,760 640,560 600,470 560,420',
} as const;

const ARMS = ['a', 'b', 'c', 'd'] as const;

/**
 * O polvo pensando, enquanto o assistente trabalha: os tentáculos ondulam em
 * sequência, um deles tamborila, a cabeça inclina e os olhos piscam. Só
 * `transform` e `opacity` — com `prefers-reduced-motion`, sobra um pulso de
 * opacidade (o carregando continua, é informação). SVG inline com
 * `currentColor`: acompanha o tema pelo `text-*`, sem cor crua.
 */
export function OctapusThinking({ className, label }: OctapusThinkingProps) {
  // Ids dos recortes únicos por instância (o `useId` traz `:`/`«»`, que
  // quebram o `url(#…)`)
  const uid = `octapus-${useId().replace(/[^a-zA-Z0-9_-]/g, '')}`;
  const shape = `${uid}-shape`;
  const part = (key: keyof typeof CLIPS) => (
    <use href={`#${shape}`} clipPath={`url(#${uid}-${key})`} />
  );

  return (
    <svg
      viewBox={OCTAPUS_VIEWBOX}
      fill="currentColor"
      className={cn(
        'octapus-thinking size-8 shrink-0 text-fg-muted',
        className,
      )}
      {...(label
        ? { role: 'img', 'aria-label': label }
        : { 'aria-hidden': true })}
    >
      <defs>
        <path id={shape} fillRule="evenodd" d={OCTAPUS_PATH} />
        {(Object.keys(CLIPS) as (keyof typeof CLIPS)[]).map(key => (
          <clipPath key={key} id={`${uid}-${key}`}>
            <polygon points={CLIPS[key]} />
          </clipPath>
        ))}
      </defs>
      <g className="oct-figure">
        <g className="oct-bob">
          {ARMS.map(arm => (
            <g key={arm} className={`oct-arm oct-arm-${arm}`}>
              {arm === 'c' ? <g className="oct-tap">{part(arm)}</g> : part(arm)}
            </g>
          ))}
          {part('body')}
          {/* Pálpebras: tapam os olhos vazados por um instante */}
          <ellipse className="oct-lid" cx="316" cy="319" rx="46" ry="65" />
          <ellipse className="oct-lid" cx="451" cy="290" rx="60" ry="81" />
        </g>
      </g>
    </svg>
  );
}
