import { gsap } from 'gsap';
import { bringToFront, eyesScaleY, poseFrom, type Scene } from '../motion';
import { drop, sparkle } from '../props';
import { ARM_NAMES, REST, armTo, svgEl } from '../rig';

/**
 * Coçando a cabeça (falha ou limite esgotado): toca uma vez e segura o fim. O
 * tentáculo da direita sobe por cima da cabeça e coça, os olhos semicerram e
 * uma gota de suor escorre pelo lado.
 */
export const scratchScene: Scene = {
  id: 'scratch',
  label: 'Coçando a cabeça',
  phase: 'failed / exhausted · uma vez',
  loop: false,
  build(rig) {
    bringToFront(rig, 'd');
    const d = rig.arms.d;
    // Sobe por fora (com o fundo aparecendo entre o braço e a cabeça) e só
    // a ponta passa por cima do alto da cabeça
    const onHead = {
      p0: { ...REST.d.p0 },
      p1: { x: 790, y: 390 },
      p2: { x: 800, y: 20 },
      p3: { x: 575, y: 45 },
    };
    const sweat = drop(rig.front);
    gsap.set(sweat, { x: 190, y: 150, scale: 0, transformOrigin: '50% 0%' });

    const tl = gsap.timeline();
    tl.to(
      rig.figure,
      { rotation: -5, svgOrigin: '420 560', duration: 0.3, ease: 'power2.out' },
      0,
    )
      .add(armTo(d, onHead, { duration: 0.45, ease: 'back.out(1.4)' }), 0)
      .add(eyesScaleY(rig, 0.55, { duration: 0.2 }), 0.2)
      .to(
        rig.eyes.group,
        { x: 10, y: -10, duration: 0.25, ease: 'power2.out' },
        0.2,
      )
      // Coça: vai e volta rápido
      .to(
        d.pose.p3,
        {
          x: '-=40',
          duration: 0.09,
          ease: 'sine.inOut',
          yoyo: true,
          repeat: 7,
        },
        0.5,
      )
      .to(sweat, { scale: 1, duration: 0.2, ease: 'back.out(2)' }, 0.6)
      .to(
        sweat,
        { y: '+=90', opacity: 0, duration: 0.7, ease: 'power2.in' },
        0.85,
      )
      // Fica assim, meio sem graça
      .to(
        rig.figure,
        {
          rotation: -3,
          svgOrigin: '420 560',
          duration: 0.3,
          ease: 'sine.inOut',
        },
        1.3,
      );

    return { timeline: tl, stillAt: 0.6 };
  },
};

/**
 * Comemorando (resposta pronta): agacha rápido, salta com os quatro
 * tentáculos para cima e os olhos "sorrindo", brilhos em volta, aterrissa
 * com um amasso e assenta. Uma vez.
 */
export const celebrateScene: Scene = {
  id: 'celebrate',
  label: 'Comemorando',
  phase: 'answered · uma vez',
  loop: false,
  build(rig) {
    const up = {
      a: poseFrom('a', {
        p1: { x: -20, y: -60 },
        p2: { x: -10, y: -160 },
        p3: { x: 40, y: -300 },
      }),
      b: poseFrom('b', { p2: { x: -60, y: -20 }, p3: { x: -110, y: -40 } }),
      c: poseFrom('c', { p2: { x: 50, y: -20 }, p3: { x: 100, y: -40 } }),
      d: poseFrom('d', {
        p1: { x: 20, y: -60 },
        p2: { x: 10, y: -200 },
        p3: { x: -60, y: -330 },
      }),
    };
    const sparkles = svgEl('g', {}, rig.front);
    const stars = [
      { x: 110, y: 60, s: 50 },
      { x: 760, y: 40, s: 44 },
      { x: 660, y: -60, s: 28 },
      { x: 200, y: -40, s: 30 },
    ].map(({ x, y, s }) => {
      const star = sparkle(sparkles, s);
      gsap.set(star, { x, y, scale: 0, transformOrigin: '50% 50%' });
      return star;
    });

    const ground = '420 780';
    const tl = gsap.timeline();
    // Antecipação: agacha
    tl.to(
      rig.figure,
      {
        scaleY: 0.9,
        scaleX: 1.05,
        svgOrigin: ground,
        duration: 0.12,
        ease: 'power3.in',
      },
      0,
    )
      // Salto: sobe com sine.out, desce com power3.in (a "gravidade")
      .to(
        rig.figure,
        {
          scaleY: 1.06,
          scaleX: 0.97,
          svgOrigin: ground,
          duration: 0.18,
          ease: 'power2.out',
        },
        0.12,
      )
      .to(rig.figure, { y: -110, duration: 0.34, ease: 'sine.out' }, 0.12)
      .to(rig.figure, { y: 0, duration: 0.24, ease: 'power3.in' }, 0.5)
      .add(eyesScaleY(rig, 0.45, { duration: 0.12 }), 0.12)
      .to(
        stars,
        {
          scale: 1,
          rotation: 90,
          duration: 0.3,
          ease: 'back.out(3)',
          stagger: 0.06,
        },
        0.2,
      )
      .to(
        stars,
        { scale: 0, duration: 0.25, ease: 'power2.in', stagger: 0.06 },
        0.75,
      );
    for (const name of ARM_NAMES) {
      tl.add(
        armTo(rig.arms[name], up[name], { duration: 0.25, ease: 'power2.out' }),
        0.12,
      ).add(
        armTo(rig.arms[name], REST[name], {
          duration: 0.4,
          ease: 'back.out(1.8)',
        }),
        0.62,
      );
    }
    // Aterrissa: amassa e assenta com passada
    tl.to(
      rig.figure,
      {
        scaleY: 0.92,
        scaleX: 1.04,
        svgOrigin: ground,
        duration: 0.08,
        ease: 'power2.out',
      },
      0.74,
    )
      .to(
        rig.figure,
        {
          scaleY: 1,
          scaleX: 1,
          svgOrigin: ground,
          duration: 0.35,
          ease: 'back.out(3)',
        },
        0.82,
      )
      .add(eyesScaleY(rig, 1, { duration: 0.2 }), 1.0);

    return { timeline: tl, stillAt: 0.3 };
  },
};
