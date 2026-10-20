import { gsap } from 'gsap';
import {
  armOutline,
  clonePose,
  cubic,
  tipAngle,
  type ArmPose,
  type WidthProfile,
} from './geometry';
import {
  EYE_LEFT,
  EYE_LEFT_BOX,
  EYE_RIGHT,
  EYE_RIGHT_BOX,
  HEAD,
  VIEWBOX,
} from './shape';

const SVG = 'http://www.w3.org/2000/svg';

/** Desfoque do filtro da silhueta: o raio das junções arredondadas */
const GOO_BLUR = 9;
const GOO_MIN_SIZE = 96;

export function svgEl<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | number> = {},
  parent?: Element,
): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG, tag);
  for (const [key, value] of Object.entries(attrs)) {
    el.setAttribute(key, String(value));
  }
  parent?.appendChild(el);
  return el;
}

/** Os quatro tentáculos, da esquerda para a direita */
export type ArmName = 'a' | 'b' | 'c' | 'd';
export const ARM_NAMES: ArmName[] = ['a', 'b', 'c', 'd'];

/** Pose de repouso: o desenho do logo */
export const REST: Record<ArmName, ArmPose> = {
  a: {
    p0: { x: 328, y: 413 },
    p1: { x: 210, y: 463 },
    p2: { x: 115, y: 497 },
    p3: { x: 48, y: 559 },
  },
  b: {
    p0: { x: 338, y: 464 },
    p1: { x: 350, y: 582 },
    p2: { x: 275, y: 603 },
    p3: { x: 268, y: 686 },
  },
  c: {
    p0: { x: 463, y: 461 },
    p1: { x: 538, y: 528 },
    p2: { x: 600, y: 610 },
    p3: { x: 623, y: 662 },
  },
  d: {
    p0: { x: 555, y: 406 },
    p1: { x: 617, y: 428 },
    p2: { x: 663, y: 407 },
    p3: { x: 820, y: 515 },
  },
};

export const PROFILES: Record<ArmName, WidthProfile> = {
  a: { root: 70, middle: 79, tip: 95 },
  b: { root: 82, middle: 89, tip: 103 },
  c: { root: 72, middle: 80, tip: 96 },
  d: { root: 74, middle: 74, tip: 97 },
};

export interface Arm {
  name: ArmName;
  path: SVGPathElement;
  /** Pose viva: o GSAP anima estes números e o `render` redesenha */
  pose: ArmPose;
  profile: WidthProfile;
  /** Grupo preso à ponta: o objeto que o tentáculo segura */
  hand: SVGGElement;
  /** O recorte do objeto segurado (na máscara, segue a ponta) */
  cut: SVGGElement;
  /** O objeto gira junto com a ponta (senão fica de pé) */
  followAngle: boolean;
  /**
   * Na frente do corpo: uma fresta em volta do tentáculo (fora da raiz) o
   * separa da cabeça e dos olhos — no monocromático, sem ela ele some.
   */
  front: boolean;
  /** A ponta segura um objeto: fica inteira por cima do recorte dele */
  grip: boolean;
}

export interface Octapus {
  svg: SVGSVGElement;
  /** Inclina e balança tudo junto */
  figure: SVGGElement;
  /** Cabeça (sem os tentáculos) */
  head: SVGPathElement;
  eyes: { group: SVGGElement; right: SVGPathElement; left: SVGPathElement };
  arms: Record<ArmName, Arm>;
  /** Objetos atrás do polvo e na frente dele */
  back: SVGGElement;
  front: SVGGElement;
  /** Recortes dos objetos da frente (mover junto com eles) */
  cutFront: SVGGElement;
  render: () => void;
  /** Redesenha a cada quadro (`true`) ou para (parado, fora da tela) */
  setLive: (live: boolean) => void;
  destroy: () => void;
}

/** Fresta em volta de um tentáculo na frente do corpo */
const ARM_GAP = 14;
/** O trecho perto da raiz fica colado à cabeça (sem fresta) */
const FRONT_FROM = 0.3;
/** Trecho final que fica inteiro quando a ponta segura um objeto */
const GRIP_FROM = 0.86;

let uid = 0;

/**
 * Monta o polvo dentro de `container`. Tudo em `currentColor`: o tema vem da
 * cor do texto do contêiner.
 *
 * A silhueta (cabeça + tentáculos) passa por um filtro "gooey" — desfoque e
 * limiar no alfa — que arredonda as junções do mesmo jeito em qualquer pose,
 * como no desenho do logo. Os olhos e os recortes ficam numa máscara por fora
 * do filtro, com borda nítida.
 */
export function createOctapus(container: Element): Octapus {
  const id = `oct${++uid}`;
  const { x, y, width, height } = VIEWBOX;
  const svg = svgEl('svg', {
    viewBox: `${x} ${y} ${width} ${height}`,
    fill: 'currentColor',
    'aria-hidden': 'true',
  });
  svg.style.overflow = 'visible';
  container.appendChild(svg);

  // Área dos filtros e da máscara: o viewBox com folga (os tentáculos saem)
  const pad = 320;
  const area = {
    x: x - pad,
    y: y - pad,
    width: width + pad * 2,
    height: height + pad * 2,
  };

  const defs = svgEl('defs', {}, svg);
  const goo = svgEl(
    'filter',
    {
      id: `${id}-goo`,
      filterUnits: 'userSpaceOnUse',
      ...area,
      'color-interpolation-filters': 'sRGB',
    },
    defs,
  );
  svgEl(
    'feGaussianBlur',
    { in: 'SourceGraphic', stdDeviation: GOO_BLUR, result: 'blur' },
    goo,
  );
  svgEl(
    'feColorMatrix',
    { in: 'blur', values: '1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 14 -7' },
    goo,
  );

  // Olhos: furos numa máscara, formas próprias para piscar e olhar
  const mask = svgEl(
    'mask',
    { id: `${id}-eyes`, maskUnits: 'userSpaceOnUse', ...area },
    defs,
  );
  svgEl('rect', { ...area, fill: 'white' }, mask);
  const eyeGroup = svgEl('g', {}, mask);
  const right = svgEl('path', { d: EYE_RIGHT, fill: 'black' }, eyeGroup);
  const left = svgEl('path', { d: EYE_LEFT, fill: 'black' }, eyeGroup);
  // Tentáculos na frente: a fresta (traço preto) e o próprio tentáculo de
  // volta em branco — passa por cima dos olhos
  const frontCuts = svgEl('g', {}, mask);
  // Recortes: a silhueta de cada objeto abre uma fresta no corpo atrás dele
  const cuts = svgEl('g', {}, mask);
  const cutFront = svgEl('g', {}, cuts);
  // As pontas que seguram algo voltam inteiras por cima do recorte
  const grips = svgEl('g', {}, mask);

  const figure = svgEl('g', {}, svg);
  const back = svgEl('g', {}, figure);
  const body = svgEl('g', { mask: `url(#${id}-eyes)` }, figure);
  const silhouette = svgEl('g', {}, body);
  // Pequeno, as junções não aparecem e o limiar do filtro serrilharia a
  // borda: o filtro só entra a partir de GOO_MIN_SIZE px
  const sizeWatch = new ResizeObserver(([entry]) => {
    const big = entry.contentRect.width >= GOO_MIN_SIZE;
    if (big) silhouette.setAttribute('filter', `url(#${id}-goo)`);
    else silhouette.removeAttribute('filter');
  });
  sizeWatch.observe(svg);
  const armsGroup = svgEl('g', {}, silhouette);
  const head = svgEl('path', { d: HEAD }, silhouette);
  const front = svgEl('g', {}, figure);
  // As mãos (objetos segurados) numa camada própria, acima de tudo
  const hands = svgEl('g', {}, figure);

  const arms = {} as Record<ArmName, Arm>;
  const extras = {} as Record<
    ArmName,
    { ring: SVGPathElement; restore: SVGPathElement; tip: SVGPathElement }
  >;
  for (const name of ARM_NAMES) {
    const path = svgEl('path', {}, armsGroup);
    const hand = svgEl('g', {}, hands);
    const cut = svgEl('g', {}, cuts);
    extras[name] = {
      ring: svgEl(
        'path',
        {
          fill: 'none',
          stroke: 'black',
          'stroke-width': ARM_GAP * 2,
          'stroke-linejoin': 'round',
        },
        frontCuts,
      ),
      restore: svgEl('path', { fill: 'white' }, frontCuts),
      tip: svgEl('path', { fill: 'white' }, grips),
    };
    arms[name] = {
      name,
      path,
      pose: clonePose(REST[name]),
      profile: { ...PROFILES[name] },
      hand,
      cut,
      followAngle: false,
      front: false,
      grip: false,
    };
  }

  const render = () => {
    for (const arm of Object.values(arms)) {
      const d = armOutline(arm.pose, arm.profile);
      arm.path.setAttribute('d', d);
      const { ring, restore, tip: tipPath } = extras[arm.name];
      ring.setAttribute(
        'd',
        arm.front ? armOutline(arm.pose, arm.profile, 18, FRONT_FROM) : '',
      );
      restore.setAttribute('d', arm.front ? d : '');
      tipPath.setAttribute(
        'd',
        arm.grip ? armOutline(arm.pose, arm.profile, 6, GRIP_FROM) : '',
      );
      const tip = cubic(arm.pose, 1);
      const transform = arm.followAngle
        ? `translate(${tip.x} ${tip.y}) rotate(${tipAngle(arm.pose)})`
        : `translate(${tip.x} ${tip.y})`;
      arm.hand.setAttribute('transform', transform);
      arm.cut.setAttribute('transform', transform);
    }
  };
  render();
  let live = false;
  const setLive = (next: boolean) => {
    if (next === live) return;
    live = next;
    if (live) gsap.ticker.add(render);
    else gsap.ticker.remove(render);
  };
  setLive(true);

  return {
    svg,
    figure,
    head,
    eyes: { group: eyeGroup, right, left },
    arms,
    back,
    front,
    cutFront,
    render,
    setLive,
    destroy: () => {
      setLive(false);
      sizeWatch.disconnect();
      svg.remove();
    },
  };
}

/** Centros dos olhos, para o pivô do piscar */
export const EYE_ORIGINS = {
  right: `${EYE_RIGHT_BOX.cx} ${EYE_RIGHT_BOX.cy}`,
  left: `${EYE_LEFT_BOX.cx} ${EYE_LEFT_BOX.cy}`,
};

/**
 * Leva um tentáculo a uma pose: os quatro pontos ao mesmo tempo. Devolve uma
 * timeline para entrar em outra (`tl.add(armTo(...), '<')`).
 */
export function armTo(
  arm: Arm,
  target: ArmPose,
  vars: gsap.TweenVars = {},
): gsap.core.Timeline {
  const tl = gsap.timeline();
  for (const key of ['p0', 'p1', 'p2', 'p3'] as const) {
    tl.to(arm.pose[key], { ...target[key], ...vars }, 0);
  }
  return tl;
}

/** Desloca uma pose inteira (todos os pontos) */
export const shiftPose = (pose: ArmPose, dx: number, dy: number): ArmPose => ({
  p0: { x: pose.p0.x + dx, y: pose.p0.y + dy },
  p1: { x: pose.p1.x + dx, y: pose.p1.y + dy },
  p2: { x: pose.p2.x + dx, y: pose.p2.y + dy },
  p3: { x: pose.p3.x + dx, y: pose.p3.y + dy },
});
