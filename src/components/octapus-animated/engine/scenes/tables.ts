import { gsap } from 'gsap';
import type { ArmPose } from '../geometry';
import { blink, breathe, type Scene } from '../motion';
import { miniTable, sparkle } from '../props';
import { ARM_NAMES, REST, armTo, svgEl, type ArmName } from '../rig';

/** O polvo sobe um pouco para caber a fileira de tabelas embaixo */
const LIFT = -60;
/** Altura (centro) das tabelas: na frente, abaixo dos tentáculos */
const ROW_Y = 800;
const SCALE = 0.9;
/** Onde cada tentáculo põe a sua tabela, da esquerda para a direita */
const SLOTS: Record<ArmName, number> = { a: 70, b: 305, c: 555, d: 790 };
/** Centro da tabela unida (quatro colunas de 70 × escala) */
const JOIN_X = 430;
const COLUMN = 70 * SCALE;

/**
 * Pose com a ponta pousada no topo da tabela: sai da raiz e desce.
 * `bend` empurra o meio da curva para fora.
 */
function holding(name: ArmName, bend: { x: number; y: number }): ArmPose {
  const p0 = REST[name].p0;
  const tip = { x: SLOTS[name], y: ROW_Y - 80 * SCALE - 4 };
  return {
    p0: { ...p0 },
    p1: { x: p0.x + bend.x, y: p0.y + bend.y },
    p2: { x: tip.x + bend.x * 0.3, y: tip.y - 90 },
    p3: tip,
  };
}

/**
 * Tabelas (executando a consulta): cada tentáculo traz uma tabela e a pousa
 * na frente, embaixo; eles soltam e as quatro deslizam umas contra as
 * outras até virar uma só, maior — o JOIN. Brilho, um pulinho e recomeça.
 */
export const tablesScene: Scene = {
  id: 'tables',
  label: 'Tabelas',
  phase: 'executing · executando',
  loop: true,
  build(rig) {
    gsap.set(rig.figure, { y: LIFT });
    breathe(rig, 6);
    blink(rig);
    // As pontas pousam inteiras sobre o cabeçalho da tabela que seguram
    for (const name of ARM_NAMES) rig.arms[name].grip = true;

    // Uma tabela de uma coluna para cada tentáculo (e o recorte dela)
    const singles = ARM_NAMES.map(name => {
      const both = [miniTable(rig.front, 1), miniTable(rig.cutFront, 1, true)];
      gsap.set(both, {
        x: SLOTS[name],
        y: ROW_Y,
        scale: 0,
        transformOrigin: '50% 50%',
      });
      return both;
    });
    const all = singles.flat();

    const joined = [miniTable(rig.front, 4), miniTable(rig.cutFront, 4, true)];
    gsap.set(joined, {
      x: JOIN_X,
      y: ROW_Y,
      scale: 0,
      transformOrigin: '50% 50%',
    });

    const sparkles = svgEl('g', {}, rig.front);
    const stars = [
      { x: JOIN_X - 230, y: ROW_Y - 90, s: 40 },
      { x: JOIN_X + 240, y: ROW_Y - 70, s: 30 },
      { x: JOIN_X + 190, y: ROW_Y - 170, s: 24 },
    ].map(({ x, y, s }) => {
      const star = sparkle(sparkles, s);
      gsap.set(star, { x, y, scale: 0, transformOrigin: '50% 50%' });
      return star;
    });

    const hold: Record<ArmName, ArmPose> = {
      a: holding('a', { x: -170, y: 10 }),
      b: holding('b', { x: -60, y: 100 }),
      c: holding('c', { x: 60, y: 100 }),
      d: holding('d', { x: 180, y: 10 }),
    };

    const tl = gsap.timeline({ repeat: -1, repeatDelay: 0.3 });
    // Descem cada um com a sua tabela, um pouco escalonados
    ARM_NAMES.forEach((name, i) => {
      tl.add(
        armTo(rig.arms[name], hold[name], {
          duration: 0.45,
          ease: 'power2.out',
        }),
        i * 0.08,
      ).to(
        singles[i],
        { scale: SCALE, duration: 0.3, ease: 'back.out(2)' },
        0.15 + i * 0.08,
      );
    });
    tl.to(
      rig.eyes.group,
      { x: 0, y: 24, duration: 0.3, ease: 'power2.out' },
      0.2,
    );
    // Soltam: os tentáculos sobem de volta e as tabelas ficam
    ARM_NAMES.forEach((name, i) => {
      tl.add(
        armTo(rig.arms[name], REST[name], {
          duration: 0.4,
          ease: 'power2.inOut',
        }),
        0.95 + i * 0.04,
      );
    });
    // As tabelas deslizam (acelerando, como ímãs) para as colunas da unida
    ARM_NAMES.forEach((_, i) => {
      tl.to(
        singles[i],
        { x: JOIN_X + (i - 1.5) * COLUMN, duration: 0.35, ease: 'power3.in' },
        1.15,
      );
    });
    // Encaixe: as quatro somem e a unida aparece com passada
    tl.to(
      all,
      { scale: SCALE * 0.9, opacity: 0, duration: 0.1, ease: 'power2.in' },
      1.5,
    )
      .to(joined, { scale: SCALE, duration: 0.35, ease: 'back.out(2.4)' }, 1.5)
      .to(
        stars,
        {
          scale: 1,
          rotation: 45,
          duration: 0.25,
          ease: 'back.out(3)',
          stagger: 0.07,
        },
        1.55,
      )
      .to(
        stars,
        { scale: 0, duration: 0.2, ease: 'power2.in', stagger: 0.07 },
        1.9,
      )
      .to(
        rig.eyes.group,
        { x: 0, y: 16, duration: 0.25, ease: 'power2.out' },
        1.5,
      )
      // Pulinho: agacha (rápido) e sobe (devagar), assenta com passada
      .to(
        rig.figure,
        {
          scaleY: 0.95,
          svgOrigin: '420 780',
          duration: 0.1,
          ease: 'power3.in',
        },
        1.55,
      )
      .to(rig.figure, {
        scaleY: 1.03,
        svgOrigin: '420 780',
        duration: 0.18,
        ease: 'sine.out',
      })
      .to(rig.figure, {
        scaleY: 1,
        svgOrigin: '420 780',
        duration: 0.25,
        ease: 'back.out(3)',
      })
      // Guarda a tabela unida e volta tudo para o começo
      .to(joined, { scale: 0, duration: 0.25, ease: 'power2.in' }, '+=0.8')
      .to(
        rig.eyes.group,
        { x: 0, y: 0, duration: 0.3, ease: 'power2.inOut' },
        '<',
      );
    ARM_NAMES.forEach((name, i) => {
      tl.set(singles[i], { x: SLOTS[name], scale: 0, opacity: 1 });
    });

    return { timeline: tl, stillAt: 0.6 };
  },
};
