import { gsap } from 'gsap';
import type { ArmPose, Point } from '../geometry';
import {
  breathe,
  eyesScaleY,
  gripOrigin,
  waveFree,
  type Scene,
} from '../motion';
import { magnifier } from '../props';
import { EYE_ORIGINS, REST, armTo } from '../rig';

/** Distância da pega ao centro da lente (cabo de 230 × escala 1,4) */
const REACH = 322;
/** Inclinação do cabo na lupa (graus) */
const TILT = -40;

/**
 * Onde fica a ponta para a lente cair em `lens`, com a lupa girada `turn`
 * graus a mais: sobre o rosto o cabo deita, e a ponta fica fora da cabeça.
 */
function tipFor(lens: Point, turn = 0): Point {
  const angle = ((TILT + turn) * Math.PI) / 180;
  return {
    x: lens.x - REACH * Math.sin(angle),
    y: lens.y + REACH * Math.cos(angle),
  };
}

/** Pose do tentáculo direito: sai da raiz para a direita e enrola até `tip` */
function holding(tip: Point, p1: Point, bend: Point): ArmPose {
  return {
    p0: { ...REST.d.p0 },
    p1,
    p2: { x: tip.x + bend.x, y: tip.y + bend.y },
    p3: { ...tip },
  };
}

/**
 * Lupa (lendo a estrutura do banco): o tentáculo direito ergue a lupa e
 * varre o rosto — cada olho cresce quando a lente passa por cima — e volta,
 * com um brilho na lente.
 */
export const magnifierScene: Scene = {
  id: 'magnifier',
  label: 'Lupa',
  phase: 'opening · lendo a estrutura',
  loop: true,
  build(rig) {
    breathe(rig, 10);
    waveFree(rig, ['d']);

    const d = rig.arms.d;
    d.grip = true;
    const lens = magnifier(d.hand);
    const lensCut = magnifier(d.cut, true);
    const both = [lens.g, lensCut.g];
    gripOrigin(both);
    gsap.set(both, { scale: 0.4, opacity: 0 });

    // Fora: o braço estendido à direita. Sobre cada olho o cabo deita e o
    // braço fica no lugar do repouso, com a ponta enrolada para cima — nunca
    // passa na frente da cabeça nem encosta nela
    const out = holding(
      tipFor({ x: 760, y: 230 }),
      { x: 660, y: 480 },
      { x: 70, y: 90 },
    );
    const RIGHT_TURN = -30;
    const LEFT_TURN = -32;
    const overRight = holding(
      tipFor({ x: 456, y: 291 }, RIGHT_TURN),
      { x: 640, y: 445 },
      { x: 50, y: 55 },
    );
    // A lente cobre o olho esquerdo pela borda: o braço não alcança mais
    const overLeft = holding(
      tipFor({ x: 372, y: 318 }, LEFT_TURN),
      { x: 630, y: 445 },
      { x: 50, y: 55 },
    );
    const { right, left, group } = rig.eyes;

    const tl = gsap.timeline({ repeat: -1, repeatDelay: 0.3 });
    tl
      // Ergue a lupa, com uma passada antes de assentar
      .add(armTo(d, out, { duration: 0.5, ease: 'back.out(1.6)' }), 0)
      .to(
        both,
        { scale: 1, opacity: 1, duration: 0.35, ease: 'back.out(2)' },
        0.1,
      )
      .to(group, { x: 18, y: -6, duration: 0.3, ease: 'power2.out' }, 0.2)
      // Varre o olho direito: ele aumenta sob a lente
      .add(armTo(d, overRight, { duration: 0.6, ease: 'power2.inOut' }), 0.8)
      .to(
        both,
        { rotation: RIGHT_TURN, duration: 0.6, ease: 'power2.inOut' },
        '<',
      )
      .to(group, { x: 6, y: 0, duration: 0.4, ease: 'power2.inOut' }, '<')
      .to(
        right,
        {
          scale: 1.35,
          svgOrigin: EYE_ORIGINS.right,
          duration: 0.25,
          ease: 'back.out(2)',
        },
        '>-0.15',
      )
      .to(
        right,
        {
          scale: 1,
          svgOrigin: EYE_ORIGINS.right,
          duration: 0.2,
          ease: 'power2.in',
        },
        '+=0.35',
      )
      // ...e o esquerdo
      .add(armTo(d, overLeft, { duration: 0.45, ease: 'power2.inOut' }), '<')
      .to(
        both,
        { rotation: LEFT_TURN, duration: 0.45, ease: 'power2.inOut' },
        '<',
      )
      .to(
        left,
        {
          scale: 1.35,
          svgOrigin: EYE_ORIGINS.left,
          duration: 0.25,
          ease: 'back.out(2)',
        },
        '>-0.12',
      )
      .to(
        left,
        {
          scale: 1,
          svgOrigin: EYE_ORIGINS.left,
          duration: 0.2,
          ease: 'power2.in',
        },
        '+=0.3',
      )
      // Volta para fora e a lente brilha
      .add(armTo(d, out, { duration: 0.55, ease: 'power2.inOut' }), '<')
      .to(both, { rotation: 0, duration: 0.55, ease: 'power2.inOut' }, '<')
      .to(group, { x: 18, y: -6, duration: 0.4, ease: 'power2.inOut' }, '<')
      .to(
        lens.glint,
        {
          opacity: 1,
          duration: 0.12,
          ease: 'power1.out',
          yoyo: true,
          repeat: 1,
        },
        '>-0.1',
      )
      // Uma piscada no meio da busca
      .add(
        eyesScaleY(rig, 0.08, { duration: 0.07, ease: 'power2.in' }),
        '+=0.2',
      )
      .add(eyesScaleY(rig, 1))
      // Guarda a lupa
      .to(
        both,
        { scale: 0.4, opacity: 0, duration: 0.25, ease: 'power2.in' },
        '+=0.3',
      )
      .add(armTo(d, REST.d, { duration: 0.45, ease: 'power2.inOut' }), '<')
      .to(group, { x: 0, y: 0, duration: 0.3, ease: 'power2.inOut' }, '<');

    return { timeline: tl, stillAt: 0.22 };
  },
};
